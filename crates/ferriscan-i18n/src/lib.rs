//! Locale detection and the list of shipped locales.
//!
//! Translation strings live in `locales/<tag>.ftl` (Fluent `key = value` subset) and are
//! served by [`Catalog`]. This crate also answers "which locale should we open with"
//! and keeps the shipped list in one place.

#![forbid(unsafe_code)]

mod catalog;

pub use catalog::Catalog;

/// Locales shipped in v1.0, in display order.
pub const SHIPPED: &[&str] = &[
    "en", "fr", "de", "es", "it", "pt-BR", "nl", "pl", "ru", "tr", "ja", "zh-CN",
];

/// Fallback locale when nothing else matches.
pub const FALLBACK: &str = "en";

/// Detects the user's preferred locale from the environment.
///
/// Reads `LANG`/`LC_ALL` on Unix and `LANG` on Windows (where the system locale
/// is usually available through the webview anyway). Returns a shipped locale or
/// [`FALLBACK`].
#[must_use]
pub fn detect_system_locale() -> String {
    #[cfg(unix)]
    {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(key) {
                if let Some(tag) = normalize(&value) {
                    return tag;
                }
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(value) = std::env::var("LANG")
            && let Some(tag) = normalize(&value)
        {
            return tag;
        }
    }
    FALLBACK.to_string()
}

/// Normalizes a POSIX locale (`fr_FR.UTF-8`) to a shipped BCP-47 tag.
#[must_use]
pub fn normalize(raw: &str) -> Option<String> {
    let trimmed = raw.split('.').next()?.split('@').next()?.replace('_', "-");
    if trimmed.is_empty() || trimmed == "C" || trimmed == "POSIX" {
        return None;
    }
    let mut parts = trimmed.split('-');
    let language = parts.next()?.to_ascii_lowercase();
    let region = parts.next().map(str::to_ascii_uppercase);

    // Exact match first: `pt-BR` and `zh-CN` are the only region-specific ones.
    if let Some(region) = &region {
        let candidate = format!("{language}-{region}");
        if let Some(exact) = SHIPPED.iter().find(|l| **l == candidate) {
            return Some((*exact).to_string());
        }
    }
    SHIPPED
        .iter()
        .find(|l| l.split('-').next() == Some(language.as_str()))
        .map(|l| (*l).to_string())
}

/// Resolves the locale to use, given an optional user override.
#[must_use]
pub fn resolve(override_tag: Option<&str>) -> String {
    if let Some(tag) = override_tag
        && let Some(normalized) = normalize(tag)
    {
        return normalized;
    }
    detect_system_locale()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_posix_locales() {
        assert_eq!(normalize("fr_FR.UTF-8"), Some("fr".to_string()));
        assert_eq!(normalize("pt_BR.UTF-8"), Some("pt-BR".to_string()));
        assert_eq!(normalize("zh_CN"), Some("zh-CN".to_string()));
        assert_eq!(normalize("ja_JP"), Some("ja".to_string()));
    }

    #[test]
    fn rejects_c_locale() {
        assert_eq!(normalize("C"), None);
        assert_eq!(normalize("POSIX"), None);
    }

    #[test]
    fn unknown_language_falls_back() {
        assert_eq!(normalize("xx_YY"), None);
        assert_eq!(resolve(Some("xx-YY")), detect_system_locale());
    }

    #[test]
    fn every_shipped_locale_is_unique_and_lowercase() {
        let mut seen = std::collections::BTreeSet::new();
        for locale in SHIPPED {
            assert!(seen.insert(*locale), "duplicate {locale}");
            let lang = locale.split('-').next().unwrap_or_default();
            assert_eq!(lang, lang.to_ascii_lowercase());
        }
    }
}
