//! Optional command-line switches, used for screenshots and smoke tests.
//!
//! They only affect the running session: nothing here is written to the settings file.

use std::path::PathBuf;

use ferriscan_persist::ThemeMode;

/// `ferriscan-app [PATH] [--treemap] [--theme light|dark|contrast] [--lang TAG]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    /// Folder or drive to scan right after the window opens.
    pub scan: Option<PathBuf>,
    /// Show the treemap instead of the table when the scan finishes.
    pub treemap: bool,
    /// Theme for this session only.
    pub theme: Option<ThemeMode>,
    /// Language tag for this session only (`en`, `fr`, `zh-CN`).
    pub lang: Option<String>,
}

impl Launch {
    /// Parses arguments (without the program name); unknown switches are ignored.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut launch = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--treemap" => launch.treemap = true,
                "--theme" => {
                    launch.theme = args.next().and_then(|v| match v.as_str() {
                        "light" => Some(ThemeMode::Light),
                        "dark" => Some(ThemeMode::Dark),
                        "contrast" => Some(ThemeMode::HighContrast),
                        _ => None,
                    });
                }
                "--lang" => launch.lang = args.next(),
                other if !other.starts_with('-') => launch.scan = Some(PathBuf::from(other)),
                _ => {}
            }
        }
        launch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Launch {
        Launch::parse(args.iter().map(|a| (*a).to_string()))
    }

    #[test]
    fn switches_are_read_and_unknown_ones_ignored() {
        let l = parse(&[
            "data",
            "--treemap",
            "--theme",
            "dark",
            "--lang",
            "fr",
            "--wat",
        ]);
        assert_eq!(l.scan, Some(PathBuf::from("data")));
        assert!(l.treemap);
        assert_eq!(l.theme, Some(ThemeMode::Dark));
        assert_eq!(l.lang.as_deref(), Some("fr"));
        assert_eq!(parse(&[]), Launch::default());
        assert_eq!(parse(&["--theme", "nope"]).theme, None);
    }
}
