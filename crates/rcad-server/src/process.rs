//! Process digital-twin telemetry + flow control (mock/simulation stage).
//!
//! A background loop runs a **general process-flow graph engine**: the line is a
//! directed graph of nodes (source / process / inspect / sink) joined by
//! transitions that can branch on an inspection verdict (pass/fail). Parts flow
//! through the graph — several in flight at once (WIP) — dwelling at each station
//! and travelling the edges between them. The engine publishes "tags"
//! (sensor/actuator values) plus the graph layout and live part positions as
//! JSON over a broadcast bus; a WebSocket streams them to the twin, and an
//! operator can start/stop/reset the line and author the graph. Everything is
//! keyed on named tags so this mock source can later be swapped for a real
//! MQTT / OPC UA gateway without changing the twin or the flow logic.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
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

/// An authorable automation rule: when `tag op value` holds, run `action`.
/// Actions stop/start/set_* fire once on the rising edge; `alarm` is level-held.
#[derive(Clone, Deserialize, Serialize)]
pub struct Rule {
    pub tag: String,
    pub op: String, // > < >= <= == !=
    pub value: f64,
    pub action: String, // stop | start | set_speed | set_dwell | set_fail_rate | alarm
    #[serde(default)]
    pub arg: f64,
    #[serde(default)]
    pub message: String,
}

// ---------------------------------------------------------------------------
// Flow graph — an authorable multi-step process.
// ---------------------------------------------------------------------------

/// A node in the process flow graph.
#[derive(Clone, Deserialize, Serialize)]
pub struct FlowNode {
    pub id: String,
    pub kind: String, // source | process | inspect | sink
    pub pos: f64,     // mm along the line (belt position + 2D layout x)
    #[serde(default)]
    pub y: f64, // lane offset for 2D layout (branches sit off the main belt)
    #[serde(default)]
    pub dwell: Option<f64>, // per-node processing time (else the global dwell)
    #[serde(default)]
    pub fail_rate: Option<f64>, // per-inspect defect rate (else the global fail_rate)
    #[serde(default)]
    pub label: Option<String>,
}

/// A directed transition. `when` routes on the upstream inspect verdict:
/// "pass" / "fail" take that branch; None (or "always") is unconditional.
#[derive(Clone, Deserialize, Serialize)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub dist: Option<f64>, // travel distance (else derived from node spacing)
}

/// The whole process as a graph, plus how often parts enter at the source.
#[derive(Clone, Deserialize, Serialize)]
pub struct FlowGraph {
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(default = "default_spawn")]
    pub spawn_interval: f64, // seconds between new parts at the source
}
fn default_spawn() -> f64 {
    3.0
}

fn node(id: &str, kind: &str, pos: f64, y: f64, label: &str) -> FlowNode {
    FlowNode {
        id: id.into(),
        kind: kind.into(),
        pos,
        y,
        dwell: None,
        fail_rate: None,
        label: Some(label.into()),
    }
}
fn edge(from: &str, to: &str, when: Option<&str>) -> FlowEdge {
    FlowEdge { from: from.into(), to: to.into(), when: when.map(Into::into), dist: None }
}

/// The default line: load → mill → inspect →(pass) pack / (fail) reject.
/// Node ids `station1`/`station2` are kept so the existing twin sensor bindings
/// and per-station tags carry over unchanged.
pub fn default_graph() -> FlowGraph {
    FlowGraph {
        nodes: vec![
            node("load", "source", 0.0, 0.0, "Load"),
            node("station1", "process", 100.0, 0.0, "Mill"),
            node("station2", "inspect", 220.0, 0.0, "Inspect"),
            node("pack", "sink", 320.0, 0.0, "Pack"),
            node("reject", "sink", 260.0, -1.0, "Reject"),
        ],
        edges: vec![
            edge("load", "station1", None),
            edge("station1", "station2", None),
            edge("station2", "pack", Some("pass")),
            edge("station2", "reject", Some("fail")),
        ],
        spawn_interval: 3.0,
    }
}

