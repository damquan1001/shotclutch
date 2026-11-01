//! Canonical environment lookups with inherited compatibility aliases.
use std::ffi::{OsStr, OsString};

fn legacy(key: &OsStr) -> Option<OsString> {
    key.to_str()?.strip_prefix("SHOTCLUTCH_").map(|suffix| OsString::from(format!("FILMCRAFT_{suffix}")))
}

/// Prefer the canonical value, including an empty value, over its legacy alias.
pub fn var_os(key: impl AsRef<OsStr>) -> Option<OsString> {
    lookup(&canonical(key.as_ref()), |key| std::env::var_os(key))
}

fn canonical(key: &OsStr) -> OsString {
    key.to_str().and_then(|s| s.strip_prefix("FILMCRAFT_")).map_or_else(|| key.to_os_string(), |suffix| OsString::from(format!("SHOTCLUTCH_{suffix}")))
}

fn lookup(key: &OsStr, get: impl Fn(&OsStr) -> Option<OsString>) -> Option<OsString> {
    get(key).or_else(|| legacy(key).and_then(|alias| get(&alias)))
}

/// Preserve Unicode errors on a present canonical value rather than hiding them with an alias.
pub fn var(key: impl AsRef<OsStr>) -> Result<String, std::env::VarError> {
    match var_os(key) {
        Some(value) => value.into_string().map_err(std::env::VarError::NotUnicode),
        None => Err(std::env::VarError::NotPresent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asking_by_a_legacy_alias_still_observes_canonical_precedence() {
        for suffix in ["CONTROL_TOKEN", "KOKORO_DIR", "CMUDICT", "FAVORITES_SCREENSHOT"] {
            let old = OsString::from(format!("FILMCRAFT_{suffix}"));
            let new = OsString::from(format!("SHOTCLUTCH_{suffix}"));
            assert_eq!(canonical(&old), new);
            let get = |key: &OsStr| {
                if key == new {
                    Some(OsString::from("new"))
                } else if key == old {
                    Some(OsString::from("old"))
                } else {
                    None
                }
            };
            assert_eq!(lookup(&canonical(&old), get), Some(OsString::from("new")));
        }
    }
    #[test]
    fn canonical_and_empty_values_win() {
        for new in ["new", ""] {
            let get = |key: &OsStr| match key.to_str() {
                Some("SHOTCLUTCH_TEST") => Some(OsString::from(new)),
                Some("FILMCRAFT_TEST") => Some(OsString::from("old")),
                _ => None,
            };
            assert_eq!(lookup(OsStr::new("SHOTCLUTCH_TEST"), get), Some(OsString::from(new)));
        }
    }
    #[test]
    fn absent_canonical_falls_back_without_touching_unrelated_names() {
        let get = |key: &OsStr| (key == "FILMCRAFT_TEST").then(|| OsString::from("old"));
        assert_eq!(lookup(OsStr::new("SHOTCLUTCH_TEST"), get), Some(OsString::from("old")));
        assert_eq!(lookup(OsStr::new("UNRELATED_TEST"), get), None);
    }
}
