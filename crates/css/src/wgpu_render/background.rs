use bytemuck::Zeroable;
use lunelle_core::Rect;
use wgpu::util::DeviceExt;

use super::{
    WgpuCssRenderer,
    pipeline::{
        GradientHeader, GradientStop, RoundedVertex, ShadowVertex, make_pipeline, set_scissor,
    },
};

pub(super) fn create_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::BindGroupLayout,
) {
    let gradient_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Lunelle CSS gradient layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let background_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Lunelle CSS background shader"),
        source: wgpu::ShaderSource::Wgsl(BACKGROUND_SHADER.into()),
    });
    let background = make_pipeline(
        device,
        format,
        &background_shader,
        &[Some(&gradient_layout)],
        std::mem::size_of::<RoundedVertex>(),
        &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Uint32],
    );
    let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Lunelle CSS shadow shader"),
        source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
    });
    let shadow = make_pipeline(
        device,
        format,
        &shadow_shader,
        &[],
        std::mem::size_of::<ShadowVertex>(),
        &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4],
    );
    (background, shadow, gradient_layout)
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
    let (backgrounds, background_ranges, headers, stops) =
        background_vertices(styles, bounds, clips, width, height);
    let (shadows, shadow_ranges) = shadow_vertices(styles, bounds, clips, width, height);
    let background_buffer = (!backgrounds.is_empty()).then(|| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lunelle CSS backgrounds"),
            contents: bytemuck::cast_slice(&backgrounds),
            usage: wgpu::BufferUsages::VERTEX,
        })
    });
    let shadow_buffer = (!shadows.is_empty()).then(|| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lunelle CSS shadows"),
            contents: bytemuck::cast_slice(&shadows),
            usage: wgpu::BufferUsages::VERTEX,
        })
    });
    let header_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Lunelle CSS gradient headers"),
        contents: bytemuck::cast_slice(&headers),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let stop_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Lunelle CSS gradient stops"),
        contents: bytemuck::cast_slice(&stops),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Lunelle CSS gradient data"),
        layout: &renderer.gradient_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: header_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: stop_buffer.as_entire_binding(),
            },
        ],
    });
    pass.set_bind_group(0, &bind_group, &[]);
    let mut shadows = shadow_ranges.iter().peekable();
    let mut backgrounds = background_ranges.iter().peekable();
    for index in 0..styles.len() {
        while shadows
            .peek()
            .is_some_and(|(element, _, _)| *element < index)
        {
            shadows.next();
        }
        if let (Some(buffer), Some((_, range, clip))) = (
            &shadow_buffer,
            shadows.next_if(|(element, _, _)| *element == index),
        ) {
            pass.set_pipeline(&renderer.shadow_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            if set_scissor(pass, *clip, width, height) {
                pass.draw(range.clone(), 0..1);
            }
        }
        while backgrounds
            .peek()
            .is_some_and(|(element, _, _)| *element < index)
        {
            backgrounds.next();
        }
        if let (Some(buffer), Some((_, range, clip))) = (
            &background_buffer,
            backgrounds.next_if(|(element, _, _)| *element == index),
        ) {
            pass.set_pipeline(&renderer.background_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            if set_scissor(pass, *clip, width, height) {
                pass.draw(range.clone(), 0..1);
            }
        }
    }
}

fn background_vertices(
    styles: &[crate::ComputedStyle],
    bounds: &[Rect],
    clips: &[Option<Rect>],
    width: u32,
    height: u32,
) -> (
    Vec<RoundedVertex>,
    Vec<(usize, std::ops::Range<u32>, Option<Rect>)>,
    Vec<GradientHeader>,
    Vec<GradientStop>,
) {
    let mut vertices = Vec::new();
    let mut ranges = Vec::new();
    let mut headers = vec![GradientHeader::zeroed()];
    let mut stops = vec![GradientStop::zeroed()];
    for (index, (bound, style)) in bounds.iter().zip(styles).enumerate() {
        if style
            .properties
            .get("display")
            .is_some_and(|value| value.eq_ignore_ascii_case("none"))
            || style.properties.get("visibility").is_some_and(|value| {
                value.eq_ignore_ascii_case("hidden") || value.eq_ignore_ascii_case("collapse")
            })
        {
            continue;
        }
        let gradient = style
            .properties
            .get("background-image")
            .or_else(|| style.properties.get("background"))
            .and_then(|v| crate::parse_gradient(v));
        let background = style
            .properties
            .get("background-color")
            .and_then(|v| crate::parse_color(v))
            .or_else(|| {
                style
                    .properties
                    .get("background")
                    .and_then(|v| crate::shorthand_color(v))
            })
            .unwrap_or([0.0; 4]);
        let border_width = [
            crate::edge_length(style, "border-width"),
            crate::edge_length(style, "border-top-width"),
            crate::edge_length(style, "border-right-width"),
            crate::edge_length(style, "border-bottom-width"),
            crate::edge_length(style, "border-left-width"),
        ]
        .into_iter()
        .fold(0.0_f32, f32::max);
        let border = style
            .properties
            .get("border-color")
            .or_else(|| style.properties.get("border-top-color"))
            .or_else(|| style.properties.get("color"))
            .and_then(|v| crate::parse_color(v))
            .or_else(|| {
                style
                    .properties
                    .get("border")
                    .and_then(|v| crate::shorthand_color(v))
            });
        let hidden = matches!(
            style.properties.get("border-style").map(String::as_str),
            Some("none" | "hidden")
        ) || matches!(
            style.properties.get("border-top-style").map(String::as_str),
            Some("none" | "hidden")
        );
        if background[3] == 0.0 && gradient.is_none() && (border_width == 0.0 || hidden) {
            continue;
        }
        let opacity = crate::style_opacity(style);
        let mut border = border.unwrap_or([0.0, 0.0, 0.0, 1.0]);
        border[3] *= opacity;
        let radii = crate::border_radii(style, bound.width, bound.height);
        let matrix = style
            .properties
            .get("transform")
            .and_then(|v| crate::parse_transform(v))
            .map(|f| crate::Transform2D::from_functions(&f, bound.width, bound.height))
            .unwrap_or_default();
        let origin = style
            .properties
            .get("transform-origin")
            .and_then(|v| crate::parse_transform_origin(v, bound.width, bound.height))
            .unwrap_or((bound.width * 0.5, bound.height * 0.5));
        let matrix = matrix.around_origin(origin.0, origin.1);
        let point = |x: f32, y: f32| {
            let (x, y) = matrix.apply(x, y);
            [
                (bound.x + x) / width as f32 * 2.0 - 1.0,
                1.0 - (bound.y + y) / height as f32 * 2.0,
            ]
        };
        let half = [bound.width * 0.5, bound.height * 0.5];
        let shape = [
            bound.width,
            bound.height,
            radii[0],
            if hidden { 0.0 } else { border_width },
        ];
        let gradient_index = if let Some(g) = &gradient {
            let first = stops.len() as u32;
            headers.push(GradientHeader {
                parameters: [g.angle, g.center[0], g.center[1], 0.0],
                metadata: [
                    first,
                    g.stops.len() as u32,
                    u32::from(g.radial),
                    u32::from(g.repeating) | (u32::from(g.conic) << 1),
                ],
            });
            stops.extend(g.stops.iter().map(|(color, position)| GradientStop {
                color: *color,
                position: *position,
                padding: [0.0; 3],
            }));
            (headers.len() - 1) as u32
        } else {
            0
        };
        let mut color = background;
        color[3] *= opacity;
        let vertex = |position, local| RoundedVertex {
            position,
            local,
            background: color,
            border,
            shape,
            radii,
            gradient_index,
            padding: [0; 3],
        };
        let p = [
            point(0.0, 0.0),
            point(bound.width, 0.0),
            point(bound.width, bound.height),
            point(0.0, bound.height),
        ];
        let start = vertices.len() as u32;
        vertices.extend([
            vertex(p[0], [-half[0], -half[1]]),
            vertex(p[1], [half[0], -half[1]]),
            vertex(p[2], [half[0], half[1]]),
            vertex(p[0], [-half[0], -half[1]]),
            vertex(p[2], [half[0], half[1]]),
            vertex(p[3], [-half[0], half[1]]),
        ]);
        ranges.push((
            index,
            start..vertices.len() as u32,
            clips.get(index).copied().flatten(),
        ));
    }
    (vertices, ranges, headers, stops)
}

fn shadow_vertices(
    styles: &[crate::ComputedStyle],
    bounds: &[Rect],
    clips: &[Option<Rect>],
    width: u32,
    height: u32,
) -> (
    Vec<ShadowVertex>,
    Vec<(usize, std::ops::Range<u32>, Option<Rect>)>,
) {
    let mut vertices = Vec::new();
    let mut ranges = Vec::new();
    for (index, (bound, style)) in bounds.iter().zip(styles).enumerate() {
        let start = vertices.len() as u32;
        let Some(value) = style.properties.get("box-shadow") else {
            continue;
        };
        let radii = crate::border_radii(style, bound.width, bound.height);
        let opacity = crate::style_opacity(style);
        if style
            .properties
            .get("display")
            .is_some_and(|value| value.eq_ignore_ascii_case("none"))
            || style.properties.get("visibility").is_some_and(|value| {
                value.eq_ignore_ascii_case("hidden") || value.eq_ignore_ascii_case("collapse")
            })
        {
            continue;
        }
        for shadow in crate::parse_shadows(value) {
            let [ox, oy, blur, spread] = shadow.lengths;
            let w = (bound.width + spread * 2.0).max(0.0);
            let h = (bound.height + spread * 2.0).max(0.0);
            let extent = blur.max(0.0) * 1.5;
            let left = bound.x + ox - spread - extent;
            let top = bound.y + oy - spread - extent;
            let right = bound.x + ox + bound.width + spread + extent;
            let bottom = bound.y + oy + bound.height + spread + extent;
            let point =
                |x: f32, y: f32| [x / width as f32 * 2.0 - 1.0, 1.0 - y / height as f32 * 2.0];
            let mut color = shadow.color;
            color[3] *= opacity;
            let shape = [w, h, radii[0] + spread, blur.max(0.0)];
            let vertex = |x, y, lx, ly| ShadowVertex {
                position: point(x, y),
                local: [lx, ly],
                color,
                shape,
            };
            let (x0, y0, x1, y1) = (
                -w * 0.5 - extent,
                -h * 0.5 - extent,
                w * 0.5 + extent,
                h * 0.5 + extent,
            );
            vertices.extend([
                vertex(left, top, x0, y0),
                vertex(right, top, x1, y0),
                vertex(right, bottom, x1, y1),
                vertex(left, top, x0, y0),
                vertex(right, bottom, x1, y1),
                vertex(left, bottom, x0, y1),
            ]);
        }
        if vertices.len() as u32 > start {
            ranges.push((
                index,
                start..vertices.len() as u32,
                clips.get(index).copied().flatten(),
            ));
        }
    }
    (vertices, ranges)
}

const BACKGROUND_SHADER: &str = r#"struct O{@builtin(position)p:vec4<f32>,@location(0)l:vec2<f32>,@location(1)b:vec4<f32>,@location(2)c:vec4<f32>,@location(3)s:vec4<f32>,@location(4)r:vec4<f32>,@location(5)@interpolate(flat)g:u32} struct H{p:vec4<f32>,m:vec4<u32>} struct S{c:vec4<f32>,p:f32,p0:f32,p1:f32,p2:f32} @group(0)@binding(0)var<storage,read>hs:array<H>; @group(0)@binding(1)var<storage,read>ss:array<S>; @vertex fn vertex_main(@location(0)p:vec2<f32>,@location(1)l:vec2<f32>,@location(2)b:vec4<f32>,@location(3)c:vec4<f32>,@location(4)s:vec4<f32>,@location(5)r:vec4<f32>,@location(6)g:u32)->O{var o:O;o.p=vec4<f32>(p,0.,1.);o.l=l;o.b=b;o.c=c;o.s=s;o.r=r;o.g=g;return o;} fn d(p:vec2<f32>,h:vec2<f32>,r:vec4<f32>)->f32{let q=abs(p)-h+vec2<f32>(r.x);return length(max(q,vec2<f32>(0.)))+min(max(q.x,q.y),0.)-r.x;} @fragment fn fragment_main(i:O)->@location(0)vec4<f32>{let dist=d(i.l,i.s.xy*.5,i.r);let edge=1.-smoothstep(-.75,.75,dist);let ba=select(0.,1.,i.s.w>0.&&dist>=-i.s.w);var bg=i.b;let h=hs[i.g];if h.m.y>0u{let a=h.p.x;let dir=vec2<f32>(sin(a),-cos(a));let rel=i.l-(h.p.yz-vec2<f32>(.5))*i.s.xy;var x=dot(rel,dir)/max(i.s.x*abs(dir.x)+i.s.y*abs(dir.y),1.)+.5;if h.m.z==1u{x=length(rel)/max(max(i.s.x,i.s.y)*.5,1.);};x=clamp(x,0.,1.);var pc=ss[h.m.x].c;var ps=ss[h.m.x].p;var gc=pc;for(var n=1u;n<h.m.y;n++){let z=ss[h.m.x+n];if x<=z.p{gc=mix(pc,z.c,clamp((x-ps)/max(z.p-ps,.000001),0.,1.));break;}pc=z.c;ps=z.p;gc=z.c;}bg=gc;}let c=select(bg,i.c,ba>.5);return vec4<f32>(c.rgb,c.a*edge);}"#;
const SHADOW_SHADER: &str = r#"struct O{@builtin(position)p:vec4<f32>,@location(0)l:vec2<f32>,@location(1)c:vec4<f32>,@location(2)s:vec4<f32>} @vertex fn vertex_main(@location(0)p:vec2<f32>,@location(1)l:vec2<f32>,@location(2)c:vec4<f32>,@location(3)s:vec4<f32>)->O{var o:O;o.p=vec4<f32>(p,0.,1.);o.l=l;o.c=c;o.s=s;return o;} fn d(p:vec2<f32>,h:vec2<f32>,r:f32)->f32{let q=abs(p)-h+vec2<f32>(r);return length(max(q,vec2<f32>(0.)))+min(max(q.x,q.y),0.)-r;} @fragment fn fragment_main(i:O)->@location(0)vec4<f32>{let x=d(i.l,i.s.xy*.5,i.s.z);let a=1.-smoothstep(-max(i.s.w,.75)*.5,max(i.s.w,.75)*.5,x);return vec4<f32>(i.c.rgb,i.c.a*a);}"#;