/// Router state: telemetry bus + shared control + the latest snapshot (so the
/// AI copilot can read line status over REST without an open WebSocket) +
/// external tag overrides (real-hardware source injected over HTTP or MQTT) +
/// authorable automation rules + the authorable process flow graph.
#[derive(Clone)]
pub struct AppState {
    pub tx: Tx,
    pub ctrl: Arc<Mutex<Control>>,
    pub latest: Arc<Mutex<String>>,
    pub overrides: Arc<Mutex<serde_json::Map<String, Value>>>,
    pub rules: Arc<Mutex<Vec<Rule>>>,
    pub graph: Arc<Mutex<FlowGraph>>,
}

/// Evaluate one rule's condition against the current tags.
fn eval_cond(tags: &serde_json::Map<String, Value>, r: &Rule) -> bool {
    let x = match tags.get(&r.tag) {
        Some(v) => v.as_f64().or_else(|| v.as_bool().map(|b| if b { 1.0 } else { 0.0 })),
        None => None,
    };
    let Some(x) = x else { return false };
    match r.op.as_str() {
        ">" => x > r.value,
        "<" => x < r.value,
        ">=" => x >= r.value,
        "<=" => x <= r.value,
        "==" => (x - r.value).abs() < 1e-9,
        "!=" => (x - r.value).abs() >= 1e-9,
        _ => false,
    }
}

const PROXIMITY: f64 = 12.0; // sensor trip window (± mm)
const TICK_MS: u64 = 100;
const MAX_PARTS: usize = 16; // WIP cap so a fast line can't run away

/// Where a part is right now: dwelling at a node, or travelling an edge.
#[derive(Clone, Copy)]
enum Loc {
    At { node: usize, dwell_left: f64, judged: bool },
    Travel { edge: usize, prog: f64, dist: f64 },
}

/// A work-piece flowing through the graph.
struct Part {
    id: u64,
    verdict: Option<bool>, // last inspection result it carries (routes branches)
    loc: Loc,
}

fn node_idx(g: &FlowGraph, id: &str) -> Option<usize> {
    g.nodes.iter().position(|n| n.id == id)
}

/// Travel distance of an edge — explicit, or derived from node spacing (lanes
/// count as ~60 mm so a branch off the belt has a sensible length).
fn edge_dist(g: &FlowGraph, e: &FlowEdge) -> f64 {
    if let Some(d) = e.dist {
        return d.max(1.0);
    }
    match (node_idx(g, &e.from), node_idx(g, &e.to)) {
        (Some(a), Some(b)) => {
            let (na, nb) = (&g.nodes[a], &g.nodes[b]);
            ((nb.pos - na.pos).powi(2) + ((nb.y - na.y) * 60.0).powi(2)).sqrt().max(1.0)
        }
        _ => 60.0,
    }
}

/// Screen position (x mm, lane y) of a part for the twin.
fn part_xy(g: &FlowGraph, p: &Part) -> (f64, f64) {
    match p.loc {
        Loc::At { node, .. } => (g.nodes[node].pos, g.nodes[node].y),
        Loc::Travel { edge, prog, dist } => {
            let e = &g.edges[edge];
            match (node_idx(g, &e.from), node_idx(g, &e.to)) {
                (Some(a), Some(b)) => {
                    let (na, nb) = (&g.nodes[a], &g.nodes[b]);
                    let f = (prog / dist).clamp(0.0, 1.0);
                    (na.pos + (nb.pos - na.pos) * f, na.y + (nb.y - na.y) * f)
                }
                _ => (0.0, 0.0),
            }
        }
    }
}

/// Choose the outgoing edge from a node given the part's verdict.
fn pick_edge(g: &FlowGraph, node_id: &str, verdict: Option<bool>) -> Option<usize> {
    let want = match verdict {
        Some(true) => "pass",
        Some(false) => "fail",
        None => "always",
    };
    let outs: Vec<usize> =
        g.edges.iter().enumerate().filter(|(_, e)| e.from == node_id).map(|(i, _)| i).collect();
    // Prefer an edge matching the verdict, then an unconditional one, then any.
    outs.iter()
        .find(|&&i| g.edges[i].when.as_deref() == Some(want))
        .or_else(|| {
            outs.iter()
                .find(|&&i| matches!(g.edges[i].when.as_deref(), None | Some("always") | Some("")))
        })
        .or_else(|| outs.first())
        .copied()
}

