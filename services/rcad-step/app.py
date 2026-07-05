"""OpenCASCADE STEP import service for rCAD.

The pure-Rust truck-stepio parser only covers a subset of STEP and chokes on
complex commercial AP242 assemblies. This sidecar uses OpenCASCADE (OCCT), the
reference STEP kernel, to read a STEP file into an XCAF document (preserving the
assembly structure, part names, and colours), tessellates it, and writes a
binary glTF (.glb). The rCAD server then runs that glb through its existing glTF
import pipeline, so complex STEP comes in as coloured parts with no new client
code.

`POST /convert` — raw STEP bytes in the request body → glb bytes back.
`GET  /health`  — liveness + OCCT version.
"""

import math
import os
import tempfile

from fastapi import FastAPI, Request, Response
from fastapi.responses import JSONResponse

from OCP.BRepBndLib import BRepBndLib
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.Bnd import Bnd_Box
from OCP.IFSelect import IFSelect_ReturnStatus
from OCP.Message import Message_ProgressRange
from OCP.RWGltf import RWGltf_CafWriter
from OCP.RWMesh import (
    RWMesh_CoordinateSystemConverter,
    RWMesh_CoordinateSystem_Yup,
    RWMesh_CoordinateSystem_Zup,
)
from OCP.STEPCAFControl import STEPCAFControl_Reader
from OCP.TCollection import TCollection_AsciiString, TCollection_ExtendedString
from OCP.TColStd import TColStd_IndexedDataMapOfStringString
from OCP.TDF import TDF_LabelSequence
from OCP.TDocStd import TDocStd_Document
from OCP.TopoDS import TopoDS_Builder, TopoDS_Compound
from OCP.XCAFApp import XCAFApp_Application
from OCP.XCAFDoc import XCAFDoc_DocumentTool

app = FastAPI(title="rCAD STEP (OpenCASCADE)")

# Relative tessellation quality: chord deflection = model diagonal * this,
# clamped so tiny parts still tessellate and huge assemblies stay reasonable.
DEFLECTION_REL = 0.0008
DEFLECTION_MIN = 0.02
ANGULAR_DEFLECTION = 0.5  # radians


@app.get("/health")
def health():
    return {"status": "ok", "kernel": "OpenCASCADE", "occt": _occt_version()}


def _occt_version():
    try:
        from OCP.Standard import Standard_Version

        return Standard_Version.Get_s()
    except Exception:  # noqa: BLE001
        return "unknown"


def step_to_glb(step_path: str, glb_path: str) -> dict:
    """Read a STEP file with OCCT and write a coloured binary glTF."""
    # XCAF document keeps assembly structure + colours + names.
    doc = TDocStd_Document(TCollection_ExtendedString("MDTV-XCAF"))
    XCAFApp_Application.GetApplication_s().InitDocument(doc)

    reader = STEPCAFControl_Reader()
    reader.SetColorMode(True)
    reader.SetNameMode(True)
    reader.SetLayerMode(True)
    if reader.ReadFile(step_path) != IFSelect_ReturnStatus.IFSelect_RetDone:
        raise ValueError("OCCT could not read the STEP file")
    if not reader.Transfer(doc):
        raise ValueError("OCCT could not transfer the STEP model")

    # Collect every free (top-level) shape into one compound to mesh at once.
    shape_tool = XCAFDoc_DocumentTool.ShapeTool_s(doc.Main())
    labels = TDF_LabelSequence()
    shape_tool.GetFreeShapes(labels)
    if labels.Length() == 0:
        raise ValueError("STEP file contained no shapes")

    builder = TopoDS_Builder()
    comp = TopoDS_Compound()
    builder.MakeCompound(comp)
    for i in range(1, labels.Length() + 1):
        builder.Add(comp, shape_tool.GetShape_s(labels.Value(i)))

    # Relative chord tolerance from the bounding-box diagonal.
    bbox = Bnd_Box()
    BRepBndLib.Add_s(comp, bbox, False)
    diag = 0.0
    if not bbox.IsVoid():
        xmin, ymin, zmin, xmax, ymax, zmax = bbox.Get()
        diag = math.sqrt((xmax - xmin) ** 2 + (ymax - ymin) ** 2 + (zmax - zmin) ** 2)
    deflection = max(diag * DEFLECTION_REL, DEFLECTION_MIN)

    BRepMesh_IncrementalMesh(comp, deflection, False, ANGULAR_DEFLECTION, True)

    # Write a binary glTF, converting OCCT's Z-up to glTF's Y-up (rCAD is Y-up).
    conv = RWMesh_CoordinateSystemConverter()
    conv.SetInputCoordinateSystem(RWMesh_CoordinateSystem_Zup)
    conv.SetOutputCoordinateSystem(RWMesh_CoordinateSystem_Yup)
    writer = RWGltf_CafWriter(TCollection_AsciiString(glb_path), True)  # binary .glb
    writer.SetCoordinateSystemConverter(conv)
    file_info = TColStd_IndexedDataMapOfStringString()
    if not writer.Perform(doc, file_info, Message_ProgressRange()):
        raise ValueError("OCCT failed to write glTF")

    return {"shapes": labels.Length(), "deflection": round(deflection, 4)}


@app.post("/convert")
async def convert(request: Request):
    data = await request.body()
    if not data:
        return JSONResponse({"ok": False, "error": "empty request body"}, status_code=400)
    with tempfile.TemporaryDirectory() as tmp:
        step_path = os.path.join(tmp, "in.step")
        glb_path = os.path.join(tmp, "out.glb")
        with open(step_path, "wb") as f:
            f.write(data)
        try:
            info = step_to_glb(step_path, glb_path)
        except Exception as e:  # noqa: BLE001
            return JSONResponse({"ok": False, "error": str(e)}, status_code=422)
        with open(glb_path, "rb") as f:
            glb = f.read()
    return Response(
        content=glb,
        media_type="model/gltf-binary",
        headers={
            "X-Step-Shapes": str(info["shapes"]),
            "X-Step-Deflection": str(info["deflection"]),
        },
    )
