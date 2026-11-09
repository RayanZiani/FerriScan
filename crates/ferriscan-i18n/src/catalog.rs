//! Message catalogue for the native UI.
//!
//! English is compiled in and is the source of truth; other locales are parsed
//! from the same `key = value` subset of Fluent and layered on top, so a partial
//! translation never shows a raw key. Plurals are not supported yet: messages
//! that need them use a neutral form (`{count} items`).

use std::collections::HashMap;

const EN: &str = include_str!("../locales/en.ftl");

/// Active message table with English fallback.
#[derive(Clone, Debug)]
pub struct Catalog {
    english: HashMap<String, String>,
    active: HashMap<String, String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

impl Catalog {
    /// Builds a catalogue showing English.
    #[must_use]
    pub fn new() -> Self {
        Self {
            english: parse(EN),
            active: HashMap::new(),
        }
    }

    /// Layers a translated `.ftl` source over English.
    pub fn load(&mut self, source: &str) {
        self.active = parse(source);
    }

    /// Looks up `key`, falling back to English and then to the key itself.
    #[must_use]
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.active
            .get(key)
            .or_else(|| self.english.get(key))
            .map_or(key, String::as_str)
    }

    /// Looks up `key` and substitutes `{name}` placeholders from `args`.
    #[must_use]
    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut out = self.t(key).to_string();
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }

    /// Message for a stable engine error code (`E_PERM`, ...).
    #[must_use]
    pub fn error(&self, code: &str) -> String {
        self.t(&format!("error.{code}")).to_string()
    }
}

fn parse(source: &str) -> HashMap<String, String> {
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_loaded_and_interpolates() {
        let c = Catalog::new();
        assert_eq!(c.t("app.name"), "FerriScan");
        assert_eq!(
            c.t_args("volumes.free_of", &[("free", "1 GiB"), ("total", "2 GiB")]),
            "1 GiB free of 2 GiB"
        );
    }

    #[test]
    fn partial_locale_falls_back_then_to_key() {
        let mut c = Catalog::new();
        c.load("volumes.title = Volumes FR\n");
        assert_eq!(c.t("volumes.title"), "Volumes FR");
        assert_eq!(c.t("volumes.scan"), "Scan");
        assert_eq!(c.t("no.such.key"), "no.such.key");
    }

    #[test]
    fn error_codes_resolve() {
        assert_eq!(Catalog::new().error("E_PERM"), "Permission denied");
    }
}
