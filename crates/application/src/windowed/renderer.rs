use super::{
    aom_css::CssScene,
    gpu::GpuContext,
    interaction::{interaction_states, interaction_states_changed},
};
use lunelle_aom::{Element, UiAction};
use lunelle_core::{MouseButton, Point, Runtime, Size};
use lunelle_css::{
    ComputedStyleTree, CssRender, StyleSheet, StyleState, WgpuContext, WgpuCssRenderer,
};
use std::{
    collections::BTreeMap,
    error::Error,
    sync::{Arc, mpsc},
    time::Instant,
};
use winit::window::Window;

pub(super) struct Renderer {
    gpu: GpuContext,
    scene: CssScene,
    css: Option<WgpuCssRenderer>,
    css_receiver: Option<mpsc::Receiver<Result<WgpuCssRenderer, String>>>,
    css_thread: Option<std::thread::JoinHandle<()>>,
    _runtime: Arc<Runtime>,
    element: Box<dyn Element>,
    pointer: Option<Point>,
    pressed: bool,
    hovered_target: Option<String>,
    pressed_target: Option<(String, MouseButton)>,
    interaction: BTreeMap<lunelle_core::NodeId, StyleState>,
    interaction_pending: bool,
    text_dirty: bool,
    timeline: CssRender,
    base_styles: Vec<lunelle_css::ComputedStyle>,
    started: Instant,
    timeline_active: bool,
}

impl Renderer {
    pub(super) async fn new(
        window: Arc<Window>,
        element: &dyn Element,
        stylesheets: &[StyleSheet],
        runtime: Arc<Runtime>,
    ) -> Result<Self, Box<dyn Error>> {
        let gpu = GpuContext::new(window).await?;
        let mut stylesheet = StyleSheet::default();
        for sheet in stylesheets {
            let offset = stylesheet
                .rules
                .iter()
                .map(|rule| rule.source_order)
                .max()
                .map_or(0, |order| order + 1);
            stylesheet
                .rules
                .extend(sheet.rules.iter().cloned().map(|mut rule| {
                    rule.source_order += offset;
                    rule
                }));
            stylesheet.keyframes.extend(sheet.keyframes.clone());
            stylesheet.diagnostics.extend(sheet.diagnostics.clone());
        }
        let timeline = CssRender::new(stylesheet);
        let scene = CssScene::from_aom(
            element,
            &timeline,
            gpu.config.width,
            gpu.config.height,
            gpu.window.scale_factor() as f32,
        );
        let base_styles = scene.styles.clone();
        let mut renderer = Self {
            gpu,
            scene,
            css: None,
            css_receiver: None,
            css_thread: None,
            _runtime: runtime,
            element: element.clone_box(),
            pointer: None,
            pressed: false,
            hovered_target: None,
            pressed_target: None,
            interaction: BTreeMap::new(),
            interaction_pending: false,
            text_dirty: true,
            timeline,
            base_styles,
            started: Instant::now(),
            timeline_active: true,
        };
        renderer.advance()?;
        Ok(renderer)
    }

    pub(super) fn shutdown(&mut self) {
        if let Some(thread) = self.css_thread.take() {
            let _ = thread.join();
        }
        self.css_receiver = None;
        self.css = None;
    }

