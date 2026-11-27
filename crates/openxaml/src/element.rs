//! The element model: a closed [`Kind`], a store of dependency properties and
//! children.
//!
//! A type is a variant, not a trait object. Layout is then one match per pass in
//! [`crate::layout`] instead of a vtable call, and adding a type is a variant and
//! one arm. The store is keyed by the [`crate::property::Property`] descriptor,
//! so a value can only enter with the type that property declares. A property
//! the layout does not understand still survives a round trip from a future XBF
//! or XAML reader.
//!
//! Defaults are in the descriptor table, following
//! `CFrameworkElement::DefaultLayoutProperties` in
//! `dxaml/xcp/core/core/elements/FrameworkElement.cpp`: alignment is `Stretch`,
//! margin is zero, and `Width`/`Height` are `Auto`. `FontSize` defaults to 14,
//! the value of `Control.FontSize` in the theme.
//!
//! A read follows the effective-value chain documented in
//! [`crate::property`]: local value, then inherited value, then default. The
//! tree owns its children, so inheritance is materialised downward when a
//! parent is set or gains a child; a local value stops the propagation at that
//! element and restarts it with the local value for its descendants.

use std::collections::BTreeMap;

use crate::property::{self, Property, PropertyError, PropertyId};

mod grid;
mod visibility;

pub use crate::value::{
    CornerRadius, GridLength, GridTrack, HorizontalAlignment, Length, Orientation, Thickness,
    Value, VerticalAlignment, Visibility,
};

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
    /// Values set on this element, by property name.
    properties: BTreeMap<PropertyId, Value>,
    /// Values received from an ancestor for an inheriting property.
    inherited: BTreeMap<PropertyId, Value>,
    children: Vec<Element>,
    name: Option<String>,
    key: Option<String>,
    /// Set by any mutation that changes measure or arrange; cleared by
    /// [`crate::layout::arrange`].
    layout_dirty: bool,
}

