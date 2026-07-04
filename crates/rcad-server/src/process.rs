//! Process digital-twin telemetry + flow control (mock/simulation stage).
//!
//! A background loop runs a small **process-flow state machine**: a part moves
//! along a conveyor, stops at each station to "process" for a dwell time, then
//! resumes. It publishes "tags" (sensor/actuator values) as JSON over a
//! broadcast bus; a WebSocket streams them to the twin, and an operator can
//! start/stop/reset the line. Everything is keyed on named tags so this mock
//! source can later be swapped for a real MQTT / OPC UA gateway without changing
//! the twin or the flow logic.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;

/// Telemetry bus: JSON tag snapshots fan out to every connected client.
pub type Tx = broadcast::Sender<String>;

/// Operator control shared with the sim loop.
pub struct Control {
    pub running: bool,
    pub reset: bool,
    pub dwell: f64,     // station processing time (s)
    pub speed: f64,     // belt speed (mm/s)
    pub fail_rate: f64, // mock vision defect rate at the inspection station (%)
}
impl Default for Control {
    fn default() -> Self {
        Self { running: true, reset: false, dwell: 1.5, speed: 60.0, fail_rate: 20.0 }
    }
}

/// Router state: telemetry bus + shared control + the latest snapshot (so the
/// AI copilot can read line status over REST without an open WebSocket).
#[derive(Clone)]
pub struct AppState {
    pub tx: Tx,
    pub ctrl: Arc<Mutex<Control>>,
    pub latest: Arc<Mutex<String>>,
}

const LENGTH: f64 = 300.0; // conveyor length (mm)
const STATIONS: [f64; 2] = [100.0, 220.0];
const PROXIMITY: f64 = 12.0; // sensor trip window (± mm)
const TICK_MS: u64 = 100;

#[derive(Clone, Copy, PartialEq)]
enum Flow {
    Moving,
    Processing(usize), // station index
}

/// Spawn the mock conveyor flow, publishing a tag snapshot each tick.
pub fn spawn_sim(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_MS));
        let dt = TICK_MS as f64 / 1000.0;
        const INSPECT: usize = 1; // station 2 is the vision-inspection station
        let mut pos = 0.0_f64;
        let mut pass_count: u64 = 0;
        let mut fail_count: u64 = 0;
        let mut inspection_num: u64 = 0;
        let mut last_inspection: Option<bool> = None; // Some(pass) since last station 2
        let mut t = 0.0_f64;
        let mut flow = Flow::Moving;
        let mut dwell_left = 0.0_f64;
        let mut processed = [false; 2]; // stations handled on this pass
        loop {
            interval.tick().await;

            // Read + consume operator control.
            let (running, dwell, speed, fail_rate) = {
                let mut c = state.ctrl.lock().unwrap();
                if c.reset {
                    c.reset = false;
                    pos = 0.0;
                    pass_count = 0;
                    fail_count = 0;
                    inspection_num = 0;
                    last_inspection = None;
                    flow = Flow::Moving;
                    dwell_left = 0.0;
                    processed = [false; 2];
                }
                (c.running, c.dwell, c.speed, c.fail_rate)
            };
            t += dt;

            let mut conveyor_running = false;
            match flow {
                Flow::Moving if running => {
                    conveyor_running = true;
                    pos += speed * dt;
                    if pos >= LENGTH {
                        pos -= LENGTH;
                        // Part left the belt → tally on its inspection verdict.
                        match last_inspection {
                            Some(false) => fail_count += 1,
                            _ => pass_count += 1, // passed or never inspected
                        }
                        last_inspection = None;
                        processed = [false; 2];
                    }
                    for (i, &s) in STATIONS.iter().enumerate() {
                        if !processed[i] && (pos - s).abs() <= PROXIMITY {
                            flow = Flow::Processing(i); // stop + run the station op
                            dwell_left = dwell;
                            processed[i] = true;
                            conveyor_running = false;
                            break;
                        }
                    }
                }
                Flow::Processing(i) if running => {
                    dwell_left -= dt;
                    if dwell_left <= 0.0 {
                        dwell_left = 0.0;
                        if i == INSPECT {
                            // Mock vision: deterministic pseudo-random pass/fail.
                            let h = (inspection_num.wrapping_mul(2654435761) >> 8) % 100;
                            last_inspection = Some((h as f64) >= fail_rate);
                            inspection_num += 1;
                        }
                        flow = Flow::Moving; // resume the belt
                    }
                }
                _ => {} // stopped by operator → hold
            }

            let busy = |i: usize| flow == Flow::Processing(i);
            let state_str = match flow {
                Flow::Moving => "MOVING".to_string(),
                Flow::Processing(i) if i == INSPECT => "INSPECTING · S2".to_string(),
                Flow::Processing(i) => format!("PROCESSING · S{}", i + 1),
            };
            let inspection = match last_inspection {
                Some(true) => "PASS",
                Some(false) => "FAIL",
                None => "—",
            };
            let near = |s: f64| (pos - s).abs() <= PROXIMITY;
            let snapshot = json!({
                "time": (t * 10.0).round() / 10.0,
                "layout": { "length": LENGTH, "stations": STATIONS },
                "flow": state_str,
                "tags": {
                    "conveyor.speed": speed,
                    "conveyor.position": (pos * 10.0).round() / 10.0,
                    "conveyor.running": conveyor_running,
                    "station1.proximity": near(STATIONS[0]),
                    "station2.proximity": near(STATIONS[1]),
                    "station1.busy": busy(0),
                    "station2.busy": busy(1),
                    "dwell.remaining": (dwell_left * 10.0).round() / 10.0,
                    "inspection.result": inspection,
                    "inspection.pass": pass_count,
                    "inspection.fail": fail_count,
                    "throughput.count": pass_count,
                    "reject.count": fail_count,
                }
            });
            let msg = snapshot.to_string();
            *state.latest.lock().unwrap() = msg.clone();
            let _ = state.tx.send(msg);
        }
    });
}

