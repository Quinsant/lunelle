use lunelle_core::{ElementId, NodeId};
use thiserror::Error;

use crate::Accessibility;
pub use lunelle_css::StyleNode;

#[cfg(feature = "base_component")]
pub use crate::base::{BuiltinComponent, Button, Div, Input, Li, List, P, Text};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleSnapshot {
    pub version: u64,
    pub nodes: Vec<StyleNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleResult {
    pub version: u64,
    pub stylesheet_version: u64,
    pub environment_version: u64,
    pub styles: Vec<lunelle_css::ComputedStyle>,
}

#[derive(Clone)]
pub struct ElementData {
    pub id: ElementId,
    pub element_id: Option<String>,
    pub classes: Vec<String>,
    pub text: Option<String>,
    pub accessibility: Accessibility,
    pub children: Vec<Box<dyn Element>>,
}

impl ElementData {
    pub fn new() -> Self {
        Self {
            id: ElementId::fresh(),
            element_id: None,
            classes: Vec::new(),
            text: None,
            accessibility: Accessibility::default(),
            children: Vec::new(),
        }
    }
}

pub trait Element: ElementClone + Send {
    fn data(&self) -> &ElementData;
    fn data_mut(&mut self) -> &mut ElementData;
    fn tag_name(&self) -> &'static str;

    fn id(&self) -> ElementId {
        self.data().id
    }
    fn set_id(&mut self, id: ElementId) {
        self.data_mut().id = id;
    }
    fn element_id(&self) -> Option<&str> {
        self.data().element_id.as_deref()
    }
    fn classes(&self) -> &[String] {
        &self.data().classes
    }
    fn text(&self) -> Option<&str> {
        self.data().text.as_deref()
    }
    fn accessibility(&self) -> &Accessibility {
        &self.data().accessibility
    }
    fn children(&self) -> &[Box<dyn Element>] {
        &self.data().children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Element>> {
        &mut self.data_mut().children
    }
    fn style_snapshot(&self, version: u64) -> StyleSnapshot {
        snapshot(self, version)
    }
}

pub trait ElementClone {
    fn clone_box(&self) -> Box<dyn Element>;
}

impl<T> ElementClone for T
where
    T: Element + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn Element> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Element> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait IntoView {
    fn into_view(self) -> Box<dyn Element>;
}

impl<T: Element + 'static> IntoView for T {
    fn into_view(self) -> Box<dyn Element> {
        Box::new(self)
    }
}

impl IntoView for Box<dyn Element> {
    fn into_view(self) -> Box<dyn Element> {
        self
    }
}

fn snapshot<T: Element + ?Sized>(element: &T, version: u64) -> StyleSnapshot {
    fn count<T: Element + ?Sized>(element: &T) -> usize {
        1 + element
            .children()
            .iter()
            .map(|child| count(child.as_ref()))
            .sum::<usize>()
    }

    fn collect<T: Element + ?Sized>(element: &T, parent: Option<NodeId>, out: &mut Vec<StyleNode>) {
        let id = NodeId::from_raw(element.id().get());
        out.push(StyleNode {
            id,
            tag_name: element.tag_name().to_owned(),
            element_id: element.element_id().map(str::to_owned),
            classes: element.classes().to_vec(),
            parent,
        });
        for child in element.children() {
            collect(child.as_ref(), Some(id), out);
        }
    }
    let mut nodes = Vec::with_capacity(count(element));
    collect(element, None, &mut nodes);
    StyleSnapshot { version, nodes }
}

#[derive(Debug, Error)]
pub enum AomError {
    #[error("stale worker result: expected snapshot {expected}, got {actual}")]
    StaleSnapshot { expected: u64, actual: u64 },
    #[error("stale worker result: expected stylesheet {expected}, got {actual}")]
    StaleStylesheet { expected: u64, actual: u64 },
    #[error("stale worker result: expected environment {expected}, got {actual}")]
    StaleEnvironment { expected: u64, actual: u64 },
}

impl StyleSnapshot {
    pub fn accepts_result(&self, result_version: u64) -> Result<(), AomError> {
        if result_version == self.version {
            Ok(())
        } else {
            Err(AomError::StaleSnapshot {
                expected: self.version,
                actual: result_version,
            })
        }
    }

    pub fn accepts_computed_result(
        &self,
        result: &StyleResult,
        stylesheet_version: u64,
        environment_version: u64,
    ) -> Result<(), AomError> {
        self.accepts_result(result.version)?;
        if result.stylesheet_version != stylesheet_version {
            return Err(AomError::StaleStylesheet {
                expected: stylesheet_version,
                actual: result.stylesheet_version,
            });
        }
        if result.environment_version != environment_version {
            return Err(AomError::StaleEnvironment {
                expected: environment_version,
                actual: result.environment_version,
            });
        }
        Ok(())
    }

    /// Computes from immutable selector data; the caller applies only results matching this version.
    pub fn compute(
        &self,
        css: &lunelle_css::CssRender,
    ) -> (StyleResult, Vec<lunelle_css::CssDiagnostic>) {
        let (styles, diagnostics) = css.compute_with_diagnostics(&self.nodes);
        (
            StyleResult {
                version: self.version,
                stylesheet_version: 0,
                environment_version: 0,
                styles,
            },
            diagnostics,
        )
    }

    pub fn compute_with_context(
        &self,
        css: &lunelle_css::CssRender,
        context: &lunelle_css::StyleComputeContext,
    ) -> (StyleResult, Vec<lunelle_css::CssDiagnostic>) {
        let mut context = context.clone();
        context.snapshot_version = self.version;
        let computed = css.compute_with_context(&self.nodes, &context);
        (
            StyleResult {
                version: computed.snapshot_version,
                stylesheet_version: computed.stylesheet_version,
                environment_version: computed.environment_version,
                styles: computed.styles,
            },
            computed.diagnostics,
        )
    }

    /// Computes styles on a worker using owned snapshot and stylesheet data.
    ///
    /// The caller retains ownership of the component tree; only this immutable snapshot
    /// is transferred to the runtime. The caller must compare the returned result version
    /// with its current snapshot before applying it on the UI thread.
    pub fn compute_async(
        &self,
        runtime: &lunelle_core::Runtime,
        css: lunelle_css::CssRender,
    ) -> Result<
        lunelle_core::Task<(StyleResult, Vec<lunelle_css::CssDiagnostic>)>,
        lunelle_core::TaskError,
    > {
        let snapshot = self.clone();
        runtime.spawn_css(move |_| Ok(snapshot.compute(&css)))
    }

    pub fn compute_async_with_context(
        &self,
        runtime: &lunelle_core::Runtime,
        css: lunelle_css::CssRender,
        context: lunelle_css::StyleComputeContext,
    ) -> Result<
        lunelle_core::Task<(StyleResult, Vec<lunelle_css::CssDiagnostic>)>,
        lunelle_core::TaskError,
    > {
        let snapshot = self.clone();
        runtime.spawn_css(move |_| Ok(snapshot.compute_with_context(&css, &context)))
    }
}
