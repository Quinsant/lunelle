use glyphon::{
    Attrs, Buffer, Color, Metrics, Shaping, Stretch, Style as FontStyle, TextArea, TextBounds,
    Weight as FontWeight,
};
use lunelle_core::Rect;

use super::{CssTextNode, WgpuCssRenderer};

impl WgpuCssRenderer {
    /// Rebuilds glyph buffers from text nodes and computed CSS values.
    pub fn set_text(
        &mut self,
        nodes: &[CssTextNode],
        styles: &[crate::ComputedStyle],
        bounds: &[Rect],
    ) {
        self.text_buffers = nodes
            .iter()
            .map(|node| {
                let presentation = crate::text_presentation(&styles[node.node]);
                let [red, green, blue, alpha] = presentation.color;
                let color = Color::rgba(red, green, blue, alpha);
                let transformed = match presentation.transform {
                    crate::TextTransform::None => node.text.clone(),
                    crate::TextTransform::Uppercase => node.text.to_uppercase(),
                    crate::TextTransform::Lowercase => node.text.to_lowercase(),
                    crate::TextTransform::Capitalize => capitalize_words(&node.text),
                };
                let mut attrs = Attrs::new().color(color);
                if presentation.italic {
                    attrs = attrs.style(FontStyle::Italic);
                }
                if let Some(weight) = presentation.weight {
                    attrs = attrs.weight(FontWeight(weight));
                }
                attrs = attrs.stretch(match presentation.stretch {
                    crate::FontStretch::Normal => Stretch::Normal,
                    crate::FontStretch::Condensed => Stretch::Condensed,
                    crate::FontStretch::Expanded => Stretch::Expanded,
                });
                let mut buffer = Buffer::new(
                    &mut self.font_system,
                    Metrics::new(presentation.font_size, presentation.line_height),
                );
                let align = match presentation.alignment {
                    crate::TextAlignment::Center => Some(glyphon::cosmic_text::Align::Center),
                    crate::TextAlignment::Right => Some(glyphon::cosmic_text::Align::Right),
                    crate::TextAlignment::Justified => Some(glyphon::cosmic_text::Align::Justified),
                    crate::TextAlignment::Left => Some(glyphon::cosmic_text::Align::Left),
                    crate::TextAlignment::Default => None,
                };
                buffer.set_size(
                    Some(bounds[node.node].width.max(1.0)),
                    Some(bounds[node.node].height.max(1.0)),
                );
                buffer.set_text(&transformed, &attrs, Shaping::Advanced, align);
                buffer.shape_until_scroll(&mut self.font_system, false);
                (buffer, color, node.node)
            })
            .collect();
    }

    /// Updates glyph buffers and prepares them for the target frame.
    pub fn prepare_text(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        styles: &[crate::ComputedStyle],
        bounds: &[Rect],
        clips: &[Option<Rect>],
        width: u32,
        height: u32,
    ) -> Result<(), glyphon::PrepareError> {
        self.viewport
            .update(queue, glyphon::Resolution { width, height });
        for (buffer, _, index) in &mut self.text_buffers {
            let bound = bounds[*index];
            buffer.set_size(Some(bound.width.max(1.0)), Some(bound.height.max(1.0)));
        }
        let areas = self
            .text_buffers
            .iter_mut()
            .filter(|(_, _, index)| {
                !styles[*index]
                    .properties
                    .get("display")
                    .is_some_and(|value| value.eq_ignore_ascii_case("none"))
                    && !styles[*index]
                        .properties
                        .get("visibility")
                        .is_some_and(|value| {
                            value.eq_ignore_ascii_case("hidden")
                                || value.eq_ignore_ascii_case("collapse")
                        })
            })
            .map(|(buffer, color, index)| {
                let bound = bounds[*index];
                let translation =
                    crate::text_translation(&styles[*index], bound.width, bound.height);
                let clip = clips[*index].map(|clip| intersect_clip(clip, bound));
                TextArea {
                    buffer,
                    left: bound.x + translation.0,
                    top: bound.y + translation.1,
                    scale: 1.0,
                    bounds: clip.map_or(
                        TextBounds {
                            left: 0,
                            top: 0,
                            right: width as i32,
                            bottom: height as i32,
                        },
                        |clip| TextBounds {
                            left: clip.x.floor() as i32,
                            top: clip.y.floor() as i32,
                            right: (clip.x + clip.width).ceil() as i32,
                            bottom: (clip.y + clip.height).ceil() as i32,
                        },
                    ),
                    default_color: *color,
                    custom_glyphs: &[],
                }
            });
        self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        )
    }

    /// Draws prepared CSS text into the current render pass.
    pub fn render_text(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), glyphon::RenderError> {
        self.text_renderer.render(&self.atlas, &self.viewport, pass)
    }

    /// Updates the text viewport after a native window resize.
    pub fn update_viewport(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.viewport
            .update(queue, glyphon::Resolution { width, height });
    }
}

fn capitalize_words(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut word_start = true;
    for character in value.chars() {
        if character.is_alphabetic() {
            if word_start {
                result.extend(character.to_uppercase());
                word_start = false;
            } else {
                result.extend(character.to_lowercase());
            }
        } else {
            word_start = true;
            result.push(character);
        }
    }
    result
}

fn intersect_clip(clip: Rect, bounds: Rect) -> Rect {
    let x = clip.x.max(bounds.x);
    let y = clip.y.max(bounds.y);
    let right = (clip.x + clip.width).min(bounds.x + bounds.width);
    let bottom = (clip.y + clip.height).min(bounds.y + bounds.height);
    Rect {
        x,
        y,
        width: (right - x).max(0.0),
        height: (bottom - y).max(0.0),
    }
}
