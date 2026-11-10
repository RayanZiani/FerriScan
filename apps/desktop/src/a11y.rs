#![allow(clippy::large_types_passed_by_value)] // `Theme` is a Copy palette, see shell.rs

//! Accessibility helpers: every clickable element gets a role, an accessible name, a tab stop
//! and a visible focus ring, so it works with a screen reader and from the keyboard alone.
//!
//! GPUI activates a focused element's `on_click` handler with Enter or Space, so no separate
//! key handler is needed for buttons.

use gpui::{Role, SharedString, StatefulInteractiveElement, Styled as _};

use crate::theme::Theme;

/// Makes an element reachable and understandable without a mouse.
pub trait Pressable: StatefulInteractiveElement + Sized {
    /// Gives the element `role` and the accessible name `label`, adds it to the Tab order and
    /// inverts its colors while it has keyboard focus (no layout shift).
    fn pressable(self, role: Role, label: impl Into<SharedString>, th: Theme) -> Self {
        self.role(role)
            .aria_label(label)
            .tab_index(0)
            .focus_visible(move |style| style.bg(th.text).text_color(th.bg))
    }
}

impl<T: StatefulInteractiveElement> Pressable for T {}
