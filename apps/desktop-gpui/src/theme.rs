//! Design tokens (mirrors `apps/desktop/src/app.css`) resolved to GPUI colors.

use ferriscan_persist::ThemeMode;
use gpui::{Rgba, WindowAppearance};

/// Colors for one appearance. High-contrast is tracked as a follow-up (a11y checklist).
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg: Rgba,
    pub surface: Rgba,
    pub surface_2: Rgba,
    pub surface_3: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub text_inverse: Rgba,
    pub accent: Rgba,
    pub danger: Rgba,
    /// Categorical colors for file families (12 hues).
    pub chart: [Rgba; 12],
}

impl Theme {
    const LIGHT: Self = Self {
        bg: hex(0xf6f7f9),
        surface: hex(0xffffff),
        surface_2: hex(0xeef0f4),
        surface_3: hex(0xe4e7ec),
        border: hex(0xd7dbe2),
        text: hex(0x14171c),
        text_muted: hex(0x5b6472),
        text_inverse: hex(0xffffff),
        accent: hex(0x2563eb),
        danger: hex(0xd92d20),
        chart: [
            hex(0x2563eb),
            hex(0x7c3aed),
            hex(0xdb2777),
            hex(0xea580c),
            hex(0xca8a04),
            hex(0x16a34a),
            hex(0x0891b2),
            hex(0x4f46e5),
            hex(0xbe123c),
            hex(0x65a30d),
            hex(0x0d9488),
            hex(0x9333ea),
        ],
    };

    const DARK: Self = Self {
        bg: hex(0x0f1115),
        surface: hex(0x151922),
        surface_2: hex(0x1b202b),
        surface_3: hex(0x232936),
        border: hex(0x2a3140),
        text: hex(0xe8ebf0),
        text_muted: hex(0x9aa4b4),
        text_inverse: hex(0x0f1115),
        accent: hex(0x5b8cff),
        danger: hex(0xff6b5e),
        chart: [
            hex(0x5b8cff),
            hex(0xa78bfa),
            hex(0xf472b6),
            hex(0xfb923c),
            hex(0xfacc15),
            hex(0x4ade80),
            hex(0x22d3ee),
            hex(0x818cf8),
            hex(0xfb7185),
            hex(0xa3e635),
            hex(0x2dd4bf),
            hex(0xc084fc),
        ],
    };

    /// Picks the palette for the user's preference and the window's OS appearance.
    #[must_use]
    pub fn resolve(mode: ThemeMode, os: WindowAppearance) -> Self {
        let dark = match mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => {
                matches!(os, WindowAppearance::Dark | WindowAppearance::VibrantDark)
            }
        };
        if dark { Self::DARK } else { Self::LIGHT }
    }
}

/// `gpui::rgb` is not `const`, so tokens build the struct directly.
const fn hex(value: u32) -> Rgba {
    Rgba {
        r: ((value >> 16) & 0xff) as f32 / 255.0,
        g: ((value >> 8) & 0xff) as f32 / 255.0,
        b: (value & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_overrides_os_appearance() {
        let dark = Theme::resolve(ThemeMode::Dark, WindowAppearance::Light);
        assert_eq!(dark.bg, hex(0x0f1115));
        let light = Theme::resolve(ThemeMode::Light, WindowAppearance::Dark);
        assert_eq!(light.bg, hex(0xf6f7f9));
        let sys = Theme::resolve(ThemeMode::System, WindowAppearance::Dark);
        assert_eq!(sys.bg, hex(0x0f1115));
    }
}
