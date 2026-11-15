use std::collections::{BTreeMap, HashMap};

use lunelle_core::NodeId;

use crate::{ComputedStyle, CssDiagnostic, StyleNode};

/// Resolves one node after its parent, applying inheritance and CSS-wide values.
pub(super) fn resolve_computed_style(
    id: NodeId,
    nodes: &BTreeMap<NodeId, &StyleNode>,
    own: &mut BTreeMap<NodeId, BTreeMap<String, String>>,
    sources: &BTreeMap<NodeId, BTreeMap<String, usize>>,
    computed: &mut BTreeMap<NodeId, ComputedStyle>,
    visiting: &mut BTreeMap<NodeId, ()>,
    diagnostics: &mut Vec<CssDiagnostic>,
) {
    if computed.contains_key(&id) || visiting.insert(id, ()).is_some() {
        return;
    }
    let Some(node) = nodes.get(&id) else {
        visiting.remove(&id);
        return;
    };
    let parent_id = node.parent.filter(|parent| nodes.contains_key(parent));
    if let Some(parent) = parent_id {
        resolve_computed_style(parent, nodes, own, sources, computed, visiting, diagnostics);
    }
    let mut properties = own.remove(&id).unwrap_or_default();
    for property in [
        "color",
        "font-family",
        "font-size",
        "font-style",
        "font-weight",
        "line-height",
        "text-align",
        "text-transform",
        "visibility",
    ] {
        if !properties.contains_key(property) {
            if let Some(value) = parent_id
                .and_then(|parent| computed.get(&parent))
                .and_then(|style| style.properties.get(property))
            {
                properties.insert(property.to_owned(), value.clone());
            }
        }
    }
    let parent_properties = parent_id
        .and_then(|parent| computed.get(&parent))
        .map(|style| &style.properties);
    if let Some(parent_properties) = parent_properties {
        for (property, value) in parent_properties {
            if property.starts_with("--") && !properties.contains_key(property) {
                properties.insert(property.clone(), value.clone());
            }
        }
    }
    let wide_keywords = properties
        .iter()
        .filter_map(|(property, value)| {
            ["inherit", "initial", "unset"]
                .iter()
                .any(|keyword| value.eq_ignore_ascii_case(keyword))
                .then(|| (property.clone(), value.clone()))
        })
        .collect::<Vec<_>>();
    for (property, keyword) in wide_keywords {
        let inherits = is_inherited_property(&property);
        let inherited_value = parent_properties
            .and_then(|values| values.get(&property))
            .cloned();
        if keyword.eq_ignore_ascii_case("inherit") {
            properties.remove(&property);
            if let Some(value) = inherited_value {
                properties.insert(property, value);
            }
        } else if keyword.eq_ignore_ascii_case("unset") && inherits {
            properties.remove(&property);
            if let Some(value) = inherited_value {
                properties.insert(property, value);
            }
        } else {
            properties.remove(&property);
            if property == "opacity" {
                properties.insert(property, "1".to_owned());
            }
        }
    }
    let specified_custom_properties = properties
        .iter()
        .filter(|(property, _)| property.starts_with("--"))
        .map(|(property, value)| (property.clone(), value.clone()))
        .collect::<HashMap<_, _>>();
    let resolved_custom_properties = specified_custom_properties
        .iter()
        .filter_map(|(property, value)| {
            crate::resolve_vars(value, &specified_custom_properties, 0)
                .map(|value| (property.clone(), value))
        })
        .collect::<HashMap<_, _>>();
    let custom_property_names = specified_custom_properties
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for property in custom_property_names {
        if let Some(value) = resolved_custom_properties.get(&property) {
            properties.insert(property, value.clone());
        } else {
            properties.remove(&property);
            if let Some(rule) = sources
                .get(&id)
                .and_then(|properties| properties.get(&property))
            {
                push_var_diagnostic(diagnostics, *rule, &property);
            }
        }
    }
    let variable_values = properties
        .iter()
        .filter(|(property, value)| {
            !property.starts_with("--") && value.to_ascii_lowercase().contains("var(")
        })
        .map(|(property, value)| {
            (
                property.clone(),
                crate::resolve_vars(value, &resolved_custom_properties, 0),
            )
        })
        .collect::<Vec<_>>();
    for (property, value) in variable_values {
        match value {
            Some(value) => {
                properties.insert(property, value);
            }
            None => {
                properties.remove(&property);
                if let Some(rule) = sources
                    .get(&id)
                    .and_then(|properties| properties.get(&property))
                {
                    push_var_diagnostic(diagnostics, *rule, &property);
                }
                if is_inherited_property(&property) {
                    if let Some(value) = parent_properties.and_then(|parent| parent.get(&property))
                    {
                        properties.insert(property, value.clone());
                    }
                }
            }
        }
    }
    visiting.remove(&id);
    computed.insert(
        id,
        ComputedStyle {
            node: id,
            properties,
        },
    );
}

fn push_var_diagnostic(diagnostics: &mut Vec<CssDiagnostic>, rule: usize, property: &str) {
    let diagnostic = CssDiagnostic {
        rule,
        message: format!(
            "invalid value for `{property}`: unresolved `var()` reference; declaration discarded"
        ),
    };
    if !diagnostics.contains(&diagnostic) {
        diagnostics.push(diagnostic);
    }
}

fn is_inherited_property(property: &str) -> bool {
    property.starts_with("--")
        || matches!(
            property,
            "color"
                | "font-family"
                | "font-size"
                | "font-style"
                | "font-weight"
                | "line-height"
                | "text-align"
                | "text-transform"
                | "visibility"
        )
}
