/// Rewrites `rem` lengths using the framework's 16-pixel root font size.
pub fn rem_to_px(declarations: &str) -> String {
    let bytes = declarations.as_bytes();
    let mut output = String::with_capacity(declarations.len());
    let mut index = 0;
    while index < bytes.len() {
        let number_start = index;
        if bytes[index] == b'-' {
            index += 1;
        }
        let digits_start = index;
        while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
            index += 1;
        }
        if index > digits_start && declarations[index..].starts_with("rem") {
            if let Ok(value) = declarations[number_start..index].parse::<f32>() {
                output.push_str(&format!("{}px", value * 16.0));
                index += 3;
                continue;
            }
        }
        index = number_start + 1;
        output.push(bytes[number_start] as char);
    }
    output
}

/// Converts a supported CSS length to pixels; percentages and `normal` are omitted.
pub fn css_length(value: &str) -> Option<f32> {
    let value = value.trim();
    if value == "normal" {
        return None;
    }
    if value == "0" {
        return Some(0.0);
    }
    if let Some(value) = value.strip_suffix("rem") {
        return value.trim().parse::<f32>().ok().map(|value| value * 16.0);
    }
    value
        .strip_suffix("px")
        .and_then(|value| value.trim().parse().ok())
}
