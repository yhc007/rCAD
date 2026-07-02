import { useState, useCallback, useRef } from 'react';
import type { MeshGeometry } from '../lib/meshParsers';
import { pickMesh, mat4Inverse, unproject } from '../lib/picking';

type RendererType = 'webgpu' | 'webgl' | null;

interface RenderState {
  type: RendererType;
  // WebGPU state
  device: GPUDevice | null;
  context: GPUCanvasContext | null;
  pipeline: GPURenderPipeline | null;
  // Two uniform buffers/bind groups: normal vs highlighted draw color.
  baseUniform: GPUBuffer | null;
  selUniform: GPUBuffer | null;
  bindGroupBase: GPUBindGroup | null;
  bindGroupSel: GPUBindGroup | null;
  depthTexture: GPUTexture | null;
  depthSize: string;
  // WebGL state
  gl: WebGLRenderingContext | WebGL2RenderingContext | null;
  glProgram: WebGLProgram | null;
}

// Mesh draw colors. BASE_COLOR is the fallback when a mesh has no per-feature
// colour; selection is applied as a brighten+tint in the shaders, not a flat
// override, so a selected mesh still shows its own colour.
const BASE_COLOR: [number, number, number] = [0.6, 0.6, 0.7];
const GRID_COLOR: [number, number, number] = [0.35, 0.35, 0.4];

interface GpuMesh {
  buffer: GPUBuffer; // interleaved pos(3) + normal(3)
  count: number;
}

interface GlMesh {
  position: WebGLBuffer;
  normal: WebGLBuffer;
  count: number;
}

const FOV = Math.PI / 4;

// Standard CAD view presets (Y-up). Top/bottom are nudged off the pole to keep
// the up vector well-defined (same clamp the orbit control uses).
const POLE = Math.PI / 2 - 0.01;
export type ViewPreset =
  | 'front' | 'back' | 'left' | 'right' | 'top' | 'bottom' | 'iso';
const VIEWS: Record<ViewPreset, { azimuth: number; elevation: number }> = {
  front: { azimuth: Math.PI / 2, elevation: 0 },
  back: { azimuth: -Math.PI / 2, elevation: 0 },
  right: { azimuth: 0, elevation: 0 },
  left: { azimuth: Math.PI, elevation: 0 },
  top: { azimuth: Math.PI / 2, elevation: POLE },
  bottom: { azimuth: Math.PI / 2, elevation: -POLE },
  iso: { azimuth: Math.PI / 4, elevation: Math.atan(1 / Math.SQRT2) },
};

