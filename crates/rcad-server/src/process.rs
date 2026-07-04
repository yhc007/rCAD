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
    pub dwell: f64, // station processing time (s)
}
impl Default for Control {
    fn default() -> Self {
        Self { running: true, reset: false, dwell: 1.5 }
    }
}

/// Router state: telemetry bus + shared control.
#[derive(Clone)]
pub struct AppState {
    pub tx: Tx,
    pub ctrl: Arc<Mutex<Control>>,
}

const LENGTH: f64 = 300.0; // conveyor length (mm)
const SPEED: f64 = 60.0; // mm/s
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
        let mut pos = 0.0_f64;
        let mut count: u64 = 0;
        let mut t = 0.0_f64;
        let mut flow = Flow::Moving;
        let mut dwell_left = 0.0_f64;
        let mut processed = [false; 2]; // stations handled on this pass
        loop {
            interval.tick().await;

            // Read + consume operator control.
            let (running, dwell) = {
                let mut c = state.ctrl.lock().unwrap();
                if c.reset {
                    c.reset = false;
                    pos = 0.0;
                    count = 0;
                    flow = Flow::Moving;
                    dwell_left = 0.0;
                    processed = [false; 2];
                }
                (c.running, c.dwell)
            };
            t += dt;

            let mut conveyor_running = false;
            match flow {
                Flow::Moving if running => {
                    conveyor_running = true;
                    pos += SPEED * dt;
                    if pos >= LENGTH {
                        pos -= LENGTH;
                        count += 1; // one part completed the belt
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
                Flow::Processing(_) if running => {
                    dwell_left -= dt;
                    if dwell_left <= 0.0 {
                        dwell_left = 0.0;
                        flow = Flow::Moving; // resume the belt
                    }
                }
                _ => {} // stopped by operator → hold
            }

            let busy = |i: usize| flow == Flow::Processing(i);
            let state_str = match flow {
                Flow::Moving => "MOVING".to_string(),
                Flow::Processing(i) => format!("PROCESSING · S{}", i + 1),
            };
            let near = |s: f64| (pos - s).abs() <= PROXIMITY;
            let snapshot = json!({
                "time": (t * 10.0).round() / 10.0,
                "layout": { "length": LENGTH, "stations": STATIONS },
                "flow": state_str,
                "tags": {
                    "conveyor.speed": SPEED,
                    "conveyor.position": (pos * 10.0).round() / 10.0,
                    "conveyor.running": conveyor_running,
                    "station1.proximity": near(STATIONS[0]),
                    "station2.proximity": near(STATIONS[1]),
                    "station1.busy": busy(0),
                    "station2.busy": busy(1),
                    "dwell.remaining": (dwell_left * 10.0).round() / 10.0,
                    "throughput.count": count,
                }
            });
            let _ = state.tx.send(snapshot.to_string());
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
}

/// `POST /api/telemetry/control` — supervisory start/stop/reset of the line.
pub async fn control(State(state): State<AppState>, Json(req): Json<ControlReq>) -> impl IntoResponse {
    let mut c = state.ctrl.lock().unwrap();
    match req.command.as_str() {
        "start" => c.running = true,
        "stop" => c.running = false,
        "reset" => {
            c.reset = true;
            c.running = true;
        }
        _ => {}
    }
    if let Some(d) = req.dwell {
        c.dwell = d.clamp(0.0, 30.0);
    }
    Json(json!({ "ok": true, "running": c.running, "dwell": c.dwell }))
}
