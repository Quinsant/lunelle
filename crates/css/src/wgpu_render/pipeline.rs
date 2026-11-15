use bytemuck::{Pod, Zeroable};
use lunelle_core::Rect;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct RoundedVertex {
    pub position: [f32; 2],
    pub local: [f32; 2],
    pub background: [f32; 4],
    pub border: [f32; 4],
    pub shape: [f32; 4],
    pub radii: [f32; 4],
    pub gradient_index: u32,
    pub padding: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct ShadowVertex {
    pub position: [f32; 2],
    pub local: [f32; 2],
    pub color: [f32; 4],
    pub shape: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GradientHeader {
    pub parameters: [f32; 4],
    pub metadata: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GradientStop {
    pub color: [f32; 4],
    pub position: f32,
    pub padding: [f32; 3],
}

pub(super) fn make_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    stride: usize,
    attributes: &[wgpu::VertexAttribute],
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Lunelle CSS pipeline layout"),
        bind_group_layouts: layouts,
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Lunelle CSS pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: stride as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes,
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
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
    })
}

pub(super) fn set_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    clip: Option<Rect>,
    width: u32,
    height: u32,
) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    let Some(clip) = clip else {
        pass.set_scissor_rect(0, 0, width, height);
        return true;
    };
    let x = clip.x.floor().max(0.0) as u32;
    let y = clip.y.floor().max(0.0) as u32;
    let right = (clip.x + clip.width).ceil().clamp(0.0, width as f32) as u32;
    let bottom = (clip.y + clip.height).ceil().clamp(0.0, height as f32) as u32;
    let draw_width = right.saturating_sub(x).min(width);
    let draw_height = bottom.saturating_sub(y).min(height);
    if draw_width == 0 || draw_height == 0 {
        return false;
    }
    pass.set_scissor_rect(x.min(width), y.min(height), draw_width, draw_height);
    true
}