impl Element {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            properties: BTreeMap::new(),
            inherited: BTreeMap::new(),
            children: Vec::new(),
            name: None,
            key: None,
            layout_dirty: true,
        }
    }

    /// The `x:Name` the source gave this element, kept apart from the store.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = Some(name.into());
    }

    /// The `x:Key` of a dictionary resource, kept apart from the store.
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    pub fn set_key(&mut self, key: impl Into<String>) {
        self.key = Some(key.into());
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

    /// The value set on this element, ignoring inheritance and defaults. Used
    /// by diagnostics and by tests that ask whether a source was stored.
    pub fn local(&self, name: &str) -> Option<&Value> {
        self.properties.get(name)
    }

    fn source(&self, property: &Property) -> Option<&Value> {
        self.properties
            .get(property.name)
            .or_else(|| self.inherited.get(property.name))
    }

    /// The effective value: local, then inherited, then the descriptor default.
    pub fn get(&self, property: &'static Property) -> &Value {
        self.source(property).unwrap_or(property.default)
    }

    /// Store a value after checking it against the descriptor. A wrong type is
    /// an error and leaves the element unchanged.
    pub fn set(&mut self, property: &'static Property, value: Value) -> Result<(), PropertyError> {
        if !property.accepts(&value) {
            return Err(PropertyError::new(property, &value));
        }
        self.store(property, value);
        Ok(())
    }

    /// Store a value the typed setters built for this descriptor. The type is
    /// an invariant of the setter, not user data, so there is no check here.
    fn store(&mut self, property: &'static Property, value: Value) {
        self.properties.insert(property.name, value.clone());
        if property.inherits {
            for child in &mut self.children {
                child.inherit_value(property, &value);
            }
        }
        self.mark_subtree_dirty();
    }

    /// Push an ancestor value down the tree. A local value wins and is the one
    /// passed further, which makes the nearest explicit ancestor win.
    fn inherit_value(&mut self, property: &'static Property, value: &Value) {
        let effective = match self.properties.get(property.name) {
            Some(local) => local.clone(),
            None => {
                self.inherited.insert(property.name, value.clone());
                value.clone()
            }
        };
        self.layout_dirty = true;
        for child in &mut self.children {
            child.inherit_value(property, &effective);
        }
    }

    fn mark_subtree_dirty(&mut self) {
        self.layout_dirty = true;
        for child in &mut self.children {
            child.mark_subtree_dirty();
        }
    }

    /// Called by arrange once the subtree has been measured and placed.
    pub(crate) fn mark_layout_clean(&mut self) {
        self.layout_dirty = false;
        for child in &mut self.children {
            child.mark_layout_clean();
        }
    }

    /// Whether this element or any descendant changed since the last arrange.
    /// The upward walk is what makes an ancestor notice a leaf's set.
    pub fn needs_layout(&self) -> bool {
        self.layout_dirty || self.children.iter().any(Element::needs_layout)
    }

    pub fn width(&self) -> Length {
        match self.get(&property::WIDTH) {
            Value::Length(length) => *length,
            _ => Length::Auto,
        }
    }

    pub fn set_width(&mut self, width: impl Into<Length>) {
        self.store(&property::WIDTH, Value::Length(width.into()));
    }

    pub fn height(&self) -> Length {
        match self.get(&property::HEIGHT) {
            Value::Length(length) => *length,
            _ => Length::Auto,
        }
    }

    pub fn set_height(&mut self, height: impl Into<Length>) {
        self.store(&property::HEIGHT, Value::Length(height.into()));
    }

    pub fn with_height(self, height: impl Into<Length>) -> Self {
        let mut element = self;
        element.set_height(height);
        element
    }

    pub fn margin(&self) -> Thickness {
        match self.get(&property::MARGIN) {
            Value::Thickness(thickness) => *thickness,
            _ => Thickness::ZERO,
        }
    }

    pub fn set_margin(&mut self, margin: impl Into<Thickness>) {
        self.store(&property::MARGIN, Value::Thickness(margin.into()));
    }

    pub fn with_margin(self, margin: impl Into<Thickness>) -> Self {
        let mut element = self;
        element.set_margin(margin);
        element
    }

    /// `Border.Padding`, and also `StackPanel.Padding` and `Grid.Padding`.
    pub fn padding(&self) -> Thickness {
        match self.get(&property::PADDING) {
            Value::Thickness(thickness) => *thickness,
            _ => Thickness::ZERO,
        }
    }

    pub fn set_padding(&mut self, padding: impl Into<Thickness>) {
        self.store(&property::PADDING, Value::Thickness(padding.into()));
    }

    pub fn with_padding(self, padding: impl Into<Thickness>) -> Self {
        let mut element = self;
        element.set_padding(padding);
        element
    }

    /// `Border.BorderThickness`; layout subtracts it as chrome.
    pub fn border_thickness(&self) -> Thickness {
        match self.get(&property::BORDER_THICKNESS) {
            Value::Thickness(thickness) => *thickness,
            _ => Thickness::ZERO,
        }
    }

    pub fn set_border_thickness(&mut self, thickness: impl Into<Thickness>) {
        self.store(
            &property::BORDER_THICKNESS,
            Value::Thickness(thickness.into()),
        );
    }

    pub fn with_border_thickness(self, thickness: impl Into<Thickness>) -> Self {
        let mut element = self;
        element.set_border_thickness(thickness);
        element
    }

    /// `Border.CornerRadius`. Layout never reads it; drawing does.
    pub fn corner_radius(&self) -> CornerRadius {
        match self.get(&property::CORNER_RADIUS) {
            Value::CornerRadius(radius) => *radius,
            _ => CornerRadius::ZERO,
        }
    }

    pub fn set_corner_radius(&mut self, radius: impl Into<CornerRadius>) {
        self.store(&property::CORNER_RADIUS, Value::CornerRadius(radius.into()));
    }

    pub fn with_corner_radius(self, radius: impl Into<CornerRadius>) -> Self {
        let mut element = self;
        element.set_corner_radius(radius);
        element
    }

    pub fn horizontal_alignment(&self) -> HorizontalAlignment {
        match self.get(&property::HORIZONTAL_ALIGNMENT) {
            Value::HorizontalAlignment(alignment) => *alignment,
            _ => HorizontalAlignment::Stretch,
        }
    }

    pub fn set_horizontal_alignment(&mut self, alignment: HorizontalAlignment) {
        self.store(
            &property::HORIZONTAL_ALIGNMENT,
            Value::HorizontalAlignment(alignment),
        );
    }

    pub fn vertical_alignment(&self) -> VerticalAlignment {
        match self.get(&property::VERTICAL_ALIGNMENT) {
            Value::VerticalAlignment(alignment) => *alignment,
            _ => VerticalAlignment::Stretch,
        }
    }

    pub fn set_vertical_alignment(&mut self, alignment: VerticalAlignment) {
        self.store(
            &property::VERTICAL_ALIGNMENT,
            Value::VerticalAlignment(alignment),
        );
    }

    /// Straight, unpremultiplied colour in linear light; `None` paints nothing.
    pub fn background(&self) -> Option<[f32; 4]> {
        match self.get(&property::BACKGROUND) {
            Value::Color(color) => Some(*color),
            _ => None,
        }
    }

    pub fn set_background(&mut self, color: [f32; 4]) {
        self.store(&property::BACKGROUND, Value::Color(color));
    }

    pub fn with_background(self, color: [f32; 4]) -> Self {
        let mut element = self;
        element.set_background(color);
        element
    }

    pub fn orientation(&self) -> Orientation {
        match self.get(&property::ORIENTATION) {
            Value::Orientation(orientation) => *orientation,
            _ => Orientation::Vertical,
        }
    }

    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.store(&property::ORIENTATION, Value::Orientation(orientation));
    }

    pub fn spacing(&self) -> f32 {
        match self.get(&property::SPACING) {
            Value::Number(spacing) => spacing.max(0.0),
            _ => 0.0,
        }
    }

    pub fn set_spacing(&mut self, spacing: f32) {
        self.store(&property::SPACING, Value::Number(spacing.max(0.0)));
    }

    pub fn text(&self) -> &str {
        match self.get(&property::TEXT) {
            Value::Text(text) => text,
            _ => "",
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.store(&property::TEXT, Value::text(text));
    }

    pub fn font_size(&self) -> f32 {
        match self.get(&property::FONT_SIZE) {
            Value::Number(size) => size.max(0.0),
            _ => property::DEFAULT_FONT_SIZE,
        }
    }

    pub fn set_font_size(&mut self, size: f32) {
        self.store(&property::FONT_SIZE, Value::Number(size.max(0.0)));
    }

    pub fn with_font_size(self, size: f32) -> Self {
        let mut element = self;
        element.set_font_size(size);
        element
    }

    pub fn grid_row(&self) -> usize {
        index_value(self.get(&property::GRID_ROW))
    }

    pub fn set_grid_row(&mut self, row: usize) {
        self.store(&property::GRID_ROW, Value::Number(row as f32));
    }

    pub fn grid_column(&self) -> usize {
        index_value(self.get(&property::GRID_COLUMN))
    }

    pub fn set_grid_column(&mut self, column: usize) {
        self.store(&property::GRID_COLUMN, Value::Number(column as f32));
    }

    pub fn with_grid_column(self, column: usize) -> Self {
        let mut element = self;
        element.set_grid_column(column);
        element
    }

    pub fn rows(&self) -> &[GridTrack] {
        definitions(self.get(&property::GRID_ROWS))
    }

    pub fn set_rows(&mut self, rows: impl IntoIterator<Item = impl Into<GridTrack>>) {
        self.store(
            &property::GRID_ROWS,
            Value::GridDefinitions(rows.into_iter().map(Into::into).collect::<Vec<_>>().into()),
        );
    }

    pub fn columns(&self) -> &[GridTrack] {
        definitions(self.get(&property::GRID_COLUMNS))
    }

    pub fn set_columns(&mut self, columns: impl IntoIterator<Item = impl Into<GridTrack>>) {
        self.store(
            &property::GRID_COLUMNS,
            Value::GridDefinitions(
                columns
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into(),
            ),
        );
    }

    pub fn children(&self) -> &[Element] {
        &self.children
    }

    pub fn children_mut(&mut self) -> &mut [Element] {
        &mut self.children
    }

    pub fn push(&mut self, mut child: Element) -> &mut Self {
        for &property in property::INHERITED {
            if let Some(value) = self.source(property) {
                child.inherit_value(property, value);
            }
        }
        self.children.push(child);
        self.mark_subtree_dirty();
        self
    }
}

fn index_value(value: &Value) -> usize {
    match value {
        Value::Number(index) if *index >= 0.0 => *index as usize,
        _ => 0,
    }
}

fn definitions(value: &Value) -> &[GridTrack] {
    match value {
        Value::GridDefinitions(definitions) => definitions.as_ref(),
        _ => &[],
    }
}
