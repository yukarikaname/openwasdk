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
    Page,
    UserControl,
    Window,
    ContentDialog,
    /* Hosts that hold resources and templates but paint nothing. */
    ResourceDictionary,
    Application,
    Style,
    DataTemplate,
    /* Text inlines: they carry a run's text, not a box to lay out. */
    Paragraph,
    Run,
    Span,
    /* Content controls the Gallery's pages put on screen. */
    Button,
    ScrollViewer,
    TextBox,
    RichTextBlock,
    InfoBar,
    TitleBar,
    SplitButton,
    ContentPresenter,
    PasswordBox,
    SymbolIcon,
    MicaBackdrop,
    SystemBackdropElement,
    /* An icon paints a glyph this renderer has no font for. */
    FontIcon,
    /* Primitive values that appear as dictionary entries or setter values. */
    Double,
    String,
    Boolean,
    Thickness,
    ListView,
    GridView,
    TabView,
    ItemsRepeater,
    ScrollView,
    DatePicker,
    RichEditBox,
    CheckBox,
    Image,
    ComboBox,
    ComboBoxItem,
    ListViewItem,
    GridViewItem,
    ToggleSwitch,
    RadioButton,
    HyperlinkButton,
    RepeatButton,
    ToggleButton,
    /* Shapes: geometry this renderer has no draw path for yet. */
    Ellipse,
    Rectangle,
    Line,
    /* Shown on hover, not laid out with the page. */
    ToolTip,
    /* A markup extension instance, not a box. */
    Binding,
    Expander,
    ProgressRing,
    SelectorBar,
    SelectorBarItem,
    AutoSuggestBox,
    Flyout,
}

