import { useState, useCallback, useRef } from 'react';

interface RenderState {
  device: GPUDevice | null;
  context: GPUCanvasContext | null;
  pipeline: GPURenderPipeline | null;
}

export function useWebGPU() {
  const [isReady, setIsReady] = useState(false);
  const stateRef = useRef<RenderState>({
    device: null,
    context: null,
    pipeline: null,
  });
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  // Camera state
  const cameraRef = useRef({
    azimuth: Math.PI / 4,
    elevation: Math.PI / 6,
    distance: 200,
    target: { x: 0, y: 0, z: 0 },
  });

  const initialize = useCallback(async (canvas: HTMLCanvasElement) => {
    canvasRef.current = canvas;

    // Check WebGPU support
    if (!navigator.gpu) {
      console.error('WebGPU not supported');
      return;
    }

    try {
      // Get adapter
      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) {
        console.error('No GPU adapter found');
        return;
      }

      // Get device
      const device = await adapter.requestDevice();

      // Get context
      const context = canvas.getContext('webgpu');
      if (!context) {
        console.error('Failed to get WebGPU context');
        return;
      }

      // Configure context
      const format = navigator.gpu.getPreferredCanvasFormat();
      context.configure({
        device,
        format,
        alphaMode: 'premultiplied',
      });

      // Create basic shader
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
            // Simple lighting
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

      // Create pipeline
      const pipeline = device.createRenderPipeline({
        label: 'Render Pipeline',
        layout: 'auto',
        vertex: {
          module: shaderModule,
          entryPoint: 'vs_main',
          buffers: [
            {
              arrayStride: 24, // 3 floats position + 3 floats normal
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

      stateRef.current = { device, context, pipeline };
      setIsReady(true);
      console.log('WebGPU initialized');
    } catch (e) {
      console.error('WebGPU initialization failed:', e);
    }
  }, []);

  const resize = useCallback((width: number, height: number) => {
    // Resize handling is done through canvas resize
  }, []);

  const render = useCallback(() => {
    const { device, context, pipeline } = stateRef.current;
    if (!device || !context || !pipeline) return;

    const canvas = canvasRef.current;
    if (!canvas) return;

    // Create command encoder
    const commandEncoder = device.createCommandEncoder();

    // Get current texture
    const textureView = context.getCurrentTexture().createView();

    // Create depth texture (recreate each frame for simplicity)
    const depthTexture = device.createTexture({
      size: [canvas.width, canvas.height],
      format: 'depth24plus',
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });

    // Begin render pass
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

    // Set pipeline and draw (placeholder)
    renderPass.setPipeline(pipeline);
    // Would set vertex buffers, bind groups, and draw here

    renderPass.end();

    // Submit
    device.queue.submit([commandEncoder.finish()]);
  }, []);

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
    // Would need to compute right/up vectors from view matrix
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
  };
}