fn node_dwell(n: &FlowNode, global: f64) -> f64 {
    match n.kind.as_str() {
        "process" | "inspect" => n.dwell.unwrap_or(global),
        _ => 0.0, // source/sink are instantaneous
    }
}

/// Advance one part by `dt`. Returns true if the part has left the line (reached
/// a sink or a dead end) and should be removed.
#[allow(clippy::too_many_arguments)]
fn advance_part(
    p: &mut Part,
    g: &FlowGraph,
    dt: f64,
    speed: f64,
    dwell: f64,
    fail_rate: f64,
    inspection_num: &mut u64,
    last_inspection: &mut Option<bool>,
    pass_count: &mut u64,
    fail_count: &mut u64,
    counts: &mut HashMap<String, u64>,
) -> bool {
    match p.loc {
        Loc::At { node, mut dwell_left, mut judged } => {
            let n = &g.nodes[node];
            if dwell_left > 0.0 {
                dwell_left -= dt;
                p.loc = Loc::At { node, dwell_left, judged };
                return false;
            }
            // Dwell finished: run the station's operation, then leave.
            if n.kind == "inspect" && !judged {
                let fr = n.fail_rate.unwrap_or(fail_rate);
                let h = (*inspection_num).wrapping_mul(2654435761) >> 8;
                let pass = (h % 100) as f64 >= fr;
                p.verdict = Some(pass);
                *last_inspection = Some(pass);
                *inspection_num += 1;
                judged = true;
                let _ = judged; // consumed below via the edge choice
            }
            if n.kind == "sink" {
                match p.verdict {
                    Some(false) => *fail_count += 1,
                    _ => *pass_count += 1,
                }
                *counts.entry(n.id.clone()).or_default() += 1;
                return true;
            }
            match pick_edge(g, &n.id, p.verdict) {
                Some(ei) => {
                    *counts.entry(n.id.clone()).or_default() += 1;
                    let dist = edge_dist(g, &g.edges[ei]);
                    p.loc = Loc::Travel { edge: ei, prog: 0.0, dist };
                    false
                }
                None => true, // nowhere to go → drop it
            }
        }
        Loc::Travel { edge, mut prog, dist } => {
            prog += speed * dt;
            if prog >= dist {
                let to = g.edges[edge].to.clone();
                match node_idx(g, &to) {
                    Some(ti) => {
                        let d = node_dwell(&g.nodes[ti], dwell);
                        p.loc = Loc::At { node: ti, dwell_left: d, judged: false };
                        false
                    }
                    None => true,
                }
            } else {
                p.loc = Loc::Travel { edge, prog, dist };
                false
            }
        }
    }
}

