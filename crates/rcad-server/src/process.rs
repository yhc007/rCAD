//! Process digital-twin telemetry (mock/simulation stage).
//!
//! A background loop simulates a conveyor carrying a part past two stations and
//! publishes "tags" (sensor/actuator values) as JSON over a broadcast channel.
//! A WebSocket endpoint streams those tags to the frontend twin. Everything is
//! keyed on named tags so this mock source can later be swapped for a real MQTT
//! / OPC UA gateway without changing the twin or the process-flow logic.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use std::time::Duration;
use tokio::sync::broadcast;

/// Telemetry bus: JSON tag snapshots fan out to every connected client.
pub type Tx = broadcast::Sender<String>;

const LENGTH: f64 = 300.0; // conveyor length (mm)
const SPEED: f64 = 60.0; // mm/s
const STATIONS: [f64; 2] = [100.0, 220.0];
const PROXIMITY: f64 = 12.0; // sensor trip window (± mm)
const TICK_MS: u64 = 100;

/// Spawn the mock conveyor simulation, publishing a tag snapshot each tick.
pub fn spawn_sim(tx: Tx) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_MS));
        let dt = TICK_MS as f64 / 1000.0;
        let mut pos = 0.0_f64;
        let mut count: u64 = 0;
        let mut t = 0.0_f64;
        loop {
            interval.tick().await;
            t += dt;
            pos += SPEED * dt;
            if pos >= LENGTH {
                pos -= LENGTH;
                count += 1; // one part completed the belt
            }
            let near = |s: f64| (pos - s).abs() <= PROXIMITY;
            let snapshot = json!({
                "time": (t * 10.0).round() / 10.0,
                "layout": { "length": LENGTH, "stations": STATIONS },
                "tags": {
                    "conveyor.speed": SPEED,
                    "conveyor.position": (pos * 10.0).round() / 10.0,
                    "station1.proximity": near(STATIONS[0]),
                    "station2.proximity": near(STATIONS[1]),
                    "throughput.count": count,
                }
            });
            // Ignore send errors (no subscribers yet is fine).
            let _ = tx.send(snapshot.to_string());
        }
    });
}

/// `GET /api/telemetry/ws` — stream tag snapshots to a twin client.
pub async fn telemetry_ws(ws: WebSocketUpgrade, State(tx): State<Tx>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, tx.subscribe()))
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
                Some(Ok(_)) => continue, // ignore inbound (control comes later)
                _ => break,              // closed / errored
            },
        }
    }
}
