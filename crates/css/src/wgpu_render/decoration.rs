use bytemuck::{Pod, Zeroable};
use lunelle_core::Rect;
use wgpu::util::DeviceExt;

use super::{
    WgpuCssRenderer,
    pipeline::{make_pipeline, set_scissor},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DecorationVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub(super) fn create_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Lunelle CSS text decoration shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    make_pipeline(
        device,
        format,
        &shader,
        &[],
        std::mem::size_of::<DecorationVertex>(),
        &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
    )
}

pub(super) fn render(
    renderer: &WgpuCssRenderer,
    device: &wgpu::Device,
    pass: &mut wgpu::RenderPass<'_>,
    styles: &[crate::ComputedStyle],
    bounds: &[Rect],
    clips: &[Option<Rect>],
    width: u32,
    height: u32,
) {
    if width == 0 || height == 0 {
        return;
    }
    let mut vertices = Vec::new();
    let mut ranges = Vec::new();
    for (index, (style, bound)) in styles.iter().zip(bounds).enumerate() {
        let presentation = crate::text_presentation(style);
        let decoration = presentation.decoration;
        if (!decoration.underline && !decoration.overline && !decoration.line_through)
            || style
                .properties
                .get("display")
                .is_some_and(|value| value.eq_ignore_ascii_case("none"))
            || style.properties.get("visibility").is_some_and(|value| {
                value.eq_ignore_ascii_case("hidden") || value.eq_ignore_ascii_case("collapse")
            })
        {
            continue;
        }
        let Some((buffer, _, _)) = renderer
            .text_buffers
            .iter()
            .find(|(_, _, node)| *node == index)
        else {
            continue;
        };
        let color = presentation
            .decoration
            .color
            .map(|value| value as f32 / 255.0);
        let color = [color[0], color[1], color[2], color[3]];
        let start = vertices.len() as u32;
        let thickness = match presentation.decoration.thickness {
            crate::TextDecorationThickness::Auto => (presentation.font_size / 16.0).max(1.0),
            crate::TextDecorationThickness::Px(value) => value.max(1.0),
        };
        for run in buffer.layout_runs() {
            let Some(first) = run.glyphs.first() else {
                continue;
            };
            let left = run
                .glyphs
                .iter()
                .map(|glyph| glyph.x)
                .fold(first.x, f32::min)
                + bound.x;
            let right = run
                .glyphs
                .iter()
                .map(|glyph| glyph.x + glyph.w)
                .fold(first.x + first.w, f32::max)
                + bound.x;
            let text_width = (right - left).max(0.0);
            let line_top = bound.y + run.line_top;
            if decoration.overline {
                vertices.extend(styled_line(
                    left,
                    line_top,
                    text_width,
                    thickness,
                    presentation.decoration.style,
                    color,
                    width,
                    height,
                ));
            }
            if decoration.line_through {
                vertices.extend(styled_line(
                    left,
                    line_top + run.line_height * 0.5,
                    text_width,
                    thickness,
                    presentation.decoration.style,
                    color,
                    width,
                    height,
                ));
            }
            if decoration.underline {
                vertices.extend(styled_line(
                    left,
                    line_top + run.line_height - thickness,
                    text_width,
                    thickness,
                    presentation.decoration.style,
                    color,
                    width,
                    height,
                ));
            }
        }
        if vertices.len() as u32 > start {
            ranges.push((
                index,
                start..vertices.len() as u32,
                clips.get(index).copied().flatten(),
            ));
        }
    }
    if vertices.is_empty() {
        return;
    }
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Lunelle CSS text decorations"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    pass.set_pipeline(&renderer.decoration_pipeline);
    pass.set_vertex_buffer(0, buffer.slice(..));
    for (_, range, clip) in ranges {
        if set_scissor(pass, clip, width, height) {
            pass.draw(range, 0..1);
        }
    }
}

fn styled_line(
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    style: crate::TextDecorationStyle,
    color: [f32; 4],
    viewport_width: u32,
    viewport_height: u32,
) -> Vec<DecorationVertex> {
    match style {
        crate::TextDecorationStyle::Solid => rect(
            x,
            y,
            width,
            thickness,
            color,
            viewport_width,
            viewport_height,
        )
        .into(),
        crate::TextDecorationStyle::Double => {
            let stroke = (thickness / 3.0).max(1.0);
            let mut vertices =
                rect(x, y, width, stroke, color, viewport_width, viewport_height).to_vec();
            vertices.extend(rect(
                x,
                y + thickness - stroke,
                width,
                stroke,
                color,
                viewport_width,
                viewport_height,
            ));
            vertices
        }
        crate::TextDecorationStyle::Dashed => patterned_line(
            x,
            y,
            width,
            thickness,
            thickness * 3.0,
            thickness * 2.0,
            color,
            viewport_width,
            viewport_height,
        ),
        crate::TextDecorationStyle::Dotted => patterned_line(
            x,
            y,
            width,
            thickness,
            thickness,
            thickness,
            color,
            viewport_width,
            viewport_height,
        ),
        crate::TextDecorationStyle::Wavy => wavy_line(
            x,
            y,
            width,
            thickness,
            color,
            viewport_width,
            viewport_height,
        ),
    }
}

