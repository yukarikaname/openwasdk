//! The element model: a closed [`Kind`], a store of named properties and children.
//!
//! A type is a variant, not a trait object. Layout is then one match per pass in
//! [`crate::layout`] instead of a vtable call, and adding a type is a variant and
//! one arm. The store is a `BTreeMap` keyed by the XAML property name, so a
//! property that layout does not understand still survives a round trip from a
//! future XBF or XAML reader.
//!
//! Defaults follow `CFrameworkElement::DefaultLayoutProperties` in
//! `dxaml/xcp/core/core/elements/FrameworkElement.cpp`: alignment is `Stretch`,
//! margin is zero, and `Width`/`Height` are `Auto`. `FontSize` defaults to 14,
//! the value of `Control.FontSize` in the theme.

use std::collections::BTreeMap;

/// A property name, exactly as XAML spells it. Attached properties keep their
/// owner prefix, for example `Grid.Row`.
pub type PropertyId = &'static str;

const WIDTH: PropertyId = "Width";
const HEIGHT: PropertyId = "Height";
const MARGIN: PropertyId = "Margin";
const PADDING: PropertyId = "Padding";
const HORIZONTAL_ALIGNMENT: PropertyId = "HorizontalAlignment";
const VERTICAL_ALIGNMENT: PropertyId = "VerticalAlignment";
const BACKGROUND: PropertyId = "Background";
const ORIENTATION: PropertyId = "Orientation";
const SPACING: PropertyId = "Spacing";
const TEXT: PropertyId = "Text";
const FONT_SIZE: PropertyId = "FontSize";
const GRID_ROW: PropertyId = "Grid.Row";
const GRID_COLUMN: PropertyId = "Grid.Column";
const GRID_ROWS: PropertyId = "Grid.RowDefinitions";
const GRID_COLUMNS: PropertyId = "Grid.ColumnDefinitions";

const DEFAULT_FONT_SIZE: f32 = 14.0;

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
    HorizontalAlignment(HorizontalAlignment),
    VerticalAlignment(VerticalAlignment),
    Orientation(Orientation),
    Color([f32; 4]),
    Text(String),
    GridDefinitions(Vec<GridLength>),
}

/// The element types this layer can lay out. The list is closed on purpose:
/// the port will add variants one at a time, and every layout pass has to make
/// a decision for each one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    StackPanel,
    Border,
    Grid,
    TextBlock,
}

impl Kind {
    /// The WinRT name used by XBF and `IXamlMetadataProvider`.
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::StackPanel => "Microsoft.UI.Xaml.Controls.StackPanel",
            Self::Border => "Microsoft.UI.Xaml.Controls.Border",
            Self::Grid => "Microsoft.UI.Xaml.Controls.Grid",
            Self::TextBlock => "Microsoft.UI.Xaml.Controls.TextBlock",
        }
    }

    /// Accepts the full name or the short XAML name.
    pub fn from_type_name(name: &str) -> Option<Self> {
        match name {
            "StackPanel" | "Microsoft.UI.Xaml.Controls.StackPanel" => Some(Self::StackPanel),
            "Border" | "Microsoft.UI.Xaml.Controls.Border" => Some(Self::Border),
            "Grid" | "Microsoft.UI.Xaml.Controls.Grid" => Some(Self::Grid),
            "TextBlock" | "Microsoft.UI.Xaml.Controls.TextBlock" => Some(Self::TextBlock),
            _ => None,
        }
    }
}

/// One node of the XAML tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    kind: Kind,
    properties: BTreeMap<PropertyId, Value>,
    children: Vec<Element>,
}

