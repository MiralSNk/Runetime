//! Player-facing strings by key. Key naming follows `docs/GLOSSARY.md`.

/// Returns the string for `key`, or `None` if there is no such key.
///
/// # Examples
///
/// ```
/// assert_eq!(loc::get("window.title"), Some("Runetime"));
/// assert_eq!(loc::get("no.such.key"), None);
/// ```
pub fn get(key: &str) -> Option<&'static str> {
    match key {
        "window.title" => Some("Runetime"),
        _ => None,
    }
}