export function useWebGPU() {
  const [isReady, setIsReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rendererType, setRendererType] = useState<RendererType>(null);
  const stateRef = useRef<RenderState>({
    type: null,
    device: null,
    context: null,
    pipeline: null,
    baseUniform: null,
    selUniform: null,
    bindGroupBase: null,
    bindGroupSel: null,
    depthTexture: null,
    depthSize: '',
    gl: null,
    glProgram: null,
  });
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  // Imported/renderable geometry and its GPU/GL resources.
  const meshGeomRef = useRef<MeshGeometry[]>([]);
  // Per-mesh base colour (linear RGB), aligned 1:1 with meshGeomRef.
  const colorsRef = useRef<[number, number, number][]>([]);
  const meshesDirtyRef = useRef(false);
  const gpuMeshesRef = useRef<GpuMesh[]>([]);
  const glMeshesRef = useRef<GlMesh[]>([]);
  // Indices (into meshGeomRef) of currently-selected meshes, drawn highlighted.
  const selectedSetRef = useRef<Set<number>>(new Set());

  // Camera state
  const cameraRef = useRef({
    azimuth: Math.PI / 4,
    elevation: Math.PI / 6,
    distance: 200,
    target: { x: 0, y: 0, z: 0 },
  });

  // Try to initialize WebGPU
  const initWebGPU = async (canvas: HTMLCanvasElement): Promise<boolean> => {
    if (!navigator.gpu) {
      console.log('WebGPU not supported, will try WebGL fallback');
      return false;
    }

    try {
      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) {
        console.log('No GPU adapter found, will try WebGL fallback');
        return false;
      }

      const device = await adapter.requestDevice();
      const context = canvas.getContext('webgpu');
      if (!context) {
        console.log('Failed to get WebGPU context, will try WebGL fallback');
        return false;
      }

      const format = navigator.gpu.getPreferredCanvasFormat();
      context.configure({
        device,
        format,
        alphaMode: 'premultiplied',
      });

      // Create WebGPU shader
      const shaderModule = device.createShaderModule({
        label: 'Basic Shader',
        code: `
          struct Uniforms {
            viewProj: mat4x4f,
            color: vec4f,
          }

          @group(0) @binding(0) var<uniform> uniforms: Uniforms;

          struct VertexOutput {
            @builtin(position) position: vec4f,
            @location(0) color: vec3f,
          }

          @vertex
          fn vs_main(
            @location(0) position: vec3f,
            @location(1) normal: vec3f,
            @location(2) color: vec3f,
          ) -> VertexOutput {
            var output: VertexOutput;
            output.position = uniforms.viewProj * vec4f(position, 1.0);
            let light = normalize(vec3f(1.0, 1.0, 1.0));
            // abs() so both faces are lit (imported meshes have arbitrary winding)
            let diffuse = abs(dot(normalize(normal), light));
            let lit = color * (0.3 + 0.7 * diffuse);
            // uniforms.color.a is the highlight flag: selected meshes keep their
            // own colour but are brightened and warm-tinted so selection reads.
            let hl = uniforms.color.a;
            output.color = lit * (1.0 + 0.35 * hl) + vec3f(0.18, 0.10, 0.0) * hl;
            return output;
          }

          @fragment
          fn fs_main(@location(0) color: vec3f) -> @location(0) vec4f {
            return vec4f(color, 1.0);
          }
        `,
      });

      const pipeline = device.createRenderPipeline({
        label: 'Render Pipeline',
        layout: 'auto',
        vertex: {
          module: shaderModule,
          entryPoint: 'vs_main',
          buffers: [
            {
              arrayStride: 36,
              attributes: [
                { shaderLocation: 0, offset: 0, format: 'float32x3' },
                { shaderLocation: 1, offset: 12, format: 'float32x3' },
                { shaderLocation: 2, offset: 24, format: 'float32x3' },
              ],
            },
          ],
        },
        fragment: {
          module: shaderModule,
          entryPoint: 'fs_main',
          targets: [{ format }],
        },
        primitive: {
          topology: 'triangle-list',
          // Imported meshes can have inconsistent winding, so draw double-sided.
          cullMode: 'none',
        },
        depthStencil: {
          format: 'depth24plus',
          depthWriteEnabled: true,
          depthCompare: 'less',
        },
      });

      // Uniform = viewProj (mat4, 64B) + params (vec4, 16B) = 80B. The mesh
      // colour now comes from a per-vertex attribute, so params.a is just the
      // highlight flag (0 = normal, 1 = selected); two buffers let both draw in
      // one pass.
      const makeUniform = (highlight: number) => {
        const buffer = device.createBuffer({
          label: 'Uniform',
          size: 80,
          usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });
        device.queue.writeBuffer(buffer, 64, new Float32Array([0, 0, 0, highlight]));
        const bindGroup = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(0),
          entries: [{ binding: 0, resource: { buffer } }],
        });
        return { buffer, bindGroup };
      };
      const base = makeUniform(0);
      const sel = makeUniform(1);

      stateRef.current = {
        ...stateRef.current,
        type: 'webgpu',
        device,
        context,
        pipeline,
        baseUniform: base.buffer,
        selUniform: sel.buffer,
        bindGroupBase: base.bindGroup,
        bindGroupSel: sel.bindGroup,
      };

      console.log('WebGPU initialized successfully');
      return true;
    } catch (e) {
      console.log('WebGPU initialization failed:', e);
      return false;
    }
  };

  // Initialize WebGL fallback
  const initWebGL = (canvas: HTMLCanvasElement): boolean => {
    try {
      const gl = canvas.getContext('webgl2') || canvas.getContext('webgl');
      if (!gl) {
        console.error('WebGL not supported');
        return false;
      }

      // Vertex shader
      const vsSource = `
        attribute vec3 aPosition;
        attribute vec3 aNormal;

        uniform mat4 uViewProj;
        uniform vec3 uColor;

        varying vec3 vColor;

        void main() {
          gl_Position = uViewProj * vec4(aPosition, 1.0);

          // Simple double-sided lighting
          vec3 light = normalize(vec3(1.0, 1.0, 1.0));
          float diffuse = abs(dot(normalize(aNormal), light));
          vColor = uColor * (0.3 + 0.7 * diffuse);
        }
      `;

      // Fragment shader
      const fsSource = `
        precision mediump float;
        varying vec3 vColor;

        void main() {
          gl_FragColor = vec4(vColor, 1.0);
        }
      `;

      // Compile shaders
      const vertexShader = compileShader(gl, gl.VERTEX_SHADER, vsSource);
      const fragmentShader = compileShader(gl, gl.FRAGMENT_SHADER, fsSource);

      if (!vertexShader || !fragmentShader) {
        return false;
      }

      // Create program
      const program = gl.createProgram();
      if (!program) {
        return false;
      }

      gl.attachShader(program, vertexShader);
      gl.attachShader(program, fragmentShader);
      gl.linkProgram(program);

      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        console.error('Program link error:', gl.getProgramInfoLog(program));
        return false;
      }

      // Setup GL state (double-sided: no back-face culling for imports)
      gl.enable(gl.DEPTH_TEST);

      stateRef.current = {
        ...stateRef.current,
        type: 'webgl',
        gl,
        glProgram: program,
      };

      console.log('WebGL initialized successfully');
      return true;
    } catch (e) {
      console.error('WebGL initialization failed:', e);
      return false;
    }
  };

  // Helper to compile WebGL shader
  const compileShader = (
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    type: number,
    source: string
  ): WebGLShader | null => {
    const shader = gl.createShader(type);
    if (!shader) return null;

    gl.shaderSource(shader, source);
    gl.compileShader(shader);

    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.error('Shader compile error:', gl.getShaderInfoLog(shader));
      gl.deleteShader(shader);
      return null;
    }

    return shader;
  };

  const initialize = useCallback(async (canvas: HTMLCanvasElement) => {
    canvasRef.current = canvas;

    // Try WebGPU first
    if (await initWebGPU(canvas)) {
      setRendererType('webgpu');
      setIsReady(true);
      return;
    }

    // Fall back to WebGL
    if (initWebGL(canvas)) {
      setRendererType('webgl');
      setIsReady(true);
      return;
    }

    // Neither worked
    setError('Neither WebGPU nor WebGL is available on this system.');
  }, []);

  const resize = useCallback((width: number, height: number) => {
    const { type, gl } = stateRef.current;
    if (type === 'webgl' && gl) {
      gl.viewport(0, 0, width * window.devicePixelRatio, height * window.devicePixelRatio);
    }
  }, []);

  // Frame the camera so the given meshes fit comfortably in view.
  const fitToMeshes = useCallback((meshes: MeshGeometry[]) => {
    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
    let any = false;

    for (const mesh of meshes) {
      const p = mesh.positions;
      for (let i = 0; i < p.length; i += 3) {
        any = true;
        if (p[i] < minX) minX = p[i];
        if (p[i] > maxX) maxX = p[i];
        if (p[i + 1] < minY) minY = p[i + 1];
        if (p[i + 1] > maxY) maxY = p[i + 1];
        if (p[i + 2] < minZ) minZ = p[i + 2];
        if (p[i + 2] > maxZ) maxZ = p[i + 2];
      }
    }
    if (!any) return;

    const cx = (minX + maxX) / 2;
    const cy = (minY + maxY) / 2;
    const cz = (minZ + maxZ) / 2;
    const radius = 0.5 * Math.hypot(maxX - minX, maxY - minY, maxZ - minZ);

    const camera = cameraRef.current;
    camera.target = { x: cx, y: cy, z: cz };
    const dist = (radius || 1) / Math.tan(FOV / 2) * 1.3;
    camera.distance = Math.max(1, Math.min(10000, dist));
  }, []);

  /**
   * Replace the set of renderable meshes. `fit` re-frames the camera (default);
   * pass false for per-frame updates during physics playback.
   */
  const setMeshes = useCallback(
    (meshes: MeshGeometry[], fit = true, colors?: [number, number, number][]) => {
      meshGeomRef.current = meshes;
      // Colours given → use them; omitted but count matches (e.g. playback
      // re-pushing transformed meshes) → keep existing; otherwise default.
      if (colors) colorsRef.current = colors;
      else if (colorsRef.current.length !== meshes.length)
        colorsRef.current = meshes.map(() => BASE_COLOR);
      meshesDirtyRef.current = true;
      if (fit) fitToMeshes(meshes);
    },
    [fitToMeshes]
  );

  /** Snap the camera to a standard view, keeping the current target/distance. */
  const setView = useCallback((preset: ViewPreset) => {
    const v = VIEWS[preset];
    cameraRef.current.azimuth = v.azimuth;
    cameraRef.current.elevation = v.elevation;
  }, []);

  /** Re-frame the camera around all current geometry. */
  const fitToContent = useCallback(() => {
    fitToMeshes(meshGeomRef.current);
  }, [fitToMeshes]);

  // Build interleaved pos+normal+color data for one mesh (WebGPU vertex buffer).
  const interleave = (mesh: MeshGeometry, color: [number, number, number]): Float32Array => {
    const n = mesh.vertexCount;
    const [cr, cg, cb] = color;
    const out = new Float32Array(n * 9);
    for (let i = 0; i < n; i++) {
      out[i * 9] = mesh.positions[i * 3];
      out[i * 9 + 1] = mesh.positions[i * 3 + 1];
      out[i * 9 + 2] = mesh.positions[i * 3 + 2];
      out[i * 9 + 3] = mesh.normals[i * 3];
      out[i * 9 + 4] = mesh.normals[i * 3 + 1];
      out[i * 9 + 5] = mesh.normals[i * 3 + 2];
      out[i * 9 + 6] = cr;
      out[i * 9 + 7] = cg;
      out[i * 9 + 8] = cb;
    }
    return out;
  };

  // Note: meshes are NOT filtered, so buffer indices stay aligned with
  // meshGeomRef / the selection set. Empty meshes just draw zero vertices.
  const rebuildGpuMeshes = (device: GPUDevice) => {
    gpuMeshesRef.current.forEach((m) => m.buffer.destroy());
    gpuMeshesRef.current = meshGeomRef.current.map((m, i) => {
      const data = interleave(m, colorsRef.current[i] ?? BASE_COLOR);
      const buffer = device.createBuffer({
        size: Math.max(data.byteLength, 4),
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      if (data.byteLength) device.queue.writeBuffer(buffer, 0, data as BufferSource);
      return { buffer, count: m.vertexCount };
    });
  };

  const rebuildGlMeshes = (gl: WebGLRenderingContext | WebGL2RenderingContext) => {
    glMeshesRef.current.forEach((m) => {
      gl.deleteBuffer(m.position);
      gl.deleteBuffer(m.normal);
    });
    glMeshesRef.current = meshGeomRef.current.map((m) => {
      const position = gl.createBuffer()!;
      gl.bindBuffer(gl.ARRAY_BUFFER, position);
      gl.bufferData(gl.ARRAY_BUFFER, m.positions, gl.STATIC_DRAW);
      const normal = gl.createBuffer()!;
      gl.bindBuffer(gl.ARRAY_BUFFER, normal);
      gl.bufferData(gl.ARRAY_BUFFER, m.normals, gl.STATIC_DRAW);
      return { position, normal, count: m.vertexCount };
    });
  };

  // Compute view-projection matrix
  const computeViewProjMatrix = useCallback((): Float32Array => {
    const camera = cameraRef.current;
    const canvas = canvasRef.current;
    const aspect = canvas ? canvas.width / canvas.height : 1;

    // Camera position from spherical coordinates
    const cosElev = Math.cos(camera.elevation);
    const eyeX = camera.target.x + camera.distance * cosElev * Math.cos(camera.azimuth);
    const eyeY = camera.target.y + camera.distance * Math.sin(camera.elevation);
    const eyeZ = camera.target.z + camera.distance * cosElev * Math.sin(camera.azimuth);

    // View matrix (lookAt)
    const view = lookAt(
      [eyeX, eyeY, eyeZ],
      [camera.target.x, camera.target.y, camera.target.z],
      [0, 1, 0]
    );

    // Projection matrix (perspective)
    const proj = perspective(FOV, aspect, 0.1, 100000);

    // Multiply projection * view
    return multiplyMatrices(proj, view);
  }, []);

  /**
   * Ray-pick the mesh under a normalized device coordinate (x,y in [-1,1]).
   * Returns the mesh's index in the current mesh list, or null for empty space.
   */
  const pick = useCallback(
    (ndcX: number, ndcY: number): number | null =>
      pickMesh(meshGeomRef.current, computeViewProjMatrix(), ndcX, ndcY),
    [computeViewProjMatrix]
  );

  /** Mark which mesh indices are selected (drawn highlighted). */
  const setSelection = useCallback((indices: number[]) => {
    selectedSetRef.current = new Set(indices);
  }, []);

  /** Project a world point to canvas CSS pixels, or null if behind the camera. */
  const project = useCallback(
    (p: readonly [number, number, number]): [number, number] | null => {
      const canvas = canvasRef.current;
      if (!canvas) return null;
      const m = computeViewProjMatrix();
      const [x, y, z] = p;
      const cx = m[0] * x + m[4] * y + m[8] * z + m[12];
      const cy = m[1] * x + m[5] * y + m[9] * z + m[13];
      const cw = m[3] * x + m[7] * y + m[11] * z + m[15];
      if (cw <= 1e-6) return null;
      const ndcX = cx / cw;
      const ndcY = cy / cw;
      return [
        (ndcX * 0.5 + 0.5) * canvas.clientWidth,
        (0.5 - ndcY * 0.5) * canvas.clientHeight,
      ];
    },
    [computeViewProjMatrix]
  );

  /** World-space ray (origin + unit dir) through a canvas CSS-pixel point. */
  const unprojectRay = useCallback(
    (cssX: number, cssY: number): { origin: number[]; dir: number[] } | null => {
      const canvas = canvasRef.current;
      if (!canvas) return null;
      const inv = mat4Inverse(computeViewProjMatrix());
      if (!inv) return null;
      const ndcX = (cssX / canvas.clientWidth) * 2 - 1;
      const ndcY = 1 - (cssY / canvas.clientHeight) * 2;
      const a = unproject(inv, ndcX, ndcY, -1);
      const b = unproject(inv, ndcX, ndcY, 1);
      let dx = b[0] - a[0], dy = b[1] - a[1], dz = b[2] - a[2];
      const len = Math.hypot(dx, dy, dz) || 1;
      return { origin: a, dir: [dx / len, dy / len, dz / len] };
    },
    [computeViewProjMatrix]
  );

  const render = useCallback(() => {
    const st = stateRef.current;
    const canvas = canvasRef.current;
    if (!canvas) return;

    const viewProj = computeViewProjMatrix();

    if (
      st.type === 'webgpu' && st.device && st.context && st.pipeline &&
      st.baseUniform && st.selUniform && st.bindGroupBase && st.bindGroupSel
    ) {
      const { device, context, pipeline } = st;

      if (meshesDirtyRef.current) {
        rebuildGpuMeshes(device);
        meshesDirtyRef.current = false;
      }

      // Both color uniforms share the current view-projection.
      device.queue.writeBuffer(st.baseUniform, 0, viewProj as BufferSource);
      device.queue.writeBuffer(st.selUniform, 0, viewProj as BufferSource);

      // (Re)create depth texture to match the current canvas size.
      const sizeKey = `${canvas.width}x${canvas.height}`;
      if (!st.depthTexture || st.depthSize !== sizeKey) {
        st.depthTexture?.destroy();
        st.depthTexture = device.createTexture({
          size: [canvas.width, canvas.height],
          format: 'depth24plus',
          usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });
        st.depthSize = sizeKey;
      }

      const commandEncoder = device.createCommandEncoder();
      const renderPass = commandEncoder.beginRenderPass({
        colorAttachments: [
          {
            view: context.getCurrentTexture().createView(),
            clearValue: { r: 0.1, g: 0.1, b: 0.12, a: 1.0 },
            loadOp: 'clear',
            storeOp: 'store',
          },
        ],
        depthStencilAttachment: {
          view: st.depthTexture.createView(),
          depthClearValue: 1.0,
          depthLoadOp: 'clear',
          depthStoreOp: 'store',
        },
      });

      renderPass.setPipeline(pipeline);
      const sel = selectedSetRef.current;
      // Draw unselected meshes, then selected ones (with the highlight uniform).
      renderPass.setBindGroup(0, st.bindGroupBase);
      gpuMeshesRef.current.forEach((mesh, i) => {
        if (mesh.count === 0 || sel.has(i)) return;
        renderPass.setVertexBuffer(0, mesh.buffer);
        renderPass.draw(mesh.count);
      });
      renderPass.setBindGroup(0, st.bindGroupSel);
      gpuMeshesRef.current.forEach((mesh, i) => {
        if (mesh.count === 0 || !sel.has(i)) return;
        renderPass.setVertexBuffer(0, mesh.buffer);
        renderPass.draw(mesh.count);
      });
      renderPass.end();

      device.queue.submit([commandEncoder.finish()]);
    } else if (st.type === 'webgl' && st.gl && st.glProgram) {
      const { gl, glProgram } = st;

      gl.clearColor(0.1, 0.1, 0.12, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

      gl.useProgram(glProgram);

      // Set view-projection matrix
      const uViewProj = gl.getUniformLocation(glProgram, 'uViewProj');
      gl.uniformMatrix4fv(uViewProj, false, viewProj);
      const uColor = gl.getUniformLocation(glProgram, 'uColor');

      // Grid for spatial reference
      gl.uniform3fv(uColor, GRID_COLOR);
      drawGrid(gl, glProgram);

      if (meshesDirtyRef.current) {
        rebuildGlMeshes(gl);
        meshesDirtyRef.current = false;
      }

      // Draw imported/renderable meshes, highlighting the selected ones.
      const sel = selectedSetRef.current;
      const aPosition = gl.getAttribLocation(glProgram, 'aPosition');
      const aNormal = gl.getAttribLocation(glProgram, 'aNormal');
      glMeshesRef.current.forEach((mesh, i) => {
        if (mesh.count === 0) return;
        const c = colorsRef.current[i] ?? BASE_COLOR;
        // Match the WebGPU highlight: brighten + warm-tint the selected mesh's
        // own colour rather than replacing it.
        const col = sel.has(i)
          ? [Math.min(1, c[0] * 1.35 + 0.18), Math.min(1, c[1] * 1.35 + 0.1), Math.min(1, c[2] * 1.35)]
          : c;
        gl.uniform3fv(uColor, col);

        gl.bindBuffer(gl.ARRAY_BUFFER, mesh.position);
        gl.enableVertexAttribArray(aPosition);
        gl.vertexAttribPointer(aPosition, 3, gl.FLOAT, false, 0, 0);

        gl.bindBuffer(gl.ARRAY_BUFFER, mesh.normal);
        gl.enableVertexAttribArray(aNormal);
        gl.vertexAttribPointer(aNormal, 3, gl.FLOAT, false, 0, 0);

        gl.drawArrays(gl.TRIANGLES, 0, mesh.count);
      });
    }
  }, [computeViewProjMatrix]);

  // Draw a simple grid
  const drawGrid = (
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    program: WebGLProgram
  ) => {
    const gridSize = 100;
    const gridStep = 10;
    const vertices: number[] = [];

    // Create grid lines
    for (let i = -gridSize; i <= gridSize; i += gridStep) {
      // X-parallel lines
      vertices.push(-gridSize, 0, i, gridSize, 0, i);
      // Z-parallel lines
      vertices.push(i, 0, -gridSize, i, 0, gridSize);
    }

    const vertexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.STATIC_DRAW);

    const aPosition = gl.getAttribLocation(program, 'aPosition');
    gl.enableVertexAttribArray(aPosition);
    gl.vertexAttribPointer(aPosition, 3, gl.FLOAT, false, 0, 0);

    // Use constant normal for grid
    const aNormal = gl.getAttribLocation(program, 'aNormal');
    gl.disableVertexAttribArray(aNormal);
    gl.vertexAttrib3f(aNormal, 0, 1, 0);

    gl.drawArrays(gl.LINES, 0, vertices.length / 3);

    gl.deleteBuffer(vertexBuffer);
  };

  const orbit = useCallback((deltaAzimuth: number, deltaElevation: number) => {
    const camera = cameraRef.current;
    camera.azimuth += deltaAzimuth;
    camera.elevation = Math.max(
      -Math.PI / 2 + 0.01,
      Math.min(Math.PI / 2 - 0.01, camera.elevation + deltaElevation)
    );
  }, []);

  const pan = useCallback((deltaX: number, deltaY: number) => {
    const camera = cameraRef.current;
    const panSpeed = camera.distance * 0.001;
    camera.target.x += deltaX * panSpeed;
    camera.target.y += deltaY * panSpeed;
  }, []);

  const zoom = useCallback((delta: number) => {
    const camera = cameraRef.current;
    camera.distance *= 1 + delta;
    camera.distance = Math.max(1, Math.min(100000, camera.distance));
  }, []);

  return {
    initialize,
    render,
    resize,
    orbit,
    pan,
    zoom,
    setMeshes,
    setView,
    fitToContent,
    pick,
    setSelection,
    project,
    unprojectRay,
    isReady,
    error,
    rendererType,
  };
}

// Matrix math utilities
function lookAt(eye: number[], target: number[], up: number[]): Float32Array {
  const zAxis = normalize(subtract(eye, target));
  const xAxis = normalize(cross(up, zAxis));
  const yAxis = cross(zAxis, xAxis);

  return new Float32Array([
    xAxis[0], yAxis[0], zAxis[0], 0,
    xAxis[1], yAxis[1], zAxis[1], 0,
    xAxis[2], yAxis[2], zAxis[2], 0,
    -dot(xAxis, eye), -dot(yAxis, eye), -dot(zAxis, eye), 1,
  ]);
}

function perspective(fov: number, aspect: number, near: number, far: number): Float32Array {
  const f = 1.0 / Math.tan(fov / 2);
  const rangeInv = 1 / (near - far);

  return new Float32Array([
    f / aspect, 0, 0, 0,
    0, f, 0, 0,
    0, 0, (near + far) * rangeInv, -1,
    0, 0, near * far * rangeInv * 2, 0,
  ]);
}

// Column-major matrix product (out = a · b), matching the column-major layout
// the WGSL/GLSL shaders expect for `matrix * vector`.
function multiplyMatrices(a: Float32Array, b: Float32Array): Float32Array {
  const out = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    const b0 = b[c * 4], b1 = b[c * 4 + 1], b2 = b[c * 4 + 2], b3 = b[c * 4 + 3];
    out[c * 4 + 0] = a[0] * b0 + a[4] * b1 + a[8] * b2 + a[12] * b3;
    out[c * 4 + 1] = a[1] * b0 + a[5] * b1 + a[9] * b2 + a[13] * b3;
    out[c * 4 + 2] = a[2] * b0 + a[6] * b1 + a[10] * b2 + a[14] * b3;
    out[c * 4 + 3] = a[3] * b0 + a[7] * b1 + a[11] * b2 + a[15] * b3;
  }
  return out;
}

function subtract(a: number[], b: number[]): number[] {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

function cross(a: number[], b: number[]): number[] {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}

function dot(a: number[], b: number[]): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

function normalize(v: number[]): number[] {
  const len = Math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
  if (len === 0) return [0, 0, 0];
  return [v[0] / len, v[1] / len, v[2] / len];
}
