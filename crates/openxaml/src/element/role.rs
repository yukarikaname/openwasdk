//! What a kind means to layout, kept apart from the kind list.
//!
//! Layout matches a role, not two hundred kinds: adding a control is a name in one table
//! instead of an arm in every pass.

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
    /// Items left to right, wrapping when the line is full.
    Wrap,
    /// Equal cells, counted or squared off from the child count.
    Uniform,
    /// One child scaled into the whole slot.
    Viewbox,
    /// Equal cells with row and column spans, wrapping by column.
    VariableSize,
    /// Children anchored to the panel's edges or to each other by name.
    Relative,
}
