use std::collections::HashMap;

/// Validates a custom property identifier accepted by this CSS implementation.
pub fn is_valid_custom_property_name(name: &str) -> bool {
    name.strip_prefix("--").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-' || character == '_'
            })
    })
}

/// Recursively substitutes `var()` references and fallbacks, with a depth limit.
pub fn resolve_vars(
    value: &str,
    custom_properties: &HashMap<String, String>,
    depth: usize,
) -> Option<String> {
    if depth >= 16 {
        return None;
    }
    let lowered = value.to_ascii_lowercase();
    let Some(start) = lowered.find("var(") else {
        return Some(value.to_owned());
    };
    let open = start + 3;
    let mut nesting = 1usize;
    let mut close = None;
    for (offset, character) in value[open + 1..].char_indices() {
        match character {
            '(' => nesting += 1,
            ')' => {
                nesting -= 1;
                if nesting == 0 {
                    close = Some(open + 1 + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let arguments = &value[open + 1..close];
    let mut nesting = 0usize;
    let mut separator = None;
    for (index, character) in arguments.char_indices() {
        match character {
            '(' => nesting += 1,
            ')' => nesting = nesting.saturating_sub(1),
            ',' if nesting == 0 => {
                separator = Some(index);
                break;
            }
            _ => {}
        }
    }
    let (name, fallback) = match separator {
        Some(index) => (&arguments[..index], Some(&arguments[index + 1..])),
        None => (arguments, None),
    };
    let name = name.trim();
    if !is_valid_custom_property_name(name) {
        return None;
    }
    let replacement = custom_properties
        .get(name)
        .and_then(|defined| resolve_vars(defined, custom_properties, depth + 1))
        .or_else(|| {
            fallback
                .and_then(|fallback| resolve_vars(fallback.trim(), custom_properties, depth + 1))
        })?;
    let mut resolved = String::with_capacity(value.len() + replacement.len());
    resolved.push_str(&value[..start]);
    resolved.push_str(&replacement);
    resolved.push_str(&resolve_vars(
        &value[close + 1..],
        custom_properties,
        depth + 1,
    )?);
    Some(resolved)
}
