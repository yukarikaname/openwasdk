//! Values the property store can hold.
//!
//! The variants are the property types the layout passes read: a length, a
//! thickness, an alignment, an orientation, a colour, a text run and grid
//! tracks. A corner radius and a type name are carried before any pass reads
//! them, so a loader does not have to drop what it read.

use std::borrow::Cow;

/// A length is either automatic (the content decides) or a pixel count.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    Auto,
    Pixels(f32),
}

impl From<f32> for Length {
    fn from(pixels: f32) -> Self {
        Self::Pixels(pixels)
    }
}

/// A distance on all four sides, in pixels. `left`/`top` come first, as in XAML.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Thickness {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Thickness {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// The same distance on every side, the XAML one-number form.
    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }

    pub const fn horizontal(self) -> f32 {
        self.left + self.right
    }

    pub const fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

impl From<f32> for Thickness {
    fn from(value: f32) -> Self {
        Self::all(value)
    }
}

/// `CornerRadius`: the four corners of a `Border`, clockwise from the top left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadius {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// The same radius on every corner, the XAML one-number form.
    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }
}

impl From<f32> for CornerRadius {
    fn from(value: f32) -> Self {
        Self::all(value)
    }
}

/// `HorizontalAlignment`: what happens in the slot the parent handed over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
    #[default]
    Stretch,
}

/// `VerticalAlignment`, the other axis of the same idea.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
    #[default]
    Stretch,
}

/// `Orientation` of a `StackPanel`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    #[default]
    Vertical,
}

/// One `Grid` row or column. A star carries its weight, not a size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridLength {
    Auto,
    Star(f32),
    Pixels(f32),
}

impl Default for GridLength {
    fn default() -> Self {
        Self::Star(1.0)
    }
}

impl GridLength {
    pub const fn star(weight: f32) -> Self {
        Self::Star(weight)
    }

    pub const fn pixels(pixels: f32) -> Self {
        Self::Pixels(pixels)
    }
}

/// A value in the property store. The variants are the property types layout
/// knows about; anything else stays a name without a typed reader.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f32),
    Length(Length),
    Thickness(Thickness),
    CornerRadius(CornerRadius),
    HorizontalAlignment(HorizontalAlignment),
    VerticalAlignment(VerticalAlignment),
    Orientation(Orientation),
    /// Straight, unpremultiplied colour in linear light.
    Color([f32; 4]),
    /// A text run. `Cow` keeps the empty default in a static descriptor.
    Text(Cow<'static, str>),
    /// Grid tracks. `Cow` keeps the empty default in a static descriptor.
    GridDefinitions(Cow<'static, [GridLength]>),
    Bool(bool),
    /// A resolved type name carried for a property that asks for a type.
    TypeName(String),
}

impl Value {
    /// Build a text value from anything the loader or a setter has.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(Cow::Owned(text.into()))
    }
}
