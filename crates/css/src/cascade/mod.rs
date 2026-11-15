use std::collections::{BTreeMap, HashMap};

use lunelle_core::NodeId;

use crate::selector::{Selector, SelectorNode, Specificity};
use crate::{ComputedStyle, CssDiagnostic, StyleComputeContext, StyleNode, StyleSheet};

pub(crate) mod custom_properties;
mod resolver;

use resolver::resolve_computed_style;

/// Per-computation state for matching rules and resolving inherited values.
struct CascadeEngine<'a> {
    sheet: &'a StyleSheet,
    nodes: &'a [StyleNode],
    context: &'a StyleComputeContext,
    by_id: BTreeMap<NodeId, &'a StyleNode>,
    indices: BTreeMap<NodeId, usize>,
    selector_nodes: Vec<SelectorNode>,
    diagnostics: Vec<CssDiagnostic>,
    own_properties: BTreeMap<NodeId, BTreeMap<String, String>>,
    own_sources: BTreeMap<NodeId, BTreeMap<String, usize>>,
    computed: BTreeMap<NodeId, ComputedStyle>,
}

/// Runs selector matching, cascade ordering, inheritance, and var substitution.
pub(crate) fn compute_with_context(
    sheet: &StyleSheet,
    nodes: &[StyleNode],
    context: &StyleComputeContext,
) -> (Vec<ComputedStyle>, Vec<CssDiagnostic>) {
    CascadeEngine::new(sheet, nodes, context).compute()
}

