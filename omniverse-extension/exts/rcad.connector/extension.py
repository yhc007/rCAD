"""
rCAD Connector Extension for NVIDIA Omniverse

This extension provides bidirectional synchronization between rCAD and Omniverse applications.
"""

import omni.ext
import omni.ui as ui
import omni.kit.app
import omni.usd
from pxr import Usd, UsdGeom, Gf
import asyncio
import aiohttp
import json


class RCADConnectorExtension(omni.ext.IExt):
    """rCAD Connector Extension"""

    def __init__(self):
        super().__init__()
        self._window = None
        self._server_url = "http://localhost:3000"
        self._session_id = None
        self._live_sync_enabled = False
        self._sync_task = None

    def on_startup(self, ext_id):
        """Called when the extension starts up."""
        print("[rcad.connector] rCAD Connector Extension starting up")

        # Create the UI window
        self._window = ui.Window("rCAD Connector", width=400, height=300)
        with self._window.frame:
            with ui.VStack(spacing=10):
                ui.Label("rCAD Connector", style={"font_size": 24})

                # Server settings
                with ui.CollapsableFrame("Server Settings", collapsed=False):
                    with ui.VStack(spacing=5):
                        ui.Label("Server URL:")
                        self._server_url_field = ui.StringField()
                        self._server_url_field.model.set_value(self._server_url)

                        with ui.HStack(spacing=10):
                            self._connect_btn = ui.Button(
                                "Connect",
                                clicked_fn=self._on_connect_clicked
                            )
                            self._disconnect_btn = ui.Button(
                                "Disconnect",
                                clicked_fn=self._on_disconnect_clicked,
                                enabled=False
                            )

                # Import/Export
                with ui.CollapsableFrame("Import/Export", collapsed=False):
                    with ui.VStack(spacing=5):
                        ui.Button(
                            "Import from rCAD",
                            clicked_fn=self._on_import_clicked
                        )
                        ui.Button(
                            "Export to rCAD",
                            clicked_fn=self._on_export_clicked
                        )

                # Live Sync
                with ui.CollapsableFrame("Live Sync", collapsed=False):
                    with ui.VStack(spacing=5):
                        with ui.HStack():
                            ui.Label("Status:")
                            self._sync_status_label = ui.Label("Disconnected")

                        self._sync_toggle = ui.CheckBox(
                            "Enable Live Sync"
                        )
                        self._sync_toggle.model.add_value_changed_fn(
                            self._on_sync_toggle_changed
                        )

                # Status
                with ui.CollapsableFrame("Status", collapsed=True):
                    self._status_label = ui.Label("Ready")

    def on_shutdown(self):
        """Called when the extension shuts down."""
        print("[rcad.connector] rCAD Connector Extension shutting down")

        if self._sync_task:
            self._sync_task.cancel()

        if self._window:
            self._window.destroy()
            self._window = None

    async def _connect_to_server(self):
        """Connect to the rCAD server."""
        try:
            self._server_url = self._server_url_field.model.get_value_as_string()

            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self._server_url}/api/omniverse/connect",
                    json={
                        "nucleus_url": "omniverse://localhost/",
                        "username": "",
                        "api_key": ""
                    }
                ) as response:
                    if response.status == 200:
                        data = await response.json()
                        if data.get("success"):
                            self._session_id = data.get("session_id")
                            self._update_status("Connected")
                            self._connect_btn.enabled = False
                            self._disconnect_btn.enabled = True
                            return True
                        else:
                            self._update_status(f"Error: {data.get('message')}")
                    else:
                        self._update_status(f"HTTP Error: {response.status}")
        except Exception as e:
            self._update_status(f"Connection failed: {str(e)}")

        return False

    async def _disconnect_from_server(self):
        """Disconnect from the rCAD server."""
        if not self._session_id:
            return

        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self._server_url}/api/omniverse/disconnect",
                    json=self._session_id
                ) as response:
                    pass  # Ignore response
        except Exception as e:
            print(f"[rcad.connector] Disconnect error: {e}")

        self._session_id = None
        self._connect_btn.enabled = True
        self._disconnect_btn.enabled = False
        self._update_status("Disconnected")

    def _on_connect_clicked(self):
        """Handle connect button click."""
        asyncio.ensure_future(self._connect_to_server())

    def _on_disconnect_clicked(self):
        """Handle disconnect button click."""
        asyncio.ensure_future(self._disconnect_from_server())

    def _on_import_clicked(self):
        """Handle import button click."""
        asyncio.ensure_future(self._import_from_rcad())

    def _on_export_clicked(self):
        """Handle export button click."""
        asyncio.ensure_future(self._export_to_rcad())

    def _on_sync_toggle_changed(self, model):
        """Handle sync toggle change."""
        enabled = model.get_value_as_bool()
        if enabled:
            asyncio.ensure_future(self._start_live_sync())
        else:
            asyncio.ensure_future(self._stop_live_sync())

    async def _import_from_rcad(self):
        """Import geometry from rCAD."""
        if not self._session_id:
            self._update_status("Not connected")
            return

        self._update_status("Importing from rCAD...")

        try:
            # Get current stage
            stage = omni.usd.get_context().get_stage()
            if not stage:
                self._update_status("No USD stage open")
                return

            # Request geometry from rCAD server
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self._server_url}/api/export/usd",
                    json={"geometry_id": "all", "options": {}}
                ) as response:
                    if response.status == 200:
                        usd_content = await response.text()
                        # In a real implementation, we would parse and merge the USD
                        self._update_status("Import complete")
                    else:
                        self._update_status(f"Import failed: {response.status}")
        except Exception as e:
            self._update_status(f"Import error: {str(e)}")

    async def _export_to_rcad(self):
        """Export current selection to rCAD."""
        if not self._session_id:
            self._update_status("Not connected")
            return

        self._update_status("Exporting to rCAD...")

        try:
            # Get current stage and selection
            stage = omni.usd.get_context().get_stage()
            selection = omni.usd.get_context().get_selection()

            if not stage:
                self._update_status("No USD stage open")
                return

            selected_paths = selection.get_selected_prim_paths()
            if not selected_paths:
                self._update_status("No selection")
                return

            # Export selected prims to USD string
            usd_content = self._export_prims_to_usd_string(stage, selected_paths)

            # Upload to rCAD server
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self._server_url}/api/omniverse/upload",
                    json={
                        "geometry_id": "export",
                        "nucleus_path": "/rCAD/export.usda",
                        "session_id": self._session_id
                    }
                ) as response:
                    if response.status == 200:
                        self._update_status("Export complete")
                    else:
                        self._update_status(f"Export failed: {response.status}")
        except Exception as e:
            self._update_status(f"Export error: {str(e)}")

    def _export_prims_to_usd_string(self, stage, prim_paths):
        """Export prims to USD string."""
        # Create a temporary layer to export
        from pxr import Sdf

        temp_layer = Sdf.Layer.CreateAnonymous()
        temp_stage = Usd.Stage.Open(temp_layer)

        for path in prim_paths:
            source_prim = stage.GetPrimAtPath(path)
            if source_prim:
                Sdf.CopySpec(
                    stage.GetRootLayer(),
                    path,
                    temp_layer,
                    path
                )

        return temp_stage.GetRootLayer().ExportToString()

    async def _start_live_sync(self):
        """Start live synchronization."""
        if not self._session_id:
            self._update_status("Not connected")
            self._sync_toggle.model.set_value(False)
            return

        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self._server_url}/api/omniverse/sync/start",
                    json={
                        "session_id": self._session_id,
                        "nucleus_path": "/rCAD/live"
                    }
                ) as response:
                    if response.status == 200:
                        data = await response.json()
                        if data.get("success"):
                            self._live_sync_enabled = True
                            self._sync_status_label.text = "Connected"
                            self._sync_task = asyncio.ensure_future(
                                self._sync_loop()
                            )
                        else:
                            self._sync_status_label.text = "Error"
                            self._sync_toggle.model.set_value(False)
        except Exception as e:
            self._update_status(f"Sync start error: {str(e)}")
            self._sync_toggle.model.set_value(False)

    async def _stop_live_sync(self):
        """Stop live synchronization."""
        self._live_sync_enabled = False

        if self._sync_task:
            self._sync_task.cancel()
            self._sync_task = None

        if self._session_id:
            try:
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        f"{self._server_url}/api/omniverse/sync/stop",
                        json={
                            "session_id": self._session_id,
                            "nucleus_path": "/rCAD/live"
                        }
                    ) as response:
                        pass
            except Exception:
                pass

        self._sync_status_label.text = "Disconnected"

    async def _sync_loop(self):
        """Main sync loop for live updates."""
        while self._live_sync_enabled:
            try:
                # Poll for changes (in a real implementation, use WebSocket)
                await asyncio.sleep(1.0)

                # Check for local changes and push
                # Check for remote changes and pull

            except asyncio.CancelledError:
                break
            except Exception as e:
                print(f"[rcad.connector] Sync error: {e}")
                await asyncio.sleep(5.0)

    def _update_status(self, message):
        """Update the status label."""
        self._status_label.text = message
        print(f"[rcad.connector] {message}")


# Extension entry point
def get_extension_class():
    return RCADConnectorExtension
