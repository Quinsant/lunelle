/// Checks a property against the default standard module registry.
pub fn is_known_property(property: &str) -> bool {
    crate::CssModuleRegistry::standard().is_known(property)
}
