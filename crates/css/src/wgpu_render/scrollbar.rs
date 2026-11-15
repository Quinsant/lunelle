use bytemuck::{Pod, Zeroable};
use lunelle_core::Rect;
use wgpu::util::DeviceExt;

use super::{
    WgpuCssRenderer,
    pipeline::{make_pipeline, set_scissor},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScrollbarVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub(super) fn create_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Lunelle CSS scrollbar shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    make_pipeline(
        device,
        format,
        &shader,
        &[],
        std::mem::size_of::<ScrollbarVertex>(),
        &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
    )
}

pub(super) fn render(
    renderer: &WgpuCssRenderer,
    device: &wgpu::Device,
    pass: &mut wgpu::RenderPass<'_>,
    bounds: &[Rect],
    clips: &[Option<Rect>],
    max_scroll: &[lunelle_core::Point],
    offsets: &[lunelle_core::Point],
    width: u32,
    height: u32,
) {
    let mut vertices = Vec::new();
    let mut ranges = Vec::new();
    for (index, bound) in bounds.iter().enumerate() {
        let maximum = max_scroll[index];
        if (maximum.x <= 0.0 && maximum.y <= 0.0) || bound.width <= 0.0 || bound.height <= 0.0 {
            continue;
        }
        let bar = 5.0_f32.min(bound.width);
        let start = vertices.len() as u32;
        if maximum.y > 0.0 {
            let thumb = (bound.height * bound.height / (bound.height + maximum.y))
                .max(18.0)
                .min(bound.height);
            let x = bound.x + bound.width - bar;
            let y = bound.y + (bound.height - thumb) * (offsets[index].y / maximum.y);
            vertices.extend(rect(
                x,
                bound.y,
                bar,
                bound.height,
                [0.24, 0.29, 0.35, 0.72],
                width,
                height,
            ));
            vertices.extend(rect(
                x,
                y,
                bar,
                thumb,
                [0.36, 0.72, 0.68, 0.95],
                width,
                height,
            ));
        }
        if maximum.x > 0.0 {
            let thumb = (bound.width * bound.width / (bound.width + maximum.x))
                .max(18.0)
                .min(bound.width);
            let x = bound.x + (bound.width - thumb) * (offsets[index].x / maximum.x);
            let y = bound.y + bound.height - bar;
            vertices.extend(rect(
                bound.x,
                y,
                bound.width,
                bar,
                [0.24, 0.29, 0.35, 0.72],
                width,
                height,
            ));
            vertices.extend(rect(
                x,
                y,
                thumb,
                bar,
                [0.36, 0.72, 0.68, 0.95],
                width,
                height,
            ));
        }
        ranges.push((
            start..vertices.len() as u32,
            clips.get(index).copied().flatten(),
        ));
    }
    if vertices.is_empty() {
        return;
    }
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Lunelle CSS scrollbars"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    pass.set_pipeline(&renderer.scrollbar_pipeline);
    pass.set_vertex_buffer(0, buffer.slice(..));
    for (range, clip) in ranges {
        if set_scissor(pass, clip, width, height) {
            pass.draw(range, 0..1);
        }
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32, color: [f32; 4], vw: u32, vh: u32) -> [ScrollbarVertex; 6] {
    let p = |x: f32, y: f32| [x / vw as f32 * 2.0 - 1.0, 1.0 - y / vh as f32 * 2.0];
    let (a, b, c, d) = (p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h));
    [
        ScrollbarVertex { position: a, color },
        ScrollbarVertex { position: b, color },
        ScrollbarVertex { position: c, color },
        ScrollbarVertex { position: a, color },
        ScrollbarVertex { position: c, color },
        ScrollbarVertex { position: d, color },
    ]
}

const SHADER: &str = r#"struct O{@builtin(position)p:vec4<f32>,@location(0)c:vec4<f32>} @vertex fn vertex_main(@location(0)p:vec2<f32>,@location(1)c:vec4<f32>)->O{var o:O;o.p=vec4<f32>(p,0.,1.);o.c=c;return o;} @fragment fn fragment_main(i:O)->@location(0)vec4<f32>{return i.c;}"#;
