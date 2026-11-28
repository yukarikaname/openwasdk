//! The two public passes: measure and arrange. Kept apart from `layout.rs` so the
//! module that defines the geometry types is not also the module that dispatches them.

use opencomp_core::Rect;

use super::border::{arrange_border, measure_border};
use super::stack::{arrange_stack, horizontal_box, measure_stack, vertical_box};
use super::{Layout, LayoutError, Size, canvas, grid, layout_round};
use crate::element::{Element, Kind, Length, Thickness};

/// Measure an element, applying the outer `FrameworkElement` rules and then the
/// rule for its kind.
pub fn measure(element: &Element, available: Size) -> Result<Size, LayoutError> {
    // A collapsed subtree never enters MeasureCore; it contributes nothing,
    // margin included (`CUIElement::GetIsLayoutSuspended`).
    if !element.is_visible() {
        return Ok(Size::new(0.0, 0.0));
    }

    let margin = element.margin();
    let inner_available = Size::new(
        reduce(available.width, margin.horizontal()),
        reduce(available.height, margin.vertical()),
    );

    let constraint = Size::new(
        match element.width() {
            Length::Pixels(width) => width.max(0.0),
            Length::Auto => inner_available.width,
        },
        match element.height() {
            Length::Pixels(height) => height.max(0.0),
            Length::Auto => inner_available.height,
        },
    );

    let content = measure_override(element, constraint)?;

    // `CLayoutElement::MeasureCore` rounds the content and the margin sums
    // separately before adding the margin back.
    let content_width = match element.width() {
        Length::Pixels(width) => width.max(0.0),
        Length::Auto => content.width,
    };
    let content_height = match element.height() {
        Length::Pixels(height) => height.max(0.0),
        Length::Auto => content.height,
    };
    let mut desired = Size::new(
        layout_round(content_width) + layout_round(margin.horizontal()),
        layout_round(content_height) + layout_round(margin.vertical()),
    );

    if available.width.is_finite() {
        desired.width = desired.width.min(available.width.max(0.0));
    }
    if available.height.is_finite() {
        desired.height = desired.height.min(available.height.max(0.0));
    }
    Ok(desired)
}

fn measure_override(element: &Element, available: Size) -> Result<Size, LayoutError> {
    match element.kind() {
        Kind::TextBlock => Ok(measure_text(element)),
        // A content host (Page, UserControl, Window) carries the same chrome: border
        // thickness and padding deflate the one child, and nothing else is laid out.
        Kind::Border
        | Kind::Page
        | Kind::UserControl
        | Kind::Window
        | Kind::ContentDialog
        | Kind::Button
        | Kind::ScrollViewer
        | Kind::TextBox
        | Kind::RichTextBlock
        | Kind::InfoBar
        | Kind::TitleBar
        | Kind::SplitButton
        | Kind::ContentPresenter
        | Kind::PasswordBox
        | Kind::SymbolIcon
        | Kind::Viewbox
        | Kind::MenuBar
        | Kind::MenuBarItem
        | Kind::VariableSizedWrapGrid
        | Kind::MediaPlayerElement
        | Kind::TreeView
        | Kind::TreeViewItem
        | Kind::ToggleSplitButton
        | Kind::ItemsRepeaterScrollHost
        | Kind::Expander
        | Kind::ProgressRing
        | Kind::SelectorBar
        | Kind::Unrealized
        | Kind::ColorPicker
        | Kind::PersonPicture
        | Kind::ItemsControl
        | Kind::WrapPanel
        | Kind::WebView2
        | Kind::UniformGrid
        | Kind::NavigationView
        | Kind::NavigationViewItem
        | Kind::DropDownButton
        | Kind::AppBarButton
        | Kind::AppBarToggleButton
        | Kind::Frame
        | Kind::FlipView
        | Kind::ProgressBar
        | Kind::TeachingTip
        | Kind::CommandBar
        | Kind::MenuFlyout
        | Kind::MenuFlyoutItem
        | Kind::TabViewItem
        | Kind::NumberBox
        | Kind::RadioButtons
        | Kind::Slider
        | Kind::BreadcrumbBar
        | Kind::RelativePanel
        | Kind::SelectorBarItem
        | Kind::AutoSuggestBox
        | Kind::ListView
        | Kind::GridView
        | Kind::TabView
        | Kind::ItemsRepeater
        | Kind::ScrollView
        | Kind::DatePicker
        | Kind::RichEditBox
        | Kind::CheckBox
        | Kind::Image
        | Kind::ComboBox
        | Kind::ComboBoxItem
        | Kind::ListViewItem
        | Kind::GridViewItem
        | Kind::ToggleSwitch
        | Kind::RadioButton
        | Kind::HyperlinkButton
        | Kind::RepeatButton
        | Kind::ToggleButton => measure_border(element, available),
        // A resource dictionary and an Application are not part of the visual tree.
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
        | Kind::CommandBarFlyout
        | Kind::ItemsPanelTemplate
        | Kind::ControlTemplate
        | Kind::DataTemplateSelector => Ok(Size::new(0.0, 0.0)),
        Kind::StackPanel => measure_stack(element, available),
        Kind::Grid => grid::measure(element, available),
        Kind::Canvas => canvas::measure_canvas(element, available),
    }
}

