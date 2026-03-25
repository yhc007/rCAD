import { useState, useCallback, useRef } from 'react';

type RendererType = 'webgpu' | 'webgl' | null;

interface RenderState {
  type: RendererType;
  // WebGPU state
  device: GPUDevice | null;
  context: GPUCanvasContext | null;
  pipeline: GPURenderPipeline | null;
  // WebGL state
  gl: WebGLRenderingContext | WebGL2RenderingContext | null;
  glProgram: WebGLProgram | null;
}

export function useWebGPU() {
  const [isReady, setIsReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rendererType, setRendererType] = useState<RendererType>(null);
  const stateRef = useRef<RenderState>({
    type: null,
    device: null,
    context: null,
    pipeline: null,
    gl: null,
    glProgram: null,
  });
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

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
          }

          @group(0) @binding(0) var<uniform> uniforms: Uniforms;

          struct VertexOutput {
            @builtin(position) position: vec4f,
            @location(0) color: vec3f,
          }

          @vertex
          fn vs_main(@location(0) position: vec3f, @location(1) normal: vec3f) -> VertexOutput {
            var output: VertexOutput;
            output.position = uniforms.viewProj * vec4f(position, 1.0);
            let light = normalize(vec3f(1.0, 1.0, 1.0));
            let diffuse = max(dot(normal, light), 0.0);
            output.color = vec3f(0.6, 0.6, 0.7) * (0.3 + 0.7 * diffuse);
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
              arrayStride: 24,
              attributes: [
                { shaderLocation: 0, offset: 0, format: 'float32x3' },
                { shaderLocation: 1, offset: 12, format: 'float32x3' },
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
          cullMode: 'back',
        },
        depthStencil: {
          format: 'depth24plus',
          depthWriteEnabled: true,
          depthCompare: 'less',
        },
      });

      stateRef.current = {
        ...stateRef.current,
        type: 'webgpu',
        device,
        context,
        pipeline,
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

        varying vec3 vColor;

        void main() {
          gl_Position = uViewProj * vec4(aPosition, 1.0);

          // Simple lighting
          vec3 light = normalize(vec3(1.0, 1.0, 1.0));
          float diffuse = max(dot(aNormal, light), 0.0);
          vColor = vec3(0.6, 0.6, 0.7) * (0.3 + 0.7 * diffuse);
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

      // Setup GL state
      gl.enable(gl.DEPTH_TEST);
      gl.enable(gl.CULL_FACE);
      gl.cullFace(gl.BACK);

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
    const proj = perspective(Math.PI / 4, aspect, 0.1, 10000);

    // Multiply projection * view
    return multiplyMatrices(proj, view);
  }, []);

  const render = useCallback(() => {
    const { type, device, context, pipeline, gl, glProgram } = stateRef.current;
    const canvas = canvasRef.current;
    if (!canvas) return;

    if (type === 'webgpu' && device && context && pipeline) {
      // WebGPU render
      const commandEncoder = device.createCommandEncoder();
      const textureView = context.getCurrentTexture().createView();

      const depthTexture = device.createTexture({
        size: [canvas.width, canvas.height],
        format: 'depth24plus',
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });

      const renderPass = commandEncoder.beginRenderPass({
        colorAttachments: [
          {
            view: textureView,
            clearValue: { r: 0.1, g: 0.1, b: 0.12, a: 1.0 },
            loadOp: 'clear',
            storeOp: 'store',
          },
        ],
        depthStencilAttachment: {
          view: depthTexture.createView(),
          depthClearValue: 1.0,
          depthLoadOp: 'clear',
          depthStoreOp: 'store',
        },
      });

      renderPass.setPipeline(pipeline);
      renderPass.end();

      device.queue.submit([commandEncoder.finish()]);
    } else if (type === 'webgl' && gl && glProgram) {
      // WebGL render
      gl.clearColor(0.1, 0.1, 0.12, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

      gl.useProgram(glProgram);

      // Set view-projection matrix
      const viewProj = computeViewProjMatrix();
      const uViewProj = gl.getUniformLocation(glProgram, 'uViewProj');
      gl.uniformMatrix4fv(uViewProj, false, viewProj);

      // Draw grid (simple lines for now)
      drawGrid(gl, glProgram);
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
    const normals: number[] = [];

    // Create grid lines
    for (let i = -gridSize; i <= gridSize; i += gridStep) {
      // X-parallel lines
      vertices.push(-gridSize, 0, i, gridSize, 0, i);
      normals.push(0, 1, 0, 0, 1, 0);
      // Z-parallel lines
      vertices.push(i, 0, -gridSize, i, 0, gridSize);
      normals.push(0, 1, 0, 0, 1, 0);
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
    camera.distance = Math.max(1, Math.min(10000, camera.distance));
  }, []);

  return {
    initialize,
    render,
    resize,
    orbit,
    pan,
    zoom,
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

function multiplyMatrices(a: Float32Array, b: Float32Array): Float32Array {
  const result = new Float32Array(16);
  for (let i = 0; i < 4; i++) {
    for (let j = 0; j < 4; j++) {
      result[i * 4 + j] =
        a[i * 4 + 0] * b[0 * 4 + j] +
        a[i * 4 + 1] * b[1 * 4 + j] +
        a[i * 4 + 2] * b[2 * 4 + j] +
        a[i * 4 + 3] * b[3 * 4 + j];
    }
  }
  return result;
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
