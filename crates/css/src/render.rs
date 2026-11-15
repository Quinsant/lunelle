use crate::timeline::{CssTimelineBackend, TimelineBackend};
use crate::{ComputedStyle, ComputedStyleTree, CssComputedState};

trait StyleBackend {
    fn stylesheet(&self) -> &crate::StyleSheet;
    fn set_stylesheet(&mut self, stylesheet: crate::StyleSheet);
    fn compute(
        &self,
        nodes: &[crate::StyleNode],
        context: &crate::StyleComputeContext,
    ) -> ComputedStyleTree;
}

#[derive(Clone, Default)]
struct StylesheetBackend {
    stylesheet: crate::StyleSheet,
}

impl StyleBackend for StylesheetBackend {
    fn stylesheet(&self) -> &crate::StyleSheet {
        &self.stylesheet
    }

    fn set_stylesheet(&mut self, stylesheet: crate::StyleSheet) {
        self.stylesheet = stylesheet;
    }

    fn compute(
        &self,
        nodes: &[crate::StyleNode],
        context: &crate::StyleComputeContext,
    ) -> ComputedStyleTree {
        let (styles, diagnostics) =
            crate::cascade::compute_with_context(&self.stylesheet, nodes, context);
        ComputedStyleTree {
            snapshot_version: context.snapshot_version,
            stylesheet_version: context.stylesheet_version,
            environment_version: context.environment_version,
            styles,
            diagnostics,
        }
    }
}

/// CSS rendering facade: samples CSS animations and transitions over computed styles.
///
/// The application supplies computed styles and a timestamp; layout and GPU submission
/// remain responsibilities of the application renderer.
#[derive(Default)]
pub struct CssRender {
    modules: crate::CssModuleRegistry,
    styles: StylesheetBackend,
    timeline: CssTimelineBackend,
}

impl CssRender {
    /// Creates a renderer using the standard CSS module registry.
    pub fn new(stylesheet: crate::StyleSheet) -> Self {
        Self::with_modules(stylesheet, crate::CssModuleRegistry::standard())
    }

    /// Creates a renderer with an explicitly supplied module registry.
    pub fn with_modules(stylesheet: crate::StyleSheet, modules: crate::CssModuleRegistry) -> Self {
        Self {
            modules,
            styles: StylesheetBackend { stylesheet },
            timeline: CssTimelineBackend::default(),
        }
    }

    /// Parses CSS source and creates a renderer with standard modules.
    pub fn parse(source: &str) -> Self {
        Self::new(crate::StyleSheet::parse(source))
    }

    /// Loads a stylesheet beneath a resource root and creates a renderer.
    pub fn load(
        resource_root: impl AsRef<std::path::Path>,
        relative_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, crate::CssLoadError> {
        crate::StyleSheet::load(resource_root, relative_path).map(Self::new)
    }

    /// Returns the currently installed stylesheet.
    pub fn stylesheet(&self) -> &crate::StyleSheet {
        self.styles.stylesheet()
    }

    /// Returns the registry used for property validation and dispatch.
    pub fn modules(&self) -> &crate::CssModuleRegistry {
        &self.modules
    }

    /// Projects computed styles into typed layout and text states.
    pub fn states(&self, styles: &[ComputedStyle]) -> Vec<CssComputedState> {
        self.modules.states(styles)
    }

    /// Reports whether layout-relevant properties differ between style sets.
    pub fn layout_changed(&self, previous: &[ComputedStyle], current: &[ComputedStyle]) -> bool {
        crate::transitions::styles_changed_in(
            previous,
            current,
            &[
                "display",
                "flex-direction",
                "justify-content",
                "align-items",
                "gap",
                "padding",
                "border-width",
                "width",
                "height",
                "min-width",
                "overflow",
                "overflow-x",
                "overflow-y",
                "grid-template-columns",
                "grid-template-rows",
                "font-size",
            ],
        )
    }

    /// Reports whether text/render-relevant properties differ between style sets.
    pub fn text_changed(&self, previous: &[ComputedStyle], current: &[ComputedStyle]) -> bool {
        crate::transitions::styles_changed_in(
            previous,
            current,
            &[
                "color",
                "opacity",
                "font-size",
                "transform",
                "transform-origin",
                "display",
                "visibility",
                "font-family",
                "font-weight",
                "font-style",
                "font-stretch",
                "line-height",
                "text-align",
                "white-space",
                "text-decoration",
                "text-decoration-line",
                "text-decoration-color",
                "text-decoration-style",
                "text-decoration-thickness",
                "text-transform",
            ],
        )
    }

    /// Replaces the module registry used by subsequent computations.
    pub fn set_modules(&mut self, modules: crate::CssModuleRegistry) {
        self.modules = modules;
    }

    /// Replaces the stylesheet used by subsequent computations.
    pub fn set_stylesheet(&mut self, stylesheet: crate::StyleSheet) {
        self.styles.set_stylesheet(stylesheet);
    }

    /// Computes styles for nodes using the default empty computation context.
    pub fn compute(&self, nodes: &[crate::StyleNode]) -> Vec<ComputedStyle> {
        self.compute_with_context(nodes, &crate::StyleComputeContext::default())
            .styles
    }

    /// Computes styles and returns diagnostics alongside them.
    pub fn compute_with_diagnostics(
        &self,
        nodes: &[crate::StyleNode],
    ) -> (Vec<ComputedStyle>, Vec<crate::CssDiagnostic>) {
        let tree = self.compute_with_context(nodes, &crate::StyleComputeContext::default());
        (tree.styles, tree.diagnostics)
    }

    /// Computes the versioned style tree using the supplied environment context.
    pub fn compute_with_context(
        &self,
        nodes: &[crate::StyleNode],
        context: &crate::StyleComputeContext,
    ) -> ComputedStyleTree {
        let mut tree = self.styles.compute(nodes, context);
        self.modules.compute(&mut tree, context);
        tree
    }

    /// Advances animation and transition timelines and samples their values.
    pub fn update(&mut self, tree: ComputedStyleTree, now_ms: f32) -> (ComputedStyleTree, bool) {
        self.timeline
            .update(tree, &self.styles.stylesheet().keyframes, now_ms)
    }

    /// Reports whether timelines require another render frame.
    pub fn needs_frame(&self, now_ms: f32) -> bool {
        self.timeline.needs_frame(now_ms)
    }

    /// Retargets supported transitions after the base computed style changes.
    pub fn retarget_transitions(
        &mut self,
        previous_base: &[ComputedStyle],
        presented: &[ComputedStyle],
        next_base: &[ComputedStyle],
        now_ms: f32,
    ) {
        self.timeline
            .retarget_transitions(previous_base, presented, next_base, now_ms);
    }
}
