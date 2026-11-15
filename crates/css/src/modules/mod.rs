use std::collections::BTreeMap;

use lunelle_core::NodeId;
use lunelle_macros::css_registry;

use crate::{
    ComputedStyle, ComputedStyleTree, CssMediaContext, Declaration, Style, StyleComputeContext,
    StyleState, TextPresentation,
};
mod computed;
pub mod standard;

/// Describes one CSS declaration and validates or stores its computed value.
///
/// Implementations are owned by a [`CssModule`] and are invoked by
/// the registry's computation pipeline while dispatching properties to their owner.
pub trait CssDeclaration: Send + Sync {
    /// Returns the canonical CSS property name handled by this declaration.
    fn name(&self) -> &'static str;
    /// Parses a raw declaration value; the default accepts non-empty text.
    fn parse(&self, value: &str) -> Option<String> {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }
    /// Applies a validated declaration to one computed style.
    fn compute(&self, declaration: &Declaration, style: &mut crate::ComputedStyle) {
        if let Some(value) = self.parse(&declaration.value) {
            style.properties.insert(self.name().to_owned(), value);
        } else {
            style.properties.remove(self.name());
        }
    }
}

/// Typed values projected from a [`ComputedStyle`] for layout and painting.
#[derive(Clone, Debug, PartialEq)]
pub struct CssComputedState {
    /// Typed layout and box values.
    pub values: Style,
    /// Parsed transform functions.
    pub transform: Option<Vec<crate::TransformFunction>>,
    /// Resolved transform origin in pixels.
    pub transform_origin: Option<(f32, f32)>,
    /// Renderer-facing text values.
    pub text: TextPresentation,
}

impl CssComputedState {
    /// Converts serialized computed properties into typed CSS values.
    ///
    /// The resulting state is consumed by the layout adapter and renderers.
    pub fn from_computed(style: &ComputedStyle) -> Self {
        computed::from_computed(style)
    }
}

/// Extension point for a group of CSS declarations and computation hooks.
///
/// Modules register property ownership, optionally prepare shared context, and
/// finalize the computed tree after declarations have been dispatched.
pub trait CssModule: Send + Sync {
    /// Returns the stable module identifier used in diagnostics and ownership.
    fn name(&self) -> &'static str;
    /// Returns the declarations owned by this module.
    fn declarations(&self) -> &[Box<dyn CssDeclaration>];

    /// Prepares the module before styles are computed.
    fn prepare(&self, _context: &StyleComputeContext) {}

    /// Finalizes the computed tree after all declarations are applied.
    fn finish(&self, _tree: &mut ComputedStyleTree, _context: &StyleComputeContext) {}

    /// Adapts state storage to the context-based preparation hook.
    fn prepare_state(&self, state: &CssModuleState) {
        let context = state.context();
        self.prepare(&context);
    }

    /// Adapts state storage to the context-based finalization hook.
    fn finish_state(&self, tree: &mut ComputedStyleTree, state: &CssModuleState) {
        let context = state.context();
        self.finish(tree, &context);
    }
}

/// Snapshot of environment and node state passed to CSS modules.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CssModuleState {
    /// Version of the active stylesheet.
    pub stylesheet_version: u64,
    /// Version of environment-dependent inputs.
    pub environment_version: u64,
    /// Version of the source tree snapshot.
    pub snapshot_version: u64,
    /// Optional viewport media context.
    pub media: Option<CssMediaContext>,
    /// Interactive pseudo-class states by node.
    pub states: BTreeMap<NodeId, StyleState>,
}

impl CssModuleState {
    /// Copies the computation context into module-owned state.
    pub fn from_context(context: &StyleComputeContext) -> Self {
        Self {
            stylesheet_version: context.stylesheet_version,
            environment_version: context.environment_version,
            snapshot_version: context.snapshot_version,
            media: context.media,
            states: context.states.clone(),
        }
    }

