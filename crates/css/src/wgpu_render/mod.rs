mod background;
mod decoration;
mod pipeline;
mod scrollbar;
mod text;

use glyphon::{Buffer, Color, FontSystem, SwashCache, TextAtlas, TextRenderer, Viewport};

/// WGPU pipelines and text resources used to render CSS output.
pub struct WgpuCssRenderer {
    pub(super) background_pipeline: wgpu::RenderPipeline,
    pub(super) shadow_pipeline: wgpu::RenderPipeline,
    pub(super) scrollbar_pipeline: wgpu::RenderPipeline,
    pub(super) decoration_pipeline: wgpu::RenderPipeline,
    pub(super) gradient_layout: wgpu::BindGroupLayout,
    pub(super) font_system: FontSystem,
    pub(super) swash_cache: SwashCache,
    pub(super) atlas: TextAtlas,
    pub(super) text_renderer: TextRenderer,
    pub(super) viewport: Viewport,
    pub(super) text_buffers: Vec<(Buffer, Color, usize)>,
}

/// GPU resources supplied by the host application to the CSS renderer.
pub struct WgpuContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub format: wgpu::TextureFormat,
}

/// Text content and its computed-style node index for CSS text rendering.
pub struct CssTextNode {
    /// Text content.
    pub text: String,
    /// Index into the computed style and layout arrays.
    pub node: usize,
}

impl WgpuCssRenderer {
    /// Builds renderer pipelines for the target texture format.
    pub fn new(context: WgpuContext<'_>) -> Self {
        let device = context.device;
        let queue = context.queue;
        let format = context.format;
        let (background_pipeline, shadow_pipeline, gradient_layout) =
            background::create_pipelines(device, format);
        let scrollbar_pipeline = scrollbar::create_pipeline(device, format);
        let decoration_pipeline = decoration::create_pipeline(device, format);
        let cache = glyphon::Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        Self {
            background_pipeline,
            shadow_pipeline,
            scrollbar_pipeline,
            decoration_pipeline,
            gradient_layout,
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            atlas,
            text_renderer,
            viewport: Viewport::new(device, &cache),
            text_buffers: Vec::new(),
        }
    }

    /// Encodes CSS background and shadow draws for computed styles.
    pub fn render(
        &self,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'_>,
        styles: &[crate::ComputedStyle],
        bounds: &[lunelle_core::Rect],
        clips: &[Option<lunelle_core::Rect>],
        width: u32,
        height: u32,
    ) {
        background::render(self, device, pass, styles, bounds, clips, width, height);
    }

    /// Encodes scroll track and thumb draws for CSS scrollable boxes.
    pub fn render_scrollbars(
        &self,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'_>,
        bounds: &[lunelle_core::Rect],
        clips: &[Option<lunelle_core::Rect>],
        max_scroll: &[lunelle_core::Point],
        scroll_offsets: &[lunelle_core::Point],
        width: u32,
        height: u32,
    ) {
        scrollbar::render(
            self,
            device,
            pass,
            bounds,
            clips,
            max_scroll,
            scroll_offsets,
            width,
            height,
        );
    }

    /// Encodes CSS text-decoration lines after glyph rendering.
    pub fn render_text_decorations(
        &self,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'_>,
        styles: &[crate::ComputedStyle],
        bounds: &[lunelle_core::Rect],
        clips: &[Option<lunelle_core::Rect>],
        width: u32,
        height: u32,
    ) {
        decoration::render(self, device, pass, styles, bounds, clips, width, height);
    }
}
