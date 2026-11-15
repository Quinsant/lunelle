use lunelle_css::resolve_vars;
use std::collections::HashMap;

#[test]
fn resolves_nested_fallbacks_and_defined_properties() {
    let properties = HashMap::from([("--accent".to_owned(), "teal".to_owned())]);
    assert_eq!(
        resolve_vars("var(--missing, var(--accent, red))", &properties, 0),
        Some("teal".to_owned())
    );
}

#[test]
fn rejects_unresolved_properties_and_excessive_recursion() {
    let properties = HashMap::new();
    assert_eq!(resolve_vars("var(--missing)", &properties, 0), None);
    assert_eq!(resolve_vars("var(--missing, red)", &properties, 16), None);
}