/// Spawn the process-flow-graph engine, publishing a tag snapshot each tick.
pub fn spawn_sim(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_MS));
        let dt = TICK_MS as f64 / 1000.0;

        let mut parts: Vec<Part> = Vec::new();
        let mut next_id: u64 = 1;
        let mut spawn_timer = 0.0_f64;
        let mut pass_count: u64 = 0;
        let mut fail_count: u64 = 0;
        let mut inspection_num: u64 = 0;
        let mut last_inspection: Option<bool> = None;
        let mut counts: HashMap<String, u64> = HashMap::new();
        let mut last_pos = 0.0_f64;
        let mut t = 0.0_f64;
        let mut last_fired: HashMap<String, bool> = HashMap::new(); // rule edge state

        loop {
            interval.tick().await;

            // Read + consume operator control.
            let (running, dwell, speed, fail_rate) = {
                let mut c = state.ctrl.lock().unwrap();
                if c.reset {
                    c.reset = false;
                    parts.clear();
                    next_id = 1;
                    spawn_timer = 0.0;
                    pass_count = 0;
                    fail_count = 0;
                    inspection_num = 0;
                    last_inspection = None;
                    counts.clear();
                    last_pos = 0.0;
                }
                (c.running, c.dwell, c.speed, c.fail_rate)
            };
            t += dt;

            let g = state.graph.lock().unwrap().clone();

            // Spawn a new part at the first source node on the spawn interval.
            spawn_timer += dt;
            if running && spawn_timer >= g.spawn_interval && parts.len() < MAX_PARTS {
                if let Some(src) = g.nodes.iter().position(|n| n.kind == "source") {
                    parts.push(Part {
                        id: next_id,
                        verdict: None,
                        loc: Loc::At { node: src, dwell_left: 0.0, judged: false },
                    });
                    next_id += 1;
                    spawn_timer = 0.0;
                }
            }

            // Advance every part (the whole line holds while stopped).
            if running {
                let mut i = 0;
                while i < parts.len() {
                    let gone = advance_part(
                        &mut parts[i],
                        &g,
                        dt,
                        speed,
                        dwell,
                        fail_rate,
                        &mut inspection_num,
                        &mut last_inspection,
                        &mut pass_count,
                        &mut fail_count,
                        &mut counts,
                    );
                    if gone {
                        parts.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }

            // --- Derive tags from the live part set ---
            let at_node = |id: &str| {
                parts.iter().any(|p| matches!(p.loc, Loc::At { node, .. } if g.nodes[node].id == id))
            };
            let part_x: Vec<f64> = parts.iter().map(|p| part_xy(&g, p).0).collect();
            let near = |pos: f64| part_x.iter().any(|x| (x - pos).abs() <= PROXIMITY);
            let traveling = parts.iter().any(|p| matches!(p.loc, Loc::Travel { .. }));
            let dwell_rem = parts
                .iter()
                .filter_map(|p| match p.loc {
                    Loc::At { node, dwell_left, .. }
                        if matches!(g.nodes[node].kind.as_str(), "process" | "inspect") =>
                    {
                        Some(dwell_left)
                    }
                    _ => None,
                })
                .fold(0.0_f64, f64::max);

            // Primary part (oldest still on the line) drives the belt position
            // tag that the 3D twin binds to.
            let primary = parts.iter().min_by_key(|p| p.id);
            let pos_now = primary.map(|p| part_xy(&g, p).0).unwrap_or(last_pos);
            last_pos = pos_now;

            let flow_str = match primary {
                None => "IDLE".to_string(),
                Some(p) => match p.loc {
                    Loc::Travel { .. } => "MOVING".to_string(),
                    Loc::At { node, .. } => {
                        let n = &g.nodes[node];
                        let lbl = n.label.clone().unwrap_or_else(|| n.id.clone());
                        match n.kind.as_str() {
                            "inspect" => format!("INSPECTING · {lbl}"),
                            "process" => format!("PROCESSING · {lbl}"),
                            "source" => format!("LOADING · {lbl}"),
                            _ => lbl,
                        }
                    }
                },
            };
            let inspection = match last_inspection {
                Some(true) => "PASS",
                Some(false) => "FAIL",
                None => "—",
            };

            let mut tags = serde_json::Map::new();
            tags.insert("conveyor.speed".into(), json!(speed));
            tags.insert("conveyor.position".into(), json!((pos_now * 10.0).round() / 10.0));
            tags.insert("conveyor.running".into(), json!(traveling));
            tags.insert("dwell.remaining".into(), json!((dwell_rem * 10.0).round() / 10.0));
            tags.insert("inspection.result".into(), json!(inspection));
            tags.insert("inspection.pass".into(), json!(pass_count));
            tags.insert("inspection.fail".into(), json!(fail_count));
            tags.insert("throughput.count".into(), json!(pass_count));
            tags.insert("reject.count".into(), json!(fail_count));
            tags.insert("wip.count".into(), json!(parts.len()));
            // Per-station compatibility tags (kept for the default line's bindings).
            for sid in ["station1", "station2"] {
                if let Some(n) = g.nodes.iter().find(|n| n.id == sid) {
                    tags.insert(format!("{sid}.proximity"), json!(near(n.pos)));
                    tags.insert(format!("{sid}.busy"), json!(at_node(sid)));
                }
            }
            // Generic per-node tags so any authored graph is observable.
            for n in &g.nodes {
                tags.insert(format!("node.{}.busy", n.id), json!(at_node(&n.id)));
                tags.insert(
                    format!("node.{}.count", n.id),
                    json!(counts.get(&n.id).copied().unwrap_or(0)),
                );
            }

            // Overlay any external tag values (a real sensor source overrides the
            // mock). If any exist, the twin is being driven from outside.
            let source = {
                let ov = state.overrides.lock().unwrap();
                for (k, v) in ov.iter() {
                    tags.insert(k.clone(), v.clone());
                }
                if ov.is_empty() { "mock" } else { "external" }
            };

            // Evaluate automation rules against the effective tags. Control
            // actions fire once on the rising edge; alarms are held while true.
            let mut alarm_active = false;
            let mut alarm_msg = String::new();
            {
                let rules = state.rules.lock().unwrap();
                for r in rules.iter() {
                    let cond = eval_cond(&tags, r);
                    let key = format!("{}|{}|{}|{}", r.tag, r.op, r.value, r.action);
                    let was = *last_fired.get(&key).unwrap_or(&false);
                    if r.action == "alarm" {
                        if cond {
                            alarm_active = true;
                            if alarm_msg.is_empty() {
                                alarm_msg = if r.message.is_empty() {
                                    format!("{} {} {}", r.tag, r.op, r.value)
                                } else {
                                    r.message.clone()
                                };
                            }
                        }
                    } else if cond && !was {
                        let mut c = state.ctrl.lock().unwrap();
                        match r.action.as_str() {
                            "stop" => c.running = false,
                            "start" => c.running = true,
                            "set_speed" => c.speed = r.arg.clamp(0.0, 300.0),
                            "set_dwell" => c.dwell = r.arg.clamp(0.0, 30.0),
                            "set_fail_rate" => c.fail_rate = r.arg.clamp(0.0, 100.0),
                            _ => {}
                        }
                    }
                    last_fired.insert(key, cond);
                }
            }
            tags.insert("alarm.active".into(), json!(alarm_active));
            tags.insert("alarm.message".into(), json!(alarm_msg));

            // Live part positions + graph layout for the twin.
            let parts_json: Vec<Value> = parts
                .iter()
                .map(|p| {
                    let (x, y) = part_xy(&g, p);
                    let node = match p.loc {
                        Loc::At { node, .. } => Some(g.nodes[node].id.clone()),
                        _ => None,
                    };
                    json!({
                        "id": p.id,
                        "x": (x * 10.0).round() / 10.0,
                        "y": y,
                        "verdict": p.verdict,
                        "node": node,
                    })
                })
                .collect();
            let length = g.nodes.iter().map(|n| n.pos).fold(0.0_f64, f64::max);
            let stations: Vec<f64> = g
                .nodes
                .iter()
                .filter(|n| n.kind == "process" || n.kind == "inspect")
                .map(|n| n.pos)
                .collect();

            let snapshot = json!({
                "time": (t * 10.0).round() / 10.0,
                "layout": { "length": length, "stations": stations },
                "graph": { "nodes": g.nodes, "edges": g.edges },
                "parts": parts_json,
                "flow": flow_str,
                "source": source,
                "tags": tags,
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

#[derive(Deserialize)]
pub struct IngestReq {
    tags: serde_json::Map<String, Value>,
}

/// `POST /api/telemetry/ingest` — an external source (a real-hardware gateway)
/// pushes tag values that overlay the mock. A null value clears an override.
/// This is the same path the MQTT bridge feeds, proving the twin is
/// source-agnostic.
pub async fn ingest(State(state): State<AppState>, Json(req): Json<IngestReq>) -> impl IntoResponse {
    let mut ov = state.overrides.lock().unwrap();
    for (k, v) in req.tags {
        if v.is_null() {
            ov.remove(&k);
        } else {
            ov.insert(k, v);
        }
    }
    Json(json!({ "ok": true, "overrides": ov.len() }))
}

#[derive(Deserialize)]
pub struct RulesReq {
    rules: Vec<Rule>,
    #[serde(default)]
    mode: String, // "append" to add; anything else replaces
}

/// `GET /api/telemetry/rules` — the current automation rules.
pub async fn get_rules(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({ "rules": *state.rules.lock().unwrap() }))
}

/// `POST /api/telemetry/rules` — replace (or append to) the automation rules.
pub async fn set_rules(State(state): State<AppState>, Json(req): Json<RulesReq>) -> impl IntoResponse {
    let mut rules = state.rules.lock().unwrap();
    if req.mode == "append" {
        rules.extend(req.rules);
    } else {
        *rules = req.rules;
    }
    Json(json!({ "ok": true, "count": rules.len() }))
}

/// `GET /api/telemetry/graph` — the current process flow graph.
pub async fn get_graph(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({ "graph": *state.graph.lock().unwrap() }))
}

#[derive(Deserialize)]
pub struct GraphReq {
    graph: FlowGraph,
}

/// `POST /api/telemetry/graph` — replace the process flow graph and restart the
/// line on it. Rejects a graph without at least one source and one sink.
pub async fn set_graph(State(state): State<AppState>, Json(req): Json<GraphReq>) -> impl IntoResponse {
    let g = req.graph;
    let has_source = g.nodes.iter().any(|n| n.kind == "source");
    let has_sink = g.nodes.iter().any(|n| n.kind == "sink");
    if g.nodes.is_empty() || !has_source || !has_sink {
        return Json(json!({
            "ok": false,
            "error": "graph needs at least one source and one sink node"
        }));
    }
    let n = g.nodes.len();
    let e = g.edges.len();
    *state.graph.lock().unwrap() = g;
    state.ctrl.lock().unwrap().reset = true; // restart parts on the new graph
    Json(json!({ "ok": true, "nodes": n, "edges": e }))
}

/// Bridge the process twin to an MQTT broker when `MQTT_BROKER=host:port` is set:
/// publish each tag snapshot to `rcad/telemetry` and ingest external tag values
/// from `rcad/tags/in`. The twin/flow/AI are all tag-keyed, so nothing else
/// changes when the source becomes real hardware.
pub fn spawn_mqtt(state: AppState) {
    let broker = match std::env::var("MQTT_BROKER") {
        Ok(b) if !b.trim().is_empty() => b,
        _ => {
            tracing::info!("MQTT bridge disabled (set MQTT_BROKER=host:port to enable)");
            return;
        }
    };
    let (host, port) = match broker.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(1883)),
        None => (broker.clone(), 1883),
    };
    tracing::info!("MQTT bridge → {host}:{port}");

    tokio::spawn(async move {
        let mut opts = MqttOptions::new("rcad-server", host, port);
        opts.set_keep_alive(Duration::from_secs(5));
        let (client, mut eventloop) = AsyncClient::new(opts, 64);

        // Egress: republish every telemetry snapshot to the broker. Skip lag
        // (which happens while disconnected) instead of exiting the task.
        let mut rx = state.tx.subscribe();
        let pub_client = client.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        let _ = pub_client
                            .publish("rcad/telemetry", QoS::AtMostOnce, false, msg)
                            .await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // Ingress: external tag values → overrides. Re-subscribe on every
        // (re)connect so it survives broker restarts.
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    let _ = client.subscribe("rcad/tags/in", QoS::AtMostOnce).await;
                    tracing::info!("MQTT connected");
                }
                Ok(Event::Incoming(Packet::Publish(p))) if p.topic == "rcad/tags/in" => {
                    if let Ok(map) = serde_json::from_slice::<serde_json::Map<String, Value>>(&p.payload) {
                        let mut ov = state.overrides.lock().unwrap();
                        for (k, v) in map {
                            if v.is_null() {
                                ov.remove(&k);
                            } else {
                                ov.insert(k, v);
                            }
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("MQTT eventloop error: {e}; retrying");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}