impl<'a> CascadeEngine<'a> {
    /// Builds node lookup tables used by selector matching and parent traversal.
    fn new(
        sheet: &'a StyleSheet,
        nodes: &'a [StyleNode],
        context: &'a StyleComputeContext,
    ) -> Self {
        let indices = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, index))
            .collect::<BTreeMap<_, _>>();
        let selector_nodes = nodes
            .iter()
            .map(|node| SelectorNode {
                tag_name: node.tag_name.clone(),
                element_id: node.element_id.clone(),
                classes: node.classes.clone(),
                parent: node.parent.and_then(|parent| indices.get(&parent).copied()),
                disabled: context
                    .states
                    .get(&node.id)
                    .is_some_and(|state| state.disabled),
                focused: context
                    .states
                    .get(&node.id)
                    .is_some_and(|state| state.focused),
                hovered: context
                    .states
                    .get(&node.id)
                    .is_some_and(|state| state.hovered),
                active: context
                    .states
                    .get(&node.id)
                    .is_some_and(|state| state.active),
            })
            .collect();
        Self {
            sheet,
            nodes,
            context,
            by_id: nodes.iter().map(|node| (node.id, node)).collect(),
            indices,
            selector_nodes,
            diagnostics: Vec::new(),
            own_properties: BTreeMap::new(),
            own_sources: BTreeMap::new(),
            computed: BTreeMap::new(),
        }
    }

    /// Executes collection and resolution stages, preserving input node order.
    fn compute(mut self) -> (Vec<ComputedStyle>, Vec<CssDiagnostic>) {
        self.collect_declarations();
        self.resolve_styles();
        let styles = self
            .nodes
            .iter()
            .filter_map(|node| self.computed.remove(&node.id))
            .collect();
        (styles, self.diagnostics)
    }

    /// Chooses winning declarations by importance, specificity, and source order.
    fn collect_declarations(&mut self) {
        let mut parsed_selectors = HashMap::new();
        for node in self.nodes {
            let mut winners = BTreeMap::<String, (bool, Specificity, usize, usize, String)>::new();
            for rule in &self.sheet.rules {
                if !crate::CssMedia::matches(&rule.media_conditions, self.context.media) {
                    continue;
                }
                for selector in rule
                    .selector
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let parsed = parsed_selectors
                        .entry(selector)
                        .or_insert_with(|| Selector::parse(selector));
                    let Ok(parsed) = parsed else {
                        let diagnostic = CssDiagnostic {
                            rule: rule.source_order + 1,
                            message: format!("unsupported selector `{selector}`"),
                        };
                        if !self.diagnostics.contains(&diagnostic) {
                            self.diagnostics.push(diagnostic);
                        }
                        continue;
                    };
                    let node_index = self.indices[&node.id];
                    if !parsed.matches(&self.selector_nodes, node_index) {
                        continue;
                    }
                    let specificity = parsed.specificity();
                    for (declaration_index, declaration) in rule.declarations.iter().enumerate() {
                        if declaration.value.eq_ignore_ascii_case("revert-layer") {
                            let diagnostic = CssDiagnostic {
                                rule: rule.source_order + 1,
                                message: format!(
                                    "unsupported CSS-wide keyword `{}` for `{}`",
                                    declaration.value, declaration.name
                                ),
                            };
                            if !self.diagnostics.contains(&diagnostic) {
                                self.diagnostics.push(diagnostic);
                            }
                            continue;
                        }
                        let names: Vec<(&str, String)> = if declaration.name == "flex" {
                            flex_longhand_values(&declaration.value)
                                .map(|values| {
                                    vec![
                                        ("flex", declaration.value.clone()),
                                        ("flex-grow", values.0),
                                        ("flex-shrink", values.1),
                                        ("flex-basis", values.2),
                                    ]
                                })
                                .unwrap_or_else(|| vec![("flex", declaration.value.clone())])
                        } else {
                            vec![(declaration.name.as_str(), declaration.value.clone())]
                        };
                        for (name, value) in names {
                            let candidate = (
                                declaration.important,
                                specificity,
                                rule.source_order,
                                declaration_index,
                                value,
                            );
                            if winners.get(name).is_none_or(|current| {
                                (candidate.0, candidate.1, candidate.2, candidate.3)
                                    >= (current.0, current.1, current.2, current.3)
                            }) {
                                winners.insert(name.to_owned(), candidate);
                            }
                        }
                    }
                }
            }
            let mut properties = BTreeMap::new();
            let mut sources = BTreeMap::new();
            for (name, (_, _, source_order, _, value)) in winners {
                if value.eq_ignore_ascii_case("revert") {
                    if name == "opacity" {
                        properties.insert(name, "1".to_owned());
                    }
                    continue;
                }
                properties.insert(name.clone(), value);
                sources.insert(name, source_order + 1);
            }
            self.own_properties.insert(node.id, properties);
            self.own_sources.insert(node.id, sources);
        }
    }

    /// Resolves inherited and custom properties for every node in the tree.
    fn resolve_styles(&mut self) {
        for node in self.nodes {
            resolve_computed_style(
                node.id,
                &self.by_id,
                &mut self.own_properties,
                &self.own_sources,
                &mut self.computed,
                &mut BTreeMap::new(),
                &mut self.diagnostics,
            );
        }
    }
}

fn flex_longhand_values(value: &str) -> Option<(String, String, String)> {
    let tokens = crate::utils::split_top_level_whitespace(value);
    match tokens.as_slice() {
        [single] if *single == "none" => Some(("0".into(), "0".into(), "auto".into())),
        [single] if *single == "auto" => Some(("1".into(), "1".into(), "auto".into())),
        [single] if *single == "initial" => Some(("0".into(), "1".into(), "auto".into())),
        [single] => single
            .parse::<f32>()
            .ok()
            .map(|_| (single.to_string(), "1".into(), "auto".into()))
            .or_else(|| {
                crate::utils::length_string(single)
                    .map(|_| ("1".into(), "1".into(), single.to_string()))
            }),
        [grow, shrink] if grow.parse::<f32>().is_ok() && shrink.parse::<f32>().is_ok() => {
            Some((grow.to_string(), shrink.to_string(), "auto".into()))
        }
        [grow, shrink, basis]
            if grow.parse::<f32>().is_ok()
                && shrink.parse::<f32>().is_ok()
                && crate::utils::length_string(basis).is_some() =>
        {
            Some((grow.to_string(), shrink.to_string(), basis.to_string()))
        }
        _ => None,
    }
}
