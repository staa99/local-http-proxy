/// Validates that a source name is a valid DNS label and URL path segment.
pub fn is_valid_source_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Must not start or end with a hyphen (DNS label rule).
    let first_char_ok = name.chars().next().map_or(false, |c| c.is_alphanumeric());
    let last_char_ok = name.chars().last().map_or(false, |c| c.is_alphanumeric());

    if !first_char_ok || !last_char_ok {
        return false;
    }

    // Must only contain alphanumeric characters or hyphens.
    // This check also implicitly forbids '.' and '/'.
    name.chars().all(|c| c.is_alphanumeric() || c == '-')
}
