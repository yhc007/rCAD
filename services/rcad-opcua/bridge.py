"""OPC UA <-> rCAD telemetry bridge.

Bridges the rCAD process twin to the industrial-standard OPC UA protocol, the
same "swappable source" pattern as the MQTT bridge (crates/rcad-server
process.rs spawn_mqtt) but over OPC UA so a real PLC/SCADA/HMI can connect.

- EGRESS: subscribes to the rCAD telemetry WebSocket and mirrors every tag as a
  read-only Variable under rCAD/Telemetry, so an OPC UA client can browse,
  read and subscribe to the live line.
- INGRESS: exposes a writable rCAD/Ingress/TagsIn String node. When a client
  writes a JSON object of tags to it, they are POSTed to /api/telemetry/ingest,
  overriding the mock line exactly like the MQTT rcad/tags/in topic.

Run:  uv run python bridge.py
Env:  RCAD_HTTP (default http://127.0.0.1:3000)
      RCAD_WS   (default ws://127.0.0.1:3000/api/telemetry/ws)
      OPCUA_ENDPOINT (default opc.tcp://0.0.0.0:4840/rcad/)
"""

import asyncio
import json
import os

import httpx
import websockets
from asyncua import Server, ua

RCAD_HTTP = os.environ.get("RCAD_HTTP", "http://127.0.0.1:3000")
RCAD_WS = os.environ.get("RCAD_WS", "ws://127.0.0.1:3000/api/telemetry/ws")
ENDPOINT = os.environ.get("OPCUA_ENDPOINT", "opc.tcp://0.0.0.0:4840/rcad/")


def _coerce(v):
    """Map a JSON tag value to a stable OPC UA-friendly Python type.

    Numbers are floated so a tag created as int never trips a type mismatch when
    it later arrives as a float; bools and strings pass through.
    """
    if isinstance(v, bool):
        return v
    if isinstance(v, (int, float)):
        return float(v)
    return str(v)


class IngressHandler:
    """Fires when an OPC UA client writes the TagsIn node -> rCAD ingest."""

    def __init__(self, client: httpx.AsyncClient):
        self._client = client

    def datachange_notification(self, node, val, data):  # noqa: D401
        if not val:
            return
        asyncio.create_task(self._push(val))

    async def _push(self, raw: str):
        try:
            tags = json.loads(raw)
            if not isinstance(tags, dict):
                return
        except (json.JSONDecodeError, TypeError):
            print(f"[ingress] ignoring non-JSON write: {raw!r}")
            return
        try:
            await self._client.post(
                f"{RCAD_HTTP}/api/telemetry/ingest", json={"tags": tags}, timeout=5
            )
            print(f"[ingress] -> rCAD {tags}")
        except httpx.HTTPError as e:
            print(f"[ingress] POST failed: {e}")


async def telemetry_loop(server, folder, idx, variables):
    """Mirror rCAD telemetry snapshots into OPC UA variables (egress)."""
    while True:
        try:
            async with websockets.connect(RCAD_WS) as ws:
                print(f"[egress] connected to {RCAD_WS}")
                async for msg in ws:
                    try:
                        snap = json.loads(msg)
                    except json.JSONDecodeError:
                        continue
                    for tag, value in (snap.get("tags") or {}).items():
                        v = _coerce(value)
                        node = variables.get(tag)
                        if node is None:
                            node = await folder.add_variable(idx, tag, v)
                            await node.set_read_only()
                            variables[tag] = node
                        else:
                            try:
                                await node.write_value(v)
                            except ua.UaError:
                                pass  # type flip on a tag; skip this tick
        except (OSError, websockets.WebSocketException) as e:
            print(f"[egress] rCAD WS down ({e}); retrying in 2s")
            await asyncio.sleep(2)


async def main():
    server = Server()
    await server.init()
    server.set_endpoint(ENDPOINT)
    server.set_server_name("rCAD OPC UA Bridge")
    idx = await server.register_namespace("http://rcad/twin")

    root = await server.nodes.objects.add_object(idx, "rCAD")
    telemetry = await root.add_folder(idx, "Telemetry")
    ingress = await root.add_object(idx, "Ingress")
    tags_in = await ingress.add_variable(
        idx, "TagsIn", "", varianttype=ua.VariantType.String
    )
    await tags_in.set_writable()

    variables: dict = {}

    async with httpx.AsyncClient() as client, server:
        print(f"[opcua] serving {ENDPOINT} (ns={idx})")
        sub = await server.create_subscription(200, IngressHandler(client))
        await sub.subscribe_data_change(tags_in)
        await telemetry_loop(server, telemetry, idx, variables)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
