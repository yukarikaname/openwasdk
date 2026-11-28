//! What a kind means to layout, kept apart from the kind list.
//!
//! Layout matches a role, not two hundred kinds: adding a control is a name in one table
//! instead of an arm in every pass.

use super::Kind;

/// What a kind means to layout: a panel, a content host, text, or nothing visual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A panel with its own measure/arrange.
    Panel(Panel),
    /// One child inside border thickness and padding.
    ContentHost,
    /// Paints a string; children are not laid out.
    Text,
    /// Nothing below it is arranged, painted or composed.
    NonVisual,
}

/// The panels this crate actually lays out; every other panel is a content host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Stack,
    Grid,
    Canvas,
}

impl Kind {
    /// The layout behaviour of this kind.
    pub const fn role(self) -> Role {
        match self {
            Kind::TextBlock => Role::Text,
            Kind::StackPanel => Role::Panel(Panel::Stack),
            Kind::Grid => Role::Panel(Panel::Grid),
            Kind::Canvas => Role::Panel(Panel::Canvas),
            Kind::ResourceDictionary
            | Kind::Application
            | Kind::Style
            | Kind::DataTemplate
            | Kind::Paragraph
            | Kind::Run
            | Kind::Span
            | Kind::MicaBackdrop
            | Kind::SystemBackdropElement
            | Kind::FontIcon
            | Kind::Double
            | Kind::String
            | Kind::Boolean
            | Kind::Thickness
            | Kind::Ellipse
            | Kind::Rectangle
            | Kind::Line
            | Kind::ToolTip
            | Kind::Binding
            | Kind::Flyout
            | Kind::ImplicitAnimationSet
            | Kind::MenuFlyoutSeparator
            | Kind::ImageIconSource
            | Kind::UniformGridLayout
            | Kind::SymbolIconSource
            | Kind::CommandBarFlyout => Role::NonVisual,
            _ => Role::ContentHost,
        }
    }
}
