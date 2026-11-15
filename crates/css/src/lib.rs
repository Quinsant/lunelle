//! CSS parsing, cascade, computed-style conversion, and rendering support.
//!
//! The crate is the public entry point for the CSS pipeline: [`CssRender`] loads
//! stylesheets, computes styles for a node tree, and exposes typed values for
//! layout and rendering backends.
mod animation;
mod cascade;
mod computed;
mod diagnostics;
mod layout;
mod media;
mod modules;
mod parser;
mod render;
mod selector;
mod stylesheet;
mod timeline;
mod transform;
mod transitions;
mod utils;
mod values;

#[cfg(feature = "render-wgpu")]
mod wgpu_render;

pub use animation::{
    Animation, AnimationController, AnimationDirection, AnimationFillMode, AnimationIterationCount,
    AnimationPlayState, AnimationRun, AnimationSample, parse_animation, resolve_animations,
    sample_animation_runs, sample_keyframes,
};
pub use cascade::custom_properties::{is_valid_custom_property_name, resolve_vars};
pub use computed::model::{
    ComputedStyle, ComputedStyleTree, Declaration, Keyframe, Keyframes, StyleComputeContext,
    StyleNode, StyleRule, StyleState, default_computed_styles, style_compute_context,
};
pub use computed::presentation::{
    CssGradient, CssShadow, border_radii, edge_length, parse_gradient, parse_shadows,
    shorthand_color,
};
pub use computed::text::{
    FontStretch, TextAlignment, TextDecoration, TextDecorationStyle, TextDecorationThickness,
    TextPresentation, TextTransform, text_intrinsic_size, text_presentation,
};
pub use diagnostics::{CssDiagnostic, CssLoadError};
#[cfg(feature = "layout-taffy")]
pub use layout::overflow::{CssOverflowLayout, apply_overflow_layout, scroll_at_point};
#[cfg(feature = "layout-taffy")]
pub use layout::taffy::{CssLayoutStyle, is_scrollable, taffy_style, taffy_style_state};
pub use media::{CssMediaContext, MediaCondition};
pub use modules::standard::{
    CssBase, CssFlexbox, CssGrid, CssMedia, CssProperty, CssText, CssTransform,
};
pub use modules::{CssComputedState, CssDeclaration, CssModule, CssModuleRegistry, CssModuleState};
pub use lunelle_macros::{css_comma_list, css_keyword_enum, css_module, css_registry};
pub use render::CssRender;
pub use selector::{Selector, SelectorNode, Specificity};
pub use stylesheet::StyleSheet;
pub use transform::text_translation;
pub use transform::{
    Transform2D, TransformFunction, TransformLength, parse_transform, parse_transform_origin,
    translation,
};
pub use transitions::styles_changed_in;
pub use transitions::{
    StepPosition, TimingFunction, Transition, TransitionProperty, TransitionRun, TransitionSample,
    parse_transition,
};
pub use utils::{css_length, is_known_property, parse_color, rem_to_px, style_opacity};
pub use values::{
    AlignItems, Color, FlexDirection, FlexWrap, FontStyle, GridAutoFlow, GridTrack, JustifyContent,
    LayoutMode, Length, LineHeight, Overflow, Position, Style, TextAlign, Visibility,
};
#[cfg(feature = "render-wgpu")]
pub use wgpu_render::CssTextNode;
#[cfg(feature = "render-wgpu")]
pub use wgpu_render::{WgpuContext, WgpuCssRenderer};
