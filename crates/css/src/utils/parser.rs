/// Splits on delimiters outside nested function parentheses.
pub fn split_top_level(value: &str, delimiter: char) -> Vec<&str> {
    let (mut parts, mut depth, mut start) = (Vec::new(), 0usize, 0);
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if character == delimiter && depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim());
    parts
}

/// Splits at the first separator outside nested parentheses.
pub fn split_top_level_once(value: &str, separator: char) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            character if character == separator && depth == 0 => {
                return Some((&value[..index], &value[index + character.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}

/// Splits whitespace-separated tokens while preserving function arguments.
pub fn split_top_level_whitespace(value: &str) -> Vec<&str> {
    let mut depth = 0usize;
    let mut start = None;
    let mut result = Vec::new();
    for (index, character) in value.char_indices() {
        match character {
            '(' => {
                depth += 1;
                start.get_or_insert(index);
            }
            ')' => depth = depth.saturating_sub(1),
            character if character.is_whitespace() && depth == 0 => {
                if let Some(start) = start.take() {
                    result.push(&value[start..index]);
                }
            }
            _ => {
                start.get_or_insert(index);
            }
        }
    }
    if let Some(start) = start {
        result.push(&value[start..]);
    }
    result
}