impl Element {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            properties: BTreeMap::new(),
            children: Vec::new(),
        }
    }

    pub fn stack_panel(orientation: Orientation) -> Self {
        let mut element = Self::new(Kind::StackPanel);
        element.set_orientation(orientation);
        element
    }

    pub fn border() -> Self {
        Self::new(Kind::Border)
    }

    pub fn grid() -> Self {
        Self::new(Kind::Grid)
    }

    pub fn text_block(text: impl Into<String>) -> Self {
        let mut element = Self::new(Kind::TextBlock);
        element.set_text(text);
        element
    }

    pub const fn kind(&self) -> Kind {
        self.kind
    }

    pub fn type_name(&self) -> &'static str {
        self.kind.type_name()
    }

    fn value(&self, id: PropertyId) -> Option<&Value> {
        self.properties.get(id)
    }

    /// Read a stored value by name, for diagnostics and for the XBF adapter.
    pub fn get(&self, id: PropertyId) -> Option<&Value> {
        self.value(id)
    }

    /// Store a value by name. Used by the future XBF/XAML readers when the
    /// property is not one of the typed setters below.
    pub fn set(&mut self, id: PropertyId, value: Value) {
        self.properties.insert(id, value);
    }

    pub fn width(&self) -> Length {
        match self.value(WIDTH) {
            Some(Value::Length(length)) => *length,
            _ => Length::Auto,
        }
    }

    pub fn set_width(&mut self, width: impl Into<Length>) {
        self.set(WIDTH, Value::Length(width.into()));
    }

    pub fn height(&self) -> Length {
        match self.value(HEIGHT) {
            Some(Value::Length(length)) => *length,
            _ => Length::Auto,
        }
    }

    pub fn set_height(&mut self, height: impl Into<Length>) {
        self.set(HEIGHT, Value::Length(height.into()));
    }

    pub fn with_height(self, height: impl Into<Length>) -> Self {
        let mut element = self;
        element.set_height(height);
        element
    }

    pub fn margin(&self) -> Thickness {
        match self.value(MARGIN) {
            Some(Value::Thickness(thickness)) => *thickness,
            _ => Thickness::ZERO,
        }
    }

    pub fn set_margin(&mut self, margin: impl Into<Thickness>) {
        self.set(MARGIN, Value::Thickness(margin.into()));
    }

    pub fn with_margin(self, margin: impl Into<Thickness>) -> Self {
        let mut element = self;
        element.set_margin(margin);
        element
    }

    /// `Border.Padding`, and also `StackPanel.Padding` and `Grid.Padding`.
    pub fn padding(&self) -> Thickness {
        match self.value(PADDING) {
            Some(Value::Thickness(thickness)) => *thickness,
            _ => Thickness::ZERO,
        }
    }

    pub fn set_padding(&mut self, padding: impl Into<Thickness>) {
        self.set(PADDING, Value::Thickness(padding.into()));
    }

    pub fn with_padding(self, padding: impl Into<Thickness>) -> Self {
        let mut element = self;
        element.set_padding(padding);
        element
    }

    pub fn horizontal_alignment(&self) -> HorizontalAlignment {
        match self.value(HORIZONTAL_ALIGNMENT) {
            Some(Value::HorizontalAlignment(alignment)) => *alignment,
            _ => HorizontalAlignment::Stretch,
        }
    }

    pub fn set_horizontal_alignment(&mut self, alignment: HorizontalAlignment) {
        self.set(HORIZONTAL_ALIGNMENT, Value::HorizontalAlignment(alignment));
    }

    pub fn vertical_alignment(&self) -> VerticalAlignment {
        match self.value(VERTICAL_ALIGNMENT) {
            Some(Value::VerticalAlignment(alignment)) => *alignment,
            _ => VerticalAlignment::Stretch,
        }
    }

    pub fn set_vertical_alignment(&mut self, alignment: VerticalAlignment) {
        self.set(VERTICAL_ALIGNMENT, Value::VerticalAlignment(alignment));
    }

    /// Straight, unpremultiplied colour in linear light; `None` paints nothing.
    pub fn background(&self) -> Option<[f32; 4]> {
        match self.value(BACKGROUND) {
            Some(Value::Color(color)) => Some(*color),
            _ => None,
        }
    }

    pub fn set_background(&mut self, color: [f32; 4]) {
        self.set(BACKGROUND, Value::Color(color));
    }

    pub fn with_background(self, color: [f32; 4]) -> Self {
        let mut element = self;
        element.set_background(color);
        element
    }

    pub fn orientation(&self) -> Orientation {
        match self.value(ORIENTATION) {
            Some(Value::Orientation(orientation)) => *orientation,
            _ => Orientation::Vertical,
        }
    }

    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.set(ORIENTATION, Value::Orientation(orientation));
    }

    pub fn spacing(&self) -> f32 {
        match self.value(SPACING) {
            Some(Value::Number(spacing)) => spacing.max(0.0),
            _ => 0.0,
        }
    }

    pub fn set_spacing(&mut self, spacing: f32) {
        self.set(SPACING, Value::Number(spacing.max(0.0)));
    }

    pub fn text(&self) -> &str {
        match self.value(TEXT) {
            Some(Value::Text(text)) => text,
            _ => "",
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.set(TEXT, Value::Text(text.into()));
    }

    pub fn font_size(&self) -> f32 {
        match self.value(FONT_SIZE) {
            Some(Value::Number(size)) => size.max(0.0),
            _ => DEFAULT_FONT_SIZE,
        }
    }

    pub fn set_font_size(&mut self, size: f32) {
        self.set(FONT_SIZE, Value::Number(size.max(0.0)));
    }

    pub fn with_font_size(self, size: f32) -> Self {
        let mut element = self;
        element.set_font_size(size);
        element
    }

    pub fn grid_row(&self) -> usize {
        index_value(self.value(GRID_ROW))
    }

    pub fn set_grid_row(&mut self, row: usize) {
        self.set(GRID_ROW, Value::Number(row as f32));
    }

    pub fn grid_column(&self) -> usize {
        index_value(self.value(GRID_COLUMN))
    }

    pub fn set_grid_column(&mut self, column: usize) {
        self.set(GRID_COLUMN, Value::Number(column as f32));
    }

    pub fn with_grid_column(self, column: usize) -> Self {
        let mut element = self;
        element.set_grid_column(column);
        element
    }

    pub fn rows(&self) -> &[GridLength] {
        definitions(self.value(GRID_ROWS))
    }

    pub fn set_rows(&mut self, rows: impl IntoIterator<Item = GridLength>) {
        self.set(
            GRID_ROWS,
            Value::GridDefinitions(rows.into_iter().collect()),
        );
    }

    pub fn columns(&self) -> &[GridLength] {
        definitions(self.value(GRID_COLUMNS))
    }

    pub fn set_columns(&mut self, columns: impl IntoIterator<Item = GridLength>) {
        self.set(
            GRID_COLUMNS,
            Value::GridDefinitions(columns.into_iter().collect()),
        );
    }

    pub fn children(&self) -> &[Element] {
        &self.children
    }

    pub fn push(&mut self, child: Element) -> &mut Self {
        self.children.push(child);
        self
    }
}

fn index_value(value: Option<&Value>) -> usize {
    match value {
        Some(Value::Number(index)) if *index >= 0.0 => *index as usize,
        _ => 0,
    }
}

fn definitions(value: Option<&Value>) -> &[GridLength] {
    match value {
        Some(Value::GridDefinitions(definitions)) => definitions,
        _ => &[],
    }
}