    /// Reconstructs the public computation context from this snapshot.
    pub fn context(&self) -> StyleComputeContext {
        StyleComputeContext {
            media: self.media,
            snapshot_version: self.snapshot_version,
            stylesheet_version: self.stylesheet_version,
            environment_version: self.environment_version,
            states: self.states.clone(),
        }
    }
}

/// Registry that dispatches CSS properties to modules and tracks ownership.
pub struct CssModuleRegistry {
    modules: Vec<Box<dyn CssModule>>,
    owners: BTreeMap<&'static str, &'static str>,
    duplicate_owners: Vec<(&'static str, &'static str, &'static str)>,
}

impl Default for CssModuleRegistry {
    fn default() -> Self {
        Self::standard()
    }
}

#[css_registry(modules(
    standard::CssBase,
    standard::CssFlexbox,
    standard::CssGrid,
    standard::CssTransform,
    standard::CssMedia,
    standard::CssProperty,
    standard::CssText,
))]
impl CssModuleRegistry {
    /// Adds a module and rebuilds the property ownership index.
    pub fn register<M: CssModule + 'static>(&mut self, module: M) {
        self.modules.push(Box::new(module));
        self.rebuild_index();
    }

    /// Iterates over modules in registration order.
    pub fn modules(&self) -> impl Iterator<Item = &dyn CssModule> {
        self.modules.iter().map(Box::as_ref)
    }

    /// Returns the module name responsible for a property, if any.
    pub fn module_for(&self, property: &str) -> Option<&'static str> {
        self.owners.get(property).copied()
    }

    /// Returns property ownership collisions found during index rebuilding.
    pub fn duplicate_owners(&self) -> &[(&'static str, &'static str, &'static str)] {
        &self.duplicate_owners
    }

    /// Reports whether a property is supported, including custom properties.
    pub fn is_known(&self, property: &str) -> bool {
        property.starts_with("--") || self.owners.contains_key(property)
    }

    /// Converts one computed style into the typed state used by consumers.
    pub fn state(&self, style: &ComputedStyle) -> CssComputedState {
        CssComputedState::from_computed(style)
    }

    /// Converts a slice of computed styles while preserving node order.
    pub fn states(&self, styles: &[ComputedStyle]) -> Vec<CssComputedState> {
        styles.iter().map(|style| self.state(style)).collect()
    }

    pub(crate) fn compute(&self, tree: &mut ComputedStyleTree, context: &StyleComputeContext) {
        let state = CssModuleState::from_context(context);
        for (property, first, duplicate) in &self.duplicate_owners {
            tree.diagnostics.push(crate::CssDiagnostic {
                rule: 0,
                message: format!(
                    "CSS property {property} is registered by both {first} and {duplicate}"
                ),
            });
        }
        for module in &self.modules {
            module.prepare_state(&state);
        }
        for style in &mut tree.styles {
            let declarations = style
                .properties
                .iter()
                .map(|(name, value)| Declaration {
                    name: name.clone(),
                    value: value.clone(),
                    important: false,
                })
                .collect::<Vec<_>>();
            for declaration in declarations {
                if let Some(module_name) = self.owners.get(declaration.name.as_str()) {
                    if let Some(module) = self
                        .modules
                        .iter()
                        .find(|module| module.name() == *module_name)
                    {
                        if let Some(handler) = module
                            .declarations()
                            .iter()
                            .find(|handler| handler.name() == declaration.name)
                        {
                            handler.compute(&declaration, style);
                        }
                    }
                }
            }
        }
        for module in &self.modules {
            module.finish_state(tree, &state);
        }
    }

    fn rebuild_index(&mut self) {
        self.owners.clear();
        self.duplicate_owners.clear();
        for module in &self.modules {
            for declaration in module.declarations() {
                if let Some(owner) = self.owners.get(declaration.name()).copied() {
                    self.duplicate_owners
                        .push((declaration.name(), owner, module.name()));
                } else {
                    self.owners.insert(declaration.name(), module.name());
                }
            }
        }
    }
}