fn measure_text(element: &Element) -> Size {
    let font_size = element.font_size();
    let characters = element.text().chars().count() as f32;
    Size::new(characters * font_size * 0.5, font_size * 1.2)
}

pub fn arrange(element: &Element, slot: Rect) -> Result<Layout, LayoutError> {
    if !element.is_visible() {
        return Ok(Layout {
            rect: Rect::new(slot.x, slot.y, 0.0, 0.0),
            children: Vec::new(),
        });
    }
    let desired = measure(element, Size::new(slot.width, slot.height))?;
    arrange_with_desired(element, slot, desired)
}

pub(crate) fn arrange_with_desired(
    element: &Element,
    slot: Rect,
    desired: Size,
) -> Result<Layout, LayoutError> {
    let margin = element.margin();
    let client = Rect::new(
        slot.x + margin.left,
        slot.y + margin.top,
        (slot.width - margin.horizontal()).max(0.0),
        (slot.height - margin.vertical()).max(0.0),
    );
    let desired_inner = Size::new(
        (desired.width - margin.horizontal()).max(0.0),
        (desired.height - margin.vertical()).max(0.0),
    );

    let (x, width) = horizontal_box(
        element.width(),
        element.horizontal_alignment(),
        desired_inner.width,
        client,
    );
    let (y, height) = vertical_box(
        element.height(),
        element.vertical_alignment(),
        desired_inner.height,
        client,
    );

    let local = Rect::new(0.0, 0.0, width, height);
    Ok(Layout {
        rect: Rect::new(x, y, width, height),
        children: arrange_children(element, local)?,
    })
}

fn arrange_children(element: &Element, content: Rect) -> Result<Vec<Layout>, LayoutError> {
    match element.kind() {
        Kind::TextBlock => Ok(Vec::new()),
        Kind::Border
        | Kind::Page
        | Kind::UserControl
        | Kind::Window
        | Kind::ContentDialog
        | Kind::Button
        | Kind::ScrollViewer
        | Kind::TextBox
        | Kind::RichTextBlock
        | Kind::InfoBar
        | Kind::TitleBar
        | Kind::SplitButton
        | Kind::ContentPresenter
        | Kind::PasswordBox
        | Kind::SymbolIcon
        | Kind::Viewbox
        | Kind::MenuBar
        | Kind::MenuBarItem
        | Kind::VariableSizedWrapGrid
        | Kind::MediaPlayerElement
        | Kind::TreeView
        | Kind::TreeViewItem
        | Kind::ToggleSplitButton
        | Kind::ItemsRepeaterScrollHost
        | Kind::Expander
        | Kind::ProgressRing
        | Kind::SelectorBar
        | Kind::Unrealized
        | Kind::ColorPicker
        | Kind::PersonPicture
        | Kind::ItemsControl
        | Kind::WrapPanel
        | Kind::WebView2
        | Kind::UniformGrid
        | Kind::NavigationView
        | Kind::NavigationViewItem
        | Kind::DropDownButton
        | Kind::AppBarButton
        | Kind::AppBarToggleButton
        | Kind::Frame
        | Kind::FlipView
        | Kind::ProgressBar
        | Kind::TeachingTip
        | Kind::CommandBar
        | Kind::MenuFlyout
        | Kind::MenuFlyoutItem
        | Kind::TabViewItem
        | Kind::NumberBox
        | Kind::RadioButtons
        | Kind::Slider
        | Kind::BreadcrumbBar
        | Kind::RelativePanel
        | Kind::SelectorBarItem
        | Kind::AutoSuggestBox
        | Kind::ListView
        | Kind::GridView
        | Kind::TabView
        | Kind::ItemsRepeater
        | Kind::ScrollView
        | Kind::DatePicker
        | Kind::RichEditBox
        | Kind::CheckBox
        | Kind::Image
        | Kind::ComboBox
        | Kind::ComboBoxItem
        | Kind::ListViewItem
        | Kind::GridViewItem
        | Kind::ToggleSwitch
        | Kind::RadioButton
        | Kind::HyperlinkButton
        | Kind::RepeatButton
        | Kind::ToggleButton => arrange_border(element, content),
        // Nothing below a non-visual host is arranged, so nothing of it is composed.
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
        | Kind::CommandBarFlyout
        | Kind::ItemsPanelTemplate
        | Kind::ControlTemplate
        | Kind::DataTemplateSelector => Ok(Vec::new()),
        Kind::StackPanel => arrange_stack(element, content),
        Kind::Grid => grid::arrange(element, content),
        Kind::Canvas => canvas::arrange_canvas(element, content),
    }
}

pub(crate) fn collapsed(origin: Rect) -> Layout {
    Layout {
        rect: Rect::new(origin.x, origin.y, 0.0, 0.0),
        children: Vec::new(),
    }
}

/// `CBorder::HelperDeflateRect`: subtract a thickness, never going negative.
pub(crate) fn deflate(rect: Rect, thickness: Thickness) -> Rect {
    Rect::new(
        rect.x + thickness.left,
        rect.y + thickness.top,
        (rect.width - thickness.horizontal()).max(0.0),
        (rect.height - thickness.vertical()).max(0.0),
    )
}

pub(crate) fn reduce(length: f32, amount: f32) -> f32 {
    if length.is_finite() {
        (length - amount).max(0.0)
    } else {
        f32::INFINITY
    }
}