impl Kind {
    /// The WinRT name used by XBF and `IXamlMetadataProvider`.
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::StackPanel => "Microsoft.UI.Xaml.Controls.StackPanel",
            Self::Border => "Microsoft.UI.Xaml.Controls.Border",
            Self::Grid => "Microsoft.UI.Xaml.Controls.Grid",
            Self::TextBlock => "Microsoft.UI.Xaml.Controls.TextBlock",
            Self::Page => "Microsoft.UI.Xaml.Controls.Page",
            Self::UserControl => "Microsoft.UI.Xaml.Controls.UserControl",
            Self::Window => "Microsoft.UI.Xaml.Window",
            Self::ContentDialog => "Microsoft.UI.Xaml.Controls.ContentDialog",
            Self::ResourceDictionary => "Microsoft.UI.Xaml.ResourceDictionary",
            Self::Application => "Microsoft.UI.Xaml.Application",
            Self::Style => "Microsoft.UI.Xaml.Style",
            Self::DataTemplate => "Microsoft.UI.Xaml.DataTemplate",
            Self::Paragraph => "Microsoft.UI.Xaml.Documents.Paragraph",
            Self::Run => "Microsoft.UI.Xaml.Documents.Run",
            Self::Span => "Microsoft.UI.Xaml.Documents.Span",
            Self::Button => "Microsoft.UI.Xaml.Controls.Button",
            Self::ScrollViewer => "Microsoft.UI.Xaml.Controls.ScrollViewer",
            Self::TextBox => "Microsoft.UI.Xaml.Controls.TextBox",
            Self::RichTextBlock => "Microsoft.UI.Xaml.Controls.RichTextBlock",
            Self::InfoBar => "Microsoft.UI.Xaml.Controls.InfoBar",
            Self::TitleBar => "Microsoft.UI.Xaml.Controls.TitleBar",
            Self::SplitButton => "Microsoft.UI.Xaml.Controls.SplitButton",
            Self::ContentPresenter => "Microsoft.UI.Xaml.Controls.ContentPresenter",
            Self::PasswordBox => "Microsoft.UI.Xaml.Controls.PasswordBox",
            Self::SymbolIcon => "Microsoft.UI.Xaml.Controls.SymbolIcon",
            Self::MicaBackdrop => "Microsoft.UI.Xaml.Media.MicaBackdrop",
            Self::SystemBackdropElement => "Microsoft.UI.Xaml.Media.SystemBackdropElement",
            Self::FontIcon => "Microsoft.UI.Xaml.Controls.FontIcon",
            Self::Double => "Double",
            Self::String => "String",
            Self::Boolean => "Boolean",
            Self::Thickness => "Thickness",
            Self::ListView => "Microsoft.UI.Xaml.Controls.ListView",
            Self::GridView => "Microsoft.UI.Xaml.Controls.GridView",
            Self::TabView => "Microsoft.UI.Xaml.Controls.TabView",
            Self::ItemsRepeater => "Microsoft.UI.Xaml.Controls.ItemsRepeater",
            Self::ScrollView => "Microsoft.UI.Xaml.Controls.ScrollView",
            Self::DatePicker => "Microsoft.UI.Xaml.Controls.DatePicker",
            Self::RichEditBox => "Microsoft.UI.Xaml.Controls.RichEditBox",
            Self::CheckBox => "Microsoft.UI.Xaml.Controls.CheckBox",
            Self::Image => "Microsoft.UI.Xaml.Controls.Image",
            Self::ComboBox => "Microsoft.UI.Xaml.Controls.ComboBox",
            Self::ComboBoxItem => "Microsoft.UI.Xaml.Controls.ComboBoxItem",
            Self::ListViewItem => "Microsoft.UI.Xaml.Controls.ListViewItem",
            Self::GridViewItem => "Microsoft.UI.Xaml.Controls.GridViewItem",
            Self::ToggleSwitch => "Microsoft.UI.Xaml.Controls.ToggleSwitch",
            Self::RadioButton => "Microsoft.UI.Xaml.Controls.RadioButton",
            Self::HyperlinkButton => "Microsoft.UI.Xaml.Controls.HyperlinkButton",
            Self::RepeatButton => "Microsoft.UI.Xaml.Controls.RepeatButton",
            Self::ToggleButton => "Microsoft.UI.Xaml.Controls.ToggleButton",
            Self::Ellipse => "Microsoft.UI.Xaml.Shapes.Ellipse",
            Self::Rectangle => "Microsoft.UI.Xaml.Shapes.Rectangle",
            Self::Line => "Microsoft.UI.Xaml.Shapes.Line",
            Self::ToolTip => "Microsoft.UI.Xaml.Controls.ToolTip",
            Self::Binding => "Microsoft.UI.Xaml.Data.Binding",
            Self::Expander => "Microsoft.UI.Xaml.Controls.Expander",
            Self::ProgressRing => "Microsoft.UI.Xaml.Controls.ProgressRing",
            Self::SelectorBar => "Microsoft.UI.Xaml.Controls.SelectorBar",
            Self::SelectorBarItem => "Microsoft.UI.Xaml.Controls.SelectorBarItem",
            Self::AutoSuggestBox => "Microsoft.UI.Xaml.Controls.AutoSuggestBox",
            Self::Flyout => "Microsoft.UI.Xaml.Controls.Flyout",
        }
    }

    /// Accepts the full name or the short XAML name.
    pub fn from_type_name(name: &str) -> Option<Self> {
        if let Some(known) = Self::from_exact_name(name) {
            return Some(known);
        }
        /*
         * XBF writes the type under the namespace it was declared in, and the
         * presentation namespace is spelled as a URI in application files. Strip only
         * the WinUI namespaces: a type from the application's own namespace is not a
         * standard control and must not be guessed to be one.
         */
        let prefixes = [
            "Microsoft.UI.Xaml.Controls.",
            "Microsoft.UI.Xaml.Documents.",
            "Microsoft.UI.Xaml.",
            "http://schemas.microsoft.com/client/2007.",
        ];
        prefixes
            .iter()
            .find_map(|prefix| name.strip_prefix(prefix))
            .and_then(Self::from_exact_name)
    }

    fn from_exact_name(name: &str) -> Option<Self> {
        match name {
            "StackPanel" | "Microsoft.UI.Xaml.Controls.StackPanel" => Some(Self::StackPanel),
            "Border" | "Microsoft.UI.Xaml.Controls.Border" => Some(Self::Border),
            "Grid" | "Microsoft.UI.Xaml.Controls.Grid" => Some(Self::Grid),
            "TextBlock" | "Microsoft.UI.Xaml.Controls.TextBlock" => Some(Self::TextBlock),
            "Page" | "Microsoft.UI.Xaml.Controls.Page" => Some(Self::Page),
            "UserControl" | "Microsoft.UI.Xaml.Controls.UserControl" => Some(Self::UserControl),
            "Window" | "Microsoft.UI.Xaml.Window" => Some(Self::Window),
            "ContentDialog" | "Microsoft.UI.Xaml.Controls.ContentDialog" => Some(Self::ContentDialog),
            "ResourceDictionary" | "Microsoft.UI.Xaml.ResourceDictionary" => Some(Self::ResourceDictionary),
            "Application" | "Microsoft.UI.Xaml.Application" => Some(Self::Application),
            "Style" | "Microsoft.UI.Xaml.Style" => Some(Self::Style),
            "DataTemplate" | "Microsoft.UI.Xaml.DataTemplate" => Some(Self::DataTemplate),
            "Paragraph" | "Microsoft.UI.Xaml.Documents.Paragraph" => Some(Self::Paragraph),
            "Run" | "Microsoft.UI.Xaml.Documents.Run" => Some(Self::Run),
            "Span" | "Microsoft.UI.Xaml.Documents.Span" => Some(Self::Span),
            "Button" | "Microsoft.UI.Xaml.Controls.Button" => Some(Self::Button),
            "ScrollViewer" | "Microsoft.UI.Xaml.Controls.ScrollViewer" => Some(Self::ScrollViewer),
            "TextBox" | "Microsoft.UI.Xaml.Controls.TextBox" => Some(Self::TextBox),
            "RichTextBlock" | "Microsoft.UI.Xaml.Controls.RichTextBlock" => Some(Self::RichTextBlock),
            "InfoBar" | "Microsoft.UI.Xaml.Controls.InfoBar" => Some(Self::InfoBar),
            "TitleBar" | "Microsoft.UI.Xaml.Controls.TitleBar" => Some(Self::TitleBar),
            "SplitButton" | "Microsoft.UI.Xaml.Controls.SplitButton" => Some(Self::SplitButton),
            "ContentPresenter" | "Microsoft.UI.Xaml.Controls.ContentPresenter" => Some(Self::ContentPresenter),
            "PasswordBox" | "Microsoft.UI.Xaml.Controls.PasswordBox" => Some(Self::PasswordBox),
            "SymbolIcon" | "Microsoft.UI.Xaml.Controls.SymbolIcon" => Some(Self::SymbolIcon),
            "MicaBackdrop" | "Microsoft.UI.Xaml.Media.MicaBackdrop" => Some(Self::MicaBackdrop),
            "SystemBackdropElement" | "Microsoft.UI.Xaml.Media.SystemBackdropElement" => {
                Some(Self::SystemBackdropElement)
            }
            "FontIcon" | "Microsoft.UI.Xaml.Controls.FontIcon" => Some(Self::FontIcon),
            "Double" => Some(Self::Double),
            "String" => Some(Self::String),
            "Boolean" => Some(Self::Boolean),
"Thickness" => Some(Self::Thickness),
            "ListView" | "Microsoft.UI.Xaml.Controls.ListView" => Some(Self::ListView),
            "GridView" | "Microsoft.UI.Xaml.Controls.GridView" => Some(Self::GridView),
            "TabView" | "Microsoft.UI.Xaml.Controls.TabView" => Some(Self::TabView),
            "ItemsRepeater" | "Microsoft.UI.Xaml.Controls.ItemsRepeater" => Some(Self::ItemsRepeater),
            "ScrollView" | "Microsoft.UI.Xaml.Controls.ScrollView" => Some(Self::ScrollView),
            "DatePicker" | "Microsoft.UI.Xaml.Controls.DatePicker" => Some(Self::DatePicker),
            "RichEditBox" | "Microsoft.UI.Xaml.Controls.RichEditBox" => Some(Self::RichEditBox),
            "CheckBox" | "Microsoft.UI.Xaml.Controls.CheckBox" => Some(Self::CheckBox),
            "Image" | "Microsoft.UI.Xaml.Controls.Image" => Some(Self::Image),
            "ComboBox" | "Microsoft.UI.Xaml.Controls.ComboBox" => Some(Self::ComboBox),
            "ComboBoxItem" | "Microsoft.UI.Xaml.Controls.ComboBoxItem" => Some(Self::ComboBoxItem),
            "ListViewItem" | "Microsoft.UI.Xaml.Controls.ListViewItem" => Some(Self::ListViewItem),
            "GridViewItem" | "Microsoft.UI.Xaml.Controls.GridViewItem" => Some(Self::GridViewItem),
            "ToggleSwitch" | "Microsoft.UI.Xaml.Controls.ToggleSwitch" => Some(Self::ToggleSwitch),
            "RadioButton" | "Microsoft.UI.Xaml.Controls.RadioButton" => Some(Self::RadioButton),
            "HyperlinkButton" | "Microsoft.UI.Xaml.Controls.HyperlinkButton" => Some(Self::HyperlinkButton),
            "RepeatButton" | "Microsoft.UI.Xaml.Controls.RepeatButton" => Some(Self::RepeatButton),
"ToggleButton" | "Microsoft.UI.Xaml.Controls.ToggleButton" => Some(Self::ToggleButton),
            "Ellipse" | "Microsoft.UI.Xaml.Shapes.Ellipse" => Some(Self::Ellipse),
            "Rectangle" | "Microsoft.UI.Xaml.Shapes.Rectangle" => Some(Self::Rectangle),
"Line" | "Microsoft.UI.Xaml.Shapes.Line" => Some(Self::Line),
            "ToolTip" | "Microsoft.UI.Xaml.Controls.ToolTip" => Some(Self::ToolTip),
"Binding" | "Microsoft.UI.Xaml.Data.Binding" => Some(Self::Binding),
            "Expander" | "Microsoft.UI.Xaml.Controls.Expander" => Some(Self::Expander),
"ProgressRing" | "Microsoft.UI.Xaml.Controls.ProgressRing" => Some(Self::ProgressRing),
"SelectorBar" | "Microsoft.UI.Xaml.Controls.SelectorBar" => Some(Self::SelectorBar),
            "SelectorBarItem" | "Microsoft.UI.Xaml.Controls.SelectorBarItem" => Some(Self::SelectorBarItem),
            "AutoSuggestBox" | "Microsoft.UI.Xaml.Controls.AutoSuggestBox" => Some(Self::AutoSuggestBox),
            "Flyout" | "Microsoft.UI.Xaml.Controls.Flyout" => Some(Self::Flyout),
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

    /// Hosts that hold resources, styles or template bodies: nothing below them is
    /// arranged, painted or composed, so the loader does not build their children.
    pub const fn is_non_visual(kind: Kind) -> bool {
        matches!(
            kind,
            Kind::ResourceDictionary
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
                | Kind::Flyout
                | Kind::Binding
        )
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
    /// `UIElement.Opacity`, clamped to 0..1 as the property definition requires.
    pub fn opacity(&self) -> f32 {
        match self.get(&property::OPACITY) {
            Value::Number(value) => value.clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

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