/// `GET /api/telemetry/ws` — stream tag snapshots to a twin client.
pub async fn telemetry_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state.tx.subscribe()))
}

async fn handle_socket(socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    let (mut sink, mut stream) = socket.split();
    loop {
        tokio::select! {
            recv = rx.recv() => match recv {
                Ok(msg) => {
                    if sink.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            client = stream.next() => match client {
                Some(Ok(_)) => continue, // ignore inbound frames
                _ => break,
            },
        }
    }
}

#[derive(Deserialize)]
pub struct ControlReq {
    command: String,
    dwell: Option<f64>,
    speed: Option<f64>,
    fail_rate: Option<f64>,
}

/// `POST /api/telemetry/control` — supervisory control: start/stop/reset plus
/// dwell/speed config (any command may carry dwell/speed).
pub async fn control(State(state): State<AppState>, Json(req): Json<ControlReq>) -> impl IntoResponse {
    let mut c = state.ctrl.lock().unwrap();
    match req.command.as_str() {
        "start" => c.running = true,
        "stop" => c.running = false,
        "reset" => {
            c.reset = true;
            c.running = true;
        }
        _ => {} // "config" / anything else → just apply dwell/speed below
    }
    if let Some(d) = req.dwell {
        c.dwell = d.clamp(0.0, 30.0);
    }
    if let Some(s) = req.speed {
        c.speed = s.clamp(0.0, 300.0);
    }
    if let Some(fr) = req.fail_rate {
        c.fail_rate = fr.clamp(0.0, 100.0);
    }
    Json(json!({ "ok": true, "running": c.running, "dwell": c.dwell, "speed": c.speed, "fail_rate": c.fail_rate }))
}

/// `GET /api/telemetry/status` — the latest tag snapshot (for the AI copilot),
/// patched with the current config setpoints so a status read right after a
/// set_speed/set_dwell reflects the change without waiting for the next tick.
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let s = state.latest.lock().unwrap().clone();
    let mut value: serde_json::Value = serde_json::from_str(&s).unwrap_or_else(|_| json!({}));
    let (speed, dwell, running, fail_rate) = {
        let c = state.ctrl.lock().unwrap();
        (c.speed, c.dwell, c.running, c.fail_rate)
    };
    if let Some(tags) = value.get_mut("tags").and_then(|t| t.as_object_mut()) {
        tags.insert("conveyor.speed".into(), json!(speed));
        tags.insert("station.dwell".into(), json!(dwell));
        tags.insert("conveyor.enabled".into(), json!(running));
        tags.insert("inspection.fail_rate".into(), json!(fail_rate));
    }
    Json(value)
}
