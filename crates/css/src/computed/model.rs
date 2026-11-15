use std::collections::BTreeMap;

use lunelle_core::NodeId;

use crate::{Animation, CssDiagnostic, CssMediaContext};

/// One parsed CSS property/value pair, retaining whether it was important.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    /// Lowercase CSS property name.
    pub name: String,
    /// Serialized property value.
    pub value: String,
    /// Whether the declaration used `!important`.
    pub important: bool,
}

/// A selector and its declarations after stylesheet parsing.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleRule {
    /// Selector source text.
    pub selector: String,
    /// Declarations belonging to the selector.
    pub declarations: Vec<Declaration>,
    /// Media conditions guarding this rule.
    pub media_conditions: Vec<crate::media::MediaCondition>,
    /// Position of the rule in the stylesheet.
    pub source_order: usize,
}

/// Declarations belonging to one keyframe offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    /// Normalized position from zero to one.
    pub offset: f32,
    /// Declarations applied at this position.
    pub declarations: Vec<Declaration>,
}

/// Named collection of keyframes used by the animation subsystem.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes {
    /// Name referenced by an animation declaration.
    pub name: String,
    /// Ordered keyframe declarations.
    pub frames: Vec<Keyframe>,
}

/// Minimal node metadata required by selector matching and cascade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleNode {
    /// Stable node identifier.
    pub id: NodeId,
    /// Lowercase element name.
    pub tag_name: String,
    /// Optional element id selector value.
    pub element_id: Option<String>,
    /// Class selector values.
    pub classes: Vec<String>,
    /// Parent node identifier.
    pub parent: Option<NodeId>,
}

/// Cascaded property map associated with one node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedStyle {
    /// Node to which this computed style belongs.
    pub node: NodeId,
    /// Cascaded serialized property values.
    pub properties: BTreeMap<String, String>,
}

impl ComputedStyle {
    /// Resolves this style's animation shorthand into animation runs.
    pub fn animations(&self) -> Option<Vec<Animation>> {
        crate::resolve_animations(&self.properties)
    }
}

/// Interactive pseudo-class state used during selector matching.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StyleState {
    /// Whether the node matches `:disabled`.
    pub disabled: bool,
    /// Whether the node matches `:focus`.
    pub focused: bool,
    /// Whether the node matches `:hover`.
    pub hovered: bool,
    /// Whether the node matches `:active`.
    pub active: bool,
}

/// Inputs that make a style computation environment-dependent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StyleComputeContext {
    /// Optional viewport media context.
    pub media: Option<CssMediaContext>,
    /// Version of the source tree snapshot.
    pub snapshot_version: u64,
    /// Version of the active stylesheet.
    pub stylesheet_version: u64,
    /// Version of environment-dependent inputs.
    pub environment_version: u64,
    /// Interactive pseudo-class states by node.
    pub states: BTreeMap<NodeId, StyleState>,
}

/// Creates empty computed styles in the same order as the supplied nodes.
pub fn default_computed_styles(nodes: &[StyleNode]) -> Vec<ComputedStyle> {
    nodes
        .iter()
        .map(|node| ComputedStyle {
            node: node.id,
            properties: BTreeMap::new(),
        })
        .collect()
}

/// Builds a computation context from window dimensions and version counters.
pub fn style_compute_context(
    snapshot_version: u64,
    stylesheet_version: u64,
    environment_version: u64,
    width: u32,
    height: u32,
    scale_factor: f32,
    states: BTreeMap<NodeId, StyleState>,
) -> StyleComputeContext {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    StyleComputeContext {
        media: Some(CssMediaContext {
            viewport: lunelle_core::Size {
                width: width as f32 / scale_factor,
                height: height as f32 / scale_factor,
            },
            scale_factor,
        }),
        snapshot_version,
        stylesheet_version,
        environment_version,
        states,
    }
}

/// Ordered computed-style results plus diagnostics from the CSS pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyleTree {
    /// Version of the source tree snapshot.
    pub snapshot_version: u64,
    /// Version of the active stylesheet.
    pub stylesheet_version: u64,
    /// Version of environment-dependent inputs.
    pub environment_version: u64,
    /// Computed styles in source node order.
    pub styles: Vec<ComputedStyle>,
    /// Diagnostics accumulated during computation.
    pub diagnostics: Vec<CssDiagnostic>,
}
