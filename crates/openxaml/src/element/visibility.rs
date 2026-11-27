//! The `Visibility` accessors.
//!
//! `CUIElement::GetIsLayoutSuspended` is `IsCollapsed`
//! (`dxaml/xcp/core/core/elements/uielement.h`); a collapsed element never
//! enters `MeasureCore` and its arrange does not advance a parent's cursor.
//! WinUI's `Visibility` has only `Visible` and `Collapsed`, so `is_visible` is
//! the negation of collapsed; there is no hidden-but-laid-out state.

use super::Element;
use crate::property;
use crate::value::{Value, Visibility};

impl Element {
    /// `UIElement.Visibility`, default `Visible`.
    pub fn visibility(&self) -> Visibility {
        match self.get(&property::VISIBILITY) {
            Value::Visibility(visibility) => *visibility,
            _ => Visibility::Visible,
        }
    }

    pub fn set_visibility(&mut self, visibility: Visibility) {
        self.store(&property::VISIBILITY, Value::Visibility(visibility));
    }

    pub fn with_visibility(self, visibility: Visibility) -> Self {
        let mut element = self;
        element.set_visibility(visibility);
        element
    }

    /// The predicate `CStackPanel::MeasureOverride` and `CStackPanel::ArrangeOverride`
    /// call as `IsVisible`.
    pub fn is_visible(&self) -> bool {
        self.visibility() != Visibility::Collapsed
    }
}