    pub(super) fn request_redraw(&self) {
        self.gpu.window.request_redraw();
    }
    pub(super) fn start_css_initialization(&mut self) {
        if self.css.is_some() || self.css_receiver.is_some() {
            return;
        }
        let device = self.gpu.device.clone();
        let queue = self.gpu.queue.clone();
        let format = self.gpu.config.format;
        let window = self.gpu.window.clone();
        let (sender, receiver) = mpsc::channel();
        self.css_thread = Some(std::thread::spawn(move || {
            let result = Ok(WgpuCssRenderer::new(WgpuContext {
                device: &device,
                queue: &queue,
                format,
            }));
            let _ = sender.send(result);
            window.request_redraw();
        }));
        self.css_receiver = Some(receiver);
    }
    pub(super) fn poll_css_initialization(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(receiver) = &self.css_receiver else {
            return Ok(());
        };
        match receiver.try_recv() {
            Ok(Ok(mut css)) => {
                css.set_text(&self.scene.text, &self.scene.styles, &self.scene.bounds);
                self.css = Some(css);
                self.css_receiver = None;
                self.request_redraw();
            }
            Ok(Err(error)) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, error).into());
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("CSS renderer initialization thread stopped unexpectedly".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        Ok(())
    }
    pub(super) fn advance(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.timeline_active {
            return Ok(false);
        }
        let now = self.started.elapsed().as_secs_f32() * 1000.0;
        let tree = ComputedStyleTree {
            snapshot_version: self.scene._snapshot.version,
            stylesheet_version: 0,
            environment_version: 0,
            styles: self.base_styles.clone(),
            diagnostics: Vec::new(),
        };
        let (tree, active) = self.timeline.update(tree, now);
        self.timeline_active = active;
        let text_changed = self.timeline.text_changed(&self.scene.styles, &tree.styles);
        let layout_changed = self.scene.apply_styles(
            self.element.as_ref(),
            &self.timeline,
            tree.styles,
            self.gpu.config.width,
            self.gpu.config.height,
        );
        if layout_changed || text_changed {
            self.text_dirty = true;
            if let Some(css) = self.css.as_mut() {
                css.set_text(&self.scene.text, &self.scene.styles, &self.scene.bounds);
            }
        }
        self.request_redraw();
        Ok(active)
    }
    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.gpu.config.width = width;
        self.gpu.config.height = height;
        self.gpu
            .surface
            .configure(&self.gpu.device, &self.gpu.config);
        self.rebuild_scene(width, height);
        self.request_redraw();
    }
    pub(super) fn resize_logical(&mut self, size: Size) {
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }
        let scale = self.gpu.window.scale_factor() as f32;
        self.resize(
            (size.width * scale).round().max(1.0) as u32,
            (size.height * scale).round().max(1.0) as u32,
        );
    }
    pub(super) fn scale_factor_changed(&mut self) {
        let size = self.gpu.window.inner_size();
        self.resize(size.width, size.height);
    }
    pub(super) fn pointer_moved(&mut self, point: Point) -> Vec<UiAction> {
        let scale = self.gpu.window.scale_factor() as f32;
        self.pointer = Some(Point {
            x: point.x * scale,
            y: point.y * scale,
        });
        self.interaction_pending = true;
        self.hover_action()
    }
    pub(super) fn scroll(&mut self, delta: Point) {
        let Some(point) = self.pointer else { return };
        if lunelle_css::scroll_at_point(
            point,
            Point {
                x: delta.x,
                y: delta.y,
            },
            1.0,
            &self.scene.bounds,
            &self.scene.clips,
            &self.scene.states,
            &self.scene.max_scroll,
            &mut self.scene.scroll_offsets,
        )
        .is_some()
        {
            self.scene.apply_overflow();
            self.interaction_pending = true;
            self.text_dirty = true;
            self.request_redraw();
        }
    }
    pub(super) fn pointer_left(&mut self) -> Vec<UiAction> {
        self.pointer = None;
        self.interaction_pending = true;
        self.hover_action()
    }
    pub(super) fn pointer_button(
        &mut self,
        pressed: bool,
        button: MouseButton,
    ) -> Option<UiAction> {
        self.pressed = pressed;
        self.interaction_pending = true;
        self.flush_interaction();
        let target = self.hit_target();
        if pressed {
            self.pressed_target = target.map(|target| (target, button));
            None
        } else {
            let clicked =
                self.pressed_target
                    .take()
                    .is_some_and(|(pressed_target, pressed_button)| {
                        pressed_button == button
                            && target.as_deref() == Some(pressed_target.as_str())
                    });
            clicked.then(|| UiAction::Click { target, button })
        }
    }

    pub(super) fn set_element(&mut self, element: &dyn Element) {
        self.element = element.clone_box();
        if self
            .hovered_target
            .as_deref()
            .is_some_and(|target| !contains_element_id(self.element.as_ref(), target))
        {
            self.hovered_target = None;
        }
        if self
            .pressed_target
            .as_ref()
            .is_some_and(|(target, _)| !contains_element_id(self.element.as_ref(), target))
        {
            self.pressed_target = None;
        }
        self.rebuild_scene(self.gpu.config.width, self.gpu.config.height);
        self.request_redraw();
    }

    fn hover_action(&mut self) -> Vec<UiAction> {
        let target = self.hit_target();
        if target == self.hovered_target {
            return Vec::new();
        }
        let mut actions = Vec::with_capacity(2);
        if let Some(previous) = std::mem::replace(&mut self.hovered_target, target.clone()) {
            actions.push(UiAction::Hover {
                target: Some(previous),
                entered: false,
                x: self.pointer.map_or(0.0, |point| point.x),
                y: self.pointer.map_or(0.0, |point| point.y),
            });
        }
        if let Some(target) = target {
            actions.push(UiAction::Hover {
                target: Some(target),
                entered: true,
                x: self.pointer.map_or(0.0, |point| point.x),
                y: self.pointer.map_or(0.0, |point| point.y),
            });
        }
        actions
    }

    fn hit_target(&self) -> Option<String> {
        let point = self.pointer?;
        let index = self
            .scene
            .bounds
            .iter()
            .enumerate()
            .rev()
            .find(|(index, bound)| {
                bound.x <= point.x
                    && point.x < bound.x + bound.width
                    && bound.y <= point.y
                    && point.y < bound.y + bound.height
                    && self.scene.clips[*index].is_none_or(|clip| {
                        clip.x <= point.x
                            && point.x < clip.x + clip.width
                            && clip.y <= point.y
                            && point.y < clip.y + clip.height
                    })
            })
            .map(|(index, _)| index)?;
        let nodes = &self.scene._snapshot.nodes;
        let mut node = &nodes[index];
        loop {
            if let Some(id) = &node.element_id {
                return Some(id.clone());
            }
            let parent = node.parent?;
            node = nodes.iter().find(|candidate| candidate.id == parent)?;
        }
    }
    pub(super) fn flush_interaction(&mut self) {
        if std::mem::take(&mut self.interaction_pending) {
            self.rebuild_interaction();
        }
    }
    fn rebuild_interaction(&mut self) {
        if !interaction_states_changed(&self.scene, self.pointer, self.pressed, &self.interaction) {
            return;
        }
        let states = interaction_states(&self.scene, self.pointer, self.pressed);
        self.rebuild_scene_with_states(states);
        self.request_redraw();
    }
    fn rebuild_scene(&mut self, width: u32, height: u32) {
        self.rebuild_scene_with_states_at(self.interaction.clone(), width, height);
        self.rebuild_interaction();
    }
    fn rebuild_scene_with_states(
        &mut self,
        states: BTreeMap<lunelle_core::NodeId, StyleState>,
    ) {
        self.rebuild_scene_with_states_at(states, self.gpu.config.width, self.gpu.config.height);
    }
    fn rebuild_scene_with_states_at(
        &mut self,
        states: BTreeMap<lunelle_core::NodeId, StyleState>,
        width: u32,
        height: u32,
    ) {
        self.interaction = states.clone();
        let mut scene = CssScene::from_aom_with_states(
            self.element.as_ref(),
            &self.timeline,
            width,
            height,
            self.gpu.window.scale_factor() as f32,
            states,
        );
        let now = self.started.elapsed().as_secs_f32() * 1000.0;
        self.timeline.retarget_transitions(
            &self.base_styles,
            &self.scene.styles,
            &scene.styles,
            now,
        );
        self.base_styles = scene.styles.clone();
        self.timeline_active = true;
        // Preserve scroll positions when the rebuilt scene has the same node count.
        if scene.scroll_offsets.len() == self.scene.scroll_offsets.len() {
            scene
                .scroll_offsets
                .copy_from_slice(&self.scene.scroll_offsets);
            scene.apply_overflow();
        }
        self.scene = scene;
        self.text_dirty = true;
        if let Some(css) = self.css.as_mut() {
            css.set_text(&self.scene.text, &self.scene.styles, &self.scene.bounds);
        }
    }
    pub(super) fn render(&mut self) -> Result<(), Box<dyn Error>> {
        let frame = match self.gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.gpu
                    .surface
                    .configure(&self.gpu.device, &self.gpu.config);
                self.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            status => return Err(format!("failed to acquire surface texture: {status:?}").into()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        if self.css.is_none() {
            let mut encoder = self
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Lunelle initial frame"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
            self.gpu.queue.submit([encoder.finish()]);
            self.gpu.queue.present(frame);
            return Ok(());
        }
        let css = self.css.as_mut().expect("CSS renderer initialized");
        if self.text_dirty {
            css.prepare_text(
                &self.gpu.device,
                &self.gpu.queue,
                &self.scene.styles,
                &self.scene.bounds,
                &self.scene.clips,
                self.gpu.config.width,
                self.gpu.config.height,
            )?;
            self.text_dirty = false;
        }
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Lunelle CSS frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            css.render(
                &self.gpu.device,
                &mut pass,
                &self.scene.styles,
                &self.scene.bounds,
                &self.scene.clips,
                self.gpu.config.width,
                self.gpu.config.height,
            );
            css.render_scrollbars(
                &self.gpu.device,
                &mut pass,
                &self.scene.bounds,
                &self.scene.clips,
                &self.scene.max_scroll,
                &self.scene.scroll_offsets,
                self.gpu.config.width,
                self.gpu.config.height,
            );
            pass.set_scissor_rect(0, 0, self.gpu.config.width, self.gpu.config.height);
            css.render_text(&mut pass)?;
            css.render_text_decorations(
                &self.gpu.device,
                &mut pass,
                &self.scene.styles,
                &self.scene.bounds,
                &self.scene.clips,
                self.gpu.config.width,
                self.gpu.config.height,
            );
        }
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.queue.present(frame);
        Ok(())
    }
}

fn contains_element_id(element: &dyn Element, target: &str) -> bool {
    if element.element_id() == Some(target) {
        return true;
    }
    element
        .children()
        .iter()
        .any(|child| contains_element_id(child.as_ref(), target))
}
