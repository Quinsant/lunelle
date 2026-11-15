use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use thiserror::Error;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::{DrawCommand, RenderFrame, Renderer, ScaleFactor};

#[derive(Debug, Error)]
pub enum WgpuRendererError {
    #[error("failed to create GPU surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error("no compatible GPU adapter was found")]
    NoAdapter,
    #[error("failed to create GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("surface has no supported formats or presentation modes")]
    UnsupportedSurface,
    #[error("failed to acquire surface texture: {0}")]
    Surface(String),
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

/// Minimal core-owned wgpu renderer for solid rectangles.
/// Construct and call it on the window/event-loop thread; the window is retained by the surface.
pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    scale_factor: f32,
}

impl WgpuRenderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, WgpuRendererError> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|_| WgpuRendererError::NoAdapter)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("lunelle-core device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            })
            .await?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(WgpuRendererError::UnsupportedSurface)?;
        let present_mode = capabilities
            .present_modes
            .first()
            .copied()
            .ok_or(WgpuRendererError::UnsupportedSurface)?;
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .ok_or(WgpuRendererError::UnsupportedSurface)?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lunelle-core rectangle shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vertex_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.color = color;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lunelle-core rectangle pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lunelle-core rectangle pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let scale_factor =
            ScaleFactor::new(window.scale_factor()).map_or(1.0, |scale| scale.get() as f32);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            scale_factor,
        })
    }

    pub fn set_scale_factor(&mut self, scale_factor: ScaleFactor) {
        self.scale_factor = scale_factor.get() as f32;
    }

    fn vertices(&self, frame: &RenderFrame) -> Vec<Vertex> {
        frame
            .commands
            .iter()
            .flat_map(|command| match command {
                DrawCommand::Rect { bounds, color } => {
                    let left = bounds.x * self.scale_factor / self.config.width as f32 * 2.0 - 1.0;
                    let right = (bounds.x + bounds.width) * self.scale_factor
                        / self.config.width as f32
                        * 2.0
                        - 1.0;
                    let top = 1.0 - bounds.y * self.scale_factor / self.config.height as f32 * 2.0;
                    let bottom = 1.0
                        - (bounds.y + bounds.height) * self.scale_factor
                            / self.config.height as f32
                            * 2.0;
                    let vertex = |position| Vertex {
                        position,
                        color: *color,
                    };
                    [
                        vertex([left, top]),
                        vertex([right, top]),
                        vertex([right, bottom]),
                        vertex([left, top]),
                        vertex([right, bottom]),
                        vertex([left, bottom]),
                    ]
                }
            })
            .collect()
    }
}

impl Renderer for WgpuRenderer {
    type Error = WgpuRendererError;

    fn render(&mut self, frame: &RenderFrame) -> Result<(), Self::Error> {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(texture)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                    status => return Err(WgpuRendererError::Surface(format!("{status:?}"))),
                }
            }
            status => return Err(WgpuRendererError::Surface(format!("{status:?}"))),
        };
        let vertices = self.vertices(frame);
        let vertex_buffer = (!vertices.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("lunelle-core rectangle vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lunelle-core render encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lunelle-core render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(vertex_buffer) = &vertex_buffer {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(texture);
        Ok(())
    }

    fn resize(&mut self, physical_size: (u32, u32)) {
        if physical_size.0 == 0 || physical_size.1 == 0 {
            return;
        }
        self.config.width = physical_size.0;
        self.config.height = physical_size.1;
        self.surface.configure(&self.device, &self.config);
    }
}