fn wavy_line(
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    color: [f32; 4],
    viewport_width: u32,
    viewport_height: u32,
) -> Vec<DecorationVertex> {
    if width <= 0.0 {
        return Vec::new();
    }
    let wavelength = (thickness * 4.0).max(4.0);
    let amplitude = (thickness * 1.5).max(1.0);
    let samples = ((width / 2.0).ceil() as usize).max(2);
    let step = width / (samples - 1) as f32;
    let mut vertices = Vec::with_capacity((samples - 1) * 6);
    for sample in 0..samples - 1 {
        let x0 = sample as f32 * step;
        let x1 = (sample + 1) as f32 * step;
        let y0 = y + amplitude * (x0 / wavelength * std::f32::consts::TAU).sin();
        let y1 = y + amplitude * (x1 / wavelength * std::f32::consts::TAU).sin();
        vertices.extend(segment(
            x + x0,
            y0,
            x + x1,
            y1,
            thickness,
            color,
            viewport_width,
            viewport_height,
        ));
    }
    vertices
}

fn segment(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    thickness: f32,
    color: [f32; 4],
    viewport_width: u32,
    viewport_height: u32,
) -> [DecorationVertex; 6] {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let length = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
    let half = thickness * 0.5;
    let nx = -dy / length * half;
    let ny = dx / length * half;
    let point = |x: f32, y: f32| {
        [
            x / viewport_width as f32 * 2.0 - 1.0,
            1.0 - y / viewport_height as f32 * 2.0,
        ]
    };
    let a = point(x0 + nx, y0 + ny);
    let b = point(x1 + nx, y1 + ny);
    let c = point(x1 - nx, y1 - ny);
    let d = point(x0 - nx, y0 - ny);
    [
        DecorationVertex { position: a, color },
        DecorationVertex { position: b, color },
        DecorationVertex { position: c, color },
        DecorationVertex { position: a, color },
        DecorationVertex { position: c, color },
        DecorationVertex { position: d, color },
    ]
}

fn patterned_line(
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    dash: f32,
    gap: f32,
    color: [f32; 4],
    viewport_width: u32,
    viewport_height: u32,
) -> Vec<DecorationVertex> {
    let mut vertices = Vec::new();
    let mut cursor = 0.0;
    while cursor < width {
        let segment = dash.min(width - cursor);
        vertices.extend(rect(
            x + cursor,
            y,
            segment,
            thickness,
            color,
            viewport_width,
            viewport_height,
        ));
        cursor += dash + gap;
    }
    vertices
}

fn rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
    viewport_width: u32,
    viewport_height: u32,
) -> [DecorationVertex; 6] {
    let point = |x: f32, y: f32| {
        [
            x / viewport_width as f32 * 2.0 - 1.0,
            1.0 - y / viewport_height as f32 * 2.0,
        ]
    };
    let a = point(x, y);
    let b = point(x + width, y);
    let c = point(x + width, y + height);
    let d = point(x, y + height);
    [
        DecorationVertex { position: a, color },
        DecorationVertex { position: b, color },
        DecorationVertex { position: c, color },
        DecorationVertex { position: a, color },
        DecorationVertex { position: c, color },
        DecorationVertex { position: d, color },
    ]
}

const SHADER: &str = r#"struct O{@builtin(position)p:vec4<f32>,@location(0)c:vec4<f32>} @vertex fn vertex_main(@location(0)p:vec2<f32>,@location(1)c:vec4<f32>)->O{var o:O;o.p=vec4<f32>(p,0.,1.);o.c=c;return o;} @fragment fn fragment_main(i:O)->@location(0)vec4<f32>{return i.c;}"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn wavy_decoration_renders_visible_sine_geometry() {
        pollster::block_on(async {
            let instance = wgpu::Instance::default();
            let Ok(adapter) = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
            else {
                return;
            };
            let Ok((device, queue)) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("CSS decoration test device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    ..Default::default()
                })
                .await
            else {
                return;
            };

            let width = 128u32;
            let height = 48u32;
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("CSS decoration test target"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let pipeline = create_pipeline(&device, format);
            let vertices = wavy_line(8.0, 24.0, 112.0, 2.0, [1.0, 0.0, 0.0, 1.0], width, height);
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("CSS decoration test vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let bytes_per_row = width * 4;
            let padded_bytes_per_row = bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("CSS decoration test readback"),
                size: (padded_bytes_per_row * height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("CSS decoration test pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let slice = readback.slice(..);
            let (sender, receiver) = mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).unwrap()
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                })
                .unwrap();
            receiver.recv().unwrap().unwrap();
            let data = slice.get_mapped_range().unwrap();
            let rows = (0..height)
                .filter(|row| {
                    (0..width).any(|column| {
                        data[(*row * padded_bytes_per_row + column * 4 + 3) as usize] > 0
                    })
                })
                .count();
            assert!(
                rows >= 4,
                "wavy decoration did not produce visible rasterized geometry"
            );
            drop(data);
            readback.unmap();
        });
    }
}
