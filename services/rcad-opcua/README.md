# rCAD OPC UA Bridge

Bridges the rCAD process twin to **OPC UA**, the industrial-standard protocol
for PLC / SCADA / HMI systems — the same "swappable source" pattern as the MQTT
bridge (`crates/rcad-server/src/process.rs` `spawn_mqtt`), but over OPC UA so
real factory equipment can connect.

Implemented as a Python (`asyncua`) sidecar rather than in the Rust server: the
`opcua` crate needs an OpenSSL/pkg-config build and a complex address-space API,
while `asyncua` is a mature server+client in one and keeps the Rust core lean —
same rationale as the Newton and Vision services.

- **EGRESS** — subscribes to the rCAD telemetry WebSocket and mirrors every tag
  as a read-only Variable under `rCAD/Telemetry`, so an OPC UA client can
  browse, read and subscribe to the live line.
- **INGRESS** — exposes a writable `rCAD/Ingress/TagsIn` String node. Write a
  JSON object of tags to it and they are POSTed to `/api/telemetry/ingest`,
  overriding the mock line exactly like the MQTT `rcad/tags/in` topic.

## Run

```bash
cd services/rcad-opcua
uv sync
uv run python bridge.py
```

Serves `opc.tcp://0.0.0.0:4840/rcad/` (unencrypted, dev). Requires the rCAD
server running on :3000 (telemetry WS + `/api/telemetry/ingest`).

Env: `RCAD_HTTP` (default `http://127.0.0.1:3000`), `RCAD_WS`
(default `ws://127.0.0.1:3000/api/telemetry/ws`),
`OPCUA_ENDPOINT` (default `opc.tcp://0.0.0.0:4840/rcad/`).

## Verify

Any OPC UA client (UaExpert, or `asyncua`): browse `rCAD/Telemetry` and read a
tag (egress); write `{"external.opcua": 777}` to `rCAD/Ingress/TagsIn` and watch
the rCAD line flip to `source: external` with the tag applied (ingress).
