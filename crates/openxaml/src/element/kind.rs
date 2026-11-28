//! The element kinds, declared once.
//!
//! The rows are emitted from the metadata:
//!
//! ```sh
//! cargo run -p comptrace -- kinds target/appsdk/Microsoft.UI.Xaml.winmd
//! ```
//!
//! which decides each class's role from its base chain (`Panel`, `UIElement`, or neither)
//! and leaves only the panels this crate actually lays out to be named by hand.
//!
//! A control used to be written in four places -- the enum, its WinRT name, the name lookup
//! and the layout pass -- and one of them was missed often enough to be a bug class. The
//! table below is the only list: the macro expands the enum, the names, the role and the
//! lookup, so adding a control is one line.

use super::role::{Panel, Role};

macro_rules! kinds {
    ($(($name:ident, $short:literal, $full:literal, $role:ident $(($panel:ident))?)),* $(,)?) => {
        /// Every element kind this crate answers to.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Kind {
            $($name,)*
        }

        impl Kind {
            /// Every kind, in declaration order, so a lookup can be a scan.
            pub const ALL: &'static [Kind] = &[$(Kind::$name,)*];

            /// The WinRT name this kind answers to.
            pub const fn type_name(self) -> &'static str {
                match self { $(Kind::$name => $full,)* }
            }

            /// The name without its namespace, which is what a page writes.
            pub const fn short_name(self) -> &'static str {
                match self { $(Kind::$name => $short,)* }
            }

            /// What the layout pass does with it.
            pub const fn role(self) -> Role {
                match self { $(Kind::$name => kinds!(@role $role $(($panel))?),)* }
            }

            /// Hosts that hold resources, styles or template bodies: nothing below them is
            /// arranged, painted or composed.
            pub const fn is_non_visual(self) -> bool {
                matches!(self.role(), Role::NonVisual)
            }
        }
    };
    (@role Panel($panel:ident)) => { Role::Panel(Panel::$panel) };
    (@role $role:ident) => { Role::$role };
}

kinds! {
    (StackPanel, "StackPanel", "Microsoft.UI.Xaml.Controls.StackPanel", Panel(Stack)),
    (Border, "Border", "Microsoft.UI.Xaml.Controls.Border", ContentHost),
    (Grid, "Grid", "Microsoft.UI.Xaml.Controls.Grid", Panel(Grid)),
    (TextBlock, "TextBlock", "Microsoft.UI.Xaml.Controls.TextBlock", Text),
    (Page, "Page", "Microsoft.UI.Xaml.Controls.Page", ContentHost),
    (UserControl, "UserControl", "Microsoft.UI.Xaml.Controls.UserControl", ContentHost),
    (Window, "Window", "Microsoft.UI.Xaml.Window", ContentHost),
    (ContentDialog, "ContentDialog", "Microsoft.UI.Xaml.Controls.ContentDialog", ContentHost),
    (ResourceDictionary, "ResourceDictionary", "Microsoft.UI.Xaml.ResourceDictionary", NonVisual),
    (Application, "Application", "Microsoft.UI.Xaml.Application", NonVisual),
    (Style, "Style", "Microsoft.UI.Xaml.Style", NonVisual),
    (DataTemplate, "DataTemplate", "Microsoft.UI.Xaml.DataTemplate", NonVisual),
    (Paragraph, "Paragraph", "Microsoft.UI.Xaml.Documents.Paragraph", NonVisual),
    (Run, "Run", "Microsoft.UI.Xaml.Documents.Run", NonVisual),
    (Span, "Span", "Microsoft.UI.Xaml.Documents.Span", NonVisual),
    (Button, "Button", "Microsoft.UI.Xaml.Controls.Button", ContentHost),
    (ScrollViewer, "ScrollViewer", "Microsoft.UI.Xaml.Controls.ScrollViewer", ContentHost),
    (ScrollContentPresenter, "ScrollContentPresenter", "Microsoft.UI.Xaml.Controls.ScrollContentPresenter", ContentHost),
    (NavigationViewItemHeader, "NavigationViewItemHeader", "Microsoft.UI.Xaml.Controls.NavigationViewItemHeader", ContentHost),
    (NavigationViewItemSeparator, "NavigationViewItemSeparator", "Microsoft.UI.Xaml.Controls.NavigationViewItemSeparator", ContentHost),
    (NavigationViewItemInvokedEventArgs, "NavigationViewItemInvokedEventArgs", "Microsoft.UI.Xaml.Controls.NavigationViewItemInvokedEventArgs", ContentHost),
    (ScrollBar, "ScrollBar", "Microsoft.UI.Xaml.Controls.ScrollBar", ContentHost),
    (Thumb, "Thumb", "Microsoft.UI.Xaml.Controls.Thumb", ContentHost),
    (ItemsPresenter, "ItemsPresenter", "Microsoft.UI.Xaml.Controls.ItemsPresenter", ContentHost),
    (Popup, "Popup", "Microsoft.UI.Xaml.Controls.Popup", ContentHost),
    (TextBox, "TextBox", "Microsoft.UI.Xaml.Controls.TextBox", ContentHost),
    (RichTextBlock, "RichTextBlock", "Microsoft.UI.Xaml.Controls.RichTextBlock", ContentHost),
    (InfoBar, "InfoBar", "Microsoft.UI.Xaml.Controls.InfoBar", ContentHost),
    (TitleBar, "TitleBar", "Microsoft.UI.Xaml.Controls.TitleBar", ContentHost),
    (SplitButton, "SplitButton", "Microsoft.UI.Xaml.Controls.SplitButton", ContentHost),
    (ContentPresenter, "ContentPresenter", "Microsoft.UI.Xaml.Controls.ContentPresenter", ContentHost),
    (PasswordBox, "PasswordBox", "Microsoft.UI.Xaml.Controls.PasswordBox", ContentHost),
    (SymbolIcon, "SymbolIcon", "Microsoft.UI.Xaml.Controls.SymbolIcon", ContentHost),
    (MicaBackdrop, "MicaBackdrop", "Microsoft.UI.Xaml.Media.MicaBackdrop", NonVisual),
    (SystemBackdropElement, "SystemBackdropElement", "Microsoft.UI.Xaml.Media.SystemBackdropElement", NonVisual),
    (FontIcon, "FontIcon", "Microsoft.UI.Xaml.Controls.FontIcon", NonVisual),
    (PathIcon, "PathIcon", "Microsoft.UI.Xaml.Controls.PathIcon", ContentHost),
    (BitmapIcon, "BitmapIcon", "Microsoft.UI.Xaml.Controls.BitmapIcon", ContentHost),
    (TranslateTransform, "TranslateTransform", "Microsoft.UI.Xaml.Media.TranslateTransform", NonVisual),
    (ScaleTransform, "ScaleTransform", "Microsoft.UI.Xaml.Media.ScaleTransform", NonVisual),
    (Double, "Double", "Double", NonVisual),
    (String, "String", "String", NonVisual),
    (Boolean, "Boolean", "Boolean", NonVisual),
    (Thickness, "Thickness", "Thickness", NonVisual),
    (ListView, "ListView", "Microsoft.UI.Xaml.Controls.ListView", ContentHost),
    (GridView, "GridView", "Microsoft.UI.Xaml.Controls.GridView", ContentHost),
    (TabView, "TabView", "Microsoft.UI.Xaml.Controls.TabView", ContentHost),
    (ItemsRepeater, "ItemsRepeater", "Microsoft.UI.Xaml.Controls.ItemsRepeater", ContentHost),
    (ScrollView, "ScrollView", "Microsoft.UI.Xaml.Controls.ScrollView", ContentHost),
    (DatePicker, "DatePicker", "Microsoft.UI.Xaml.Controls.DatePicker", ContentHost),
    (RichEditBox, "RichEditBox", "Microsoft.UI.Xaml.Controls.RichEditBox", ContentHost),
    (CheckBox, "CheckBox", "Microsoft.UI.Xaml.Controls.CheckBox", ContentHost),
    (Image, "Image", "Microsoft.UI.Xaml.Controls.Image", ContentHost),
    (ComboBox, "ComboBox", "Microsoft.UI.Xaml.Controls.ComboBox", ContentHost),
    (ComboBoxItem, "ComboBoxItem", "Microsoft.UI.Xaml.Controls.ComboBoxItem", ContentHost),
    (ListViewItem, "ListViewItem", "Microsoft.UI.Xaml.Controls.ListViewItem", ContentHost),
    (GridViewItem, "GridViewItem", "Microsoft.UI.Xaml.Controls.GridViewItem", ContentHost),
    (ToggleSwitch, "ToggleSwitch", "Microsoft.UI.Xaml.Controls.ToggleSwitch", ContentHost),
    (RadioButton, "RadioButton", "Microsoft.UI.Xaml.Controls.RadioButton", ContentHost),
    (HyperlinkButton, "HyperlinkButton", "Microsoft.UI.Xaml.Controls.HyperlinkButton", ContentHost),
    (RepeatButton, "RepeatButton", "Microsoft.UI.Xaml.Controls.RepeatButton", ContentHost),
    (ToggleButton, "ToggleButton", "Microsoft.UI.Xaml.Controls.ToggleButton", ContentHost),
    (Ellipse, "Ellipse", "Microsoft.UI.Xaml.Shapes.Ellipse", NonVisual),
    (Rectangle, "Rectangle", "Microsoft.UI.Xaml.Shapes.Rectangle", NonVisual),
    (Line, "Line", "Microsoft.UI.Xaml.Shapes.Line", NonVisual),
    (ToolTip, "ToolTip", "Microsoft.UI.Xaml.Controls.ToolTip", NonVisual),
    (Binding, "Binding", "Microsoft.UI.Xaml.Data.Binding", NonVisual),
    (Expander, "Expander", "Microsoft.UI.Xaml.Controls.Expander", ContentHost),
    (ProgressRing, "ProgressRing", "Microsoft.UI.Xaml.Controls.ProgressRing", ContentHost),
    (SelectorBar, "SelectorBar", "Microsoft.UI.Xaml.Controls.SelectorBar", Panel(Stack)),
    (SwitchPresenter, "SwitchPresenter", "CommunityToolkit.WinUI.Controls.SwitchPresenter", ContentHost),
    (SelectorBarItem, "SelectorBarItem", "Microsoft.UI.Xaml.Controls.SelectorBarItem", ContentHost),
    (AutoSuggestBox, "AutoSuggestBox", "Microsoft.UI.Xaml.Controls.AutoSuggestBox", ContentHost),
    (Flyout, "Flyout", "Microsoft.UI.Xaml.Controls.Flyout", NonVisual),
    (Unrealized, "Unrealized", "Microsoft.UI.Xaml.Unrealized", ContentHost),
    (ColorPicker, "ColorPicker", "Microsoft.UI.Xaml.Controls.ColorPicker", ContentHost),
    (ImplicitAnimationSet, "ImplicitAnimationSet", "Microsoft.UI.Xaml.Media.ImplicitAnimationSet", NonVisual),
    (MenuFlyoutSeparator, "MenuFlyoutSeparator", "Microsoft.UI.Xaml.Controls.MenuFlyoutSeparator", NonVisual),
    (ImageIconSource, "ImageIconSource", "Microsoft.UI.Xaml.Media.ImageIconSource", NonVisual),
    (NavigationView, "NavigationView", "Microsoft.UI.Xaml.Controls.NavigationView", ContentHost),
    (NavigationViewItem, "NavigationViewItem", "Microsoft.UI.Xaml.Controls.NavigationViewItem", ContentHost),
    (DropDownButton, "DropDownButton", "Microsoft.UI.Xaml.Controls.DropDownButton", ContentHost),
    (AppBarButton, "AppBarButton", "Microsoft.UI.Xaml.Controls.AppBarButton", ContentHost),
    (AppBarSeparator, "AppBarSeparator", "Microsoft.UI.Xaml.Controls.AppBarSeparator", ContentHost),
    (AppBarToggleButton, "AppBarToggleButton", "Microsoft.UI.Xaml.Controls.AppBarToggleButton", ContentHost),
    (Frame, "Frame", "Microsoft.UI.Xaml.Controls.Frame", ContentHost),
    (FlipView, "FlipView", "Microsoft.UI.Xaml.Controls.FlipView", ContentHost),
    (ProgressBar, "ProgressBar", "Microsoft.UI.Xaml.Controls.ProgressBar", ContentHost),
    (TeachingTip, "TeachingTip", "Microsoft.UI.Xaml.Controls.TeachingTip", ContentHost),
    (CommandBar, "CommandBar", "Microsoft.UI.Xaml.Controls.CommandBar", ContentHost),
    (MenuFlyout, "MenuFlyout", "Microsoft.UI.Xaml.Controls.MenuFlyout", ContentHost),
    (MenuFlyoutItem, "MenuFlyoutItem", "Microsoft.UI.Xaml.Controls.MenuFlyoutItem", ContentHost),
    (TabViewItem, "TabViewItem", "Microsoft.UI.Xaml.Controls.TabViewItem", ContentHost),
    (NumberBox, "NumberBox", "Microsoft.UI.Xaml.Controls.NumberBox", ContentHost),
    (RadioButtons, "RadioButtons", "Microsoft.UI.Xaml.Controls.RadioButtons", ContentHost),
    (Slider, "Slider", "Microsoft.UI.Xaml.Controls.Slider", ContentHost),
    (BreadcrumbBar, "BreadcrumbBar", "Microsoft.UI.Xaml.Controls.BreadcrumbBar", ContentHost),
    (RelativePanel, "RelativePanel", "Microsoft.UI.Xaml.Controls.RelativePanel", ContentHost),
    (Canvas, "Canvas", "Microsoft.UI.Xaml.Controls.Canvas", Panel(Canvas)),
    (UniformGridLayout, "UniformGridLayout", "Microsoft.UI.Xaml.Controls.UniformGridLayout", NonVisual),
    (PersonPicture, "PersonPicture", "Microsoft.UI.Xaml.Controls.PersonPicture", ContentHost),
    (ItemsControl, "ItemsControl", "Microsoft.UI.Xaml.Controls.ItemsControl", ContentHost),
    (WrapPanel, "WrapPanel", "Microsoft.UI.Xaml.Controls.WrapPanel", ContentHost),
    (WebView2, "WebView2", "Microsoft.UI.Xaml.Controls.WebView2", ContentHost),
    (UniformGrid, "UniformGrid", "Microsoft.UI.Xaml.Controls.UniformGrid", ContentHost),
    (ItemsPanelTemplate, "ItemsPanelTemplate", "Microsoft.UI.Xaml.Controls.ItemsPanelTemplate", ContentHost),
    (ControlTemplate, "ControlTemplate", "Microsoft.UI.Xaml.Controls.ControlTemplate", ContentHost),
    (DataTemplateSelector, "DataTemplateSelector", "Microsoft.UI.Xaml.Controls.DataTemplateSelector", ContentHost),
    (Viewbox, "Viewbox", "Microsoft.UI.Xaml.Controls.Viewbox", ContentHost),
    (MenuBar, "MenuBar", "Microsoft.UI.Xaml.Controls.MenuBar", ContentHost),
    (MenuBarItem, "MenuBarItem", "Microsoft.UI.Xaml.Controls.MenuBarItem", ContentHost),
    (VariableSizedWrapGrid, "VariableSizedWrapGrid", "Microsoft.UI.Xaml.Controls.VariableSizedWrapGrid", ContentHost),
    (MediaPlayerElement, "MediaPlayerElement", "Microsoft.UI.Xaml.Controls.MediaPlayerElement", ContentHost),
    (TreeView, "TreeView", "Microsoft.UI.Xaml.Controls.TreeView", ContentHost),
    (TreeViewItem, "TreeViewItem", "Microsoft.UI.Xaml.Controls.TreeViewItem", ContentHost),
    (ToggleSplitButton, "ToggleSplitButton", "Microsoft.UI.Xaml.Controls.ToggleSplitButton", ContentHost),
    (ItemsRepeaterScrollHost, "ItemsRepeaterScrollHost", "Microsoft.UI.Xaml.Controls.ItemsRepeaterScrollHost", ContentHost),
    (SymbolIconSource, "SymbolIconSource", "Microsoft.UI.Xaml.Controls.SymbolIconSource", NonVisual),
    (CommandBarFlyout, "CommandBarFlyout", "Microsoft.UI.Xaml.Controls.CommandBarFlyout", NonVisual),
}

impl Kind {
    /// A type name from a compiled page, or nothing when it is not a kind this crate has.
    ///
    /// A full WinRT name is accepted as it is; a short name is accepted only under one of the
    /// WinUI namespaces, because a type from the application's own namespace must not be
    /// guessed to be a control.
    pub fn from_type_name(name: &str) -> Option<Self> {
        if let Some(kind) = Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.type_name() == name)
        {
            return Some(kind);
        }
        if let Some(kind) = Self::from_short(name) {
            return Some(kind);
        }
        const PREFIXES: &[&str] = &[
            "Microsoft.UI.Xaml.Controls.",
            "Microsoft.UI.Xaml.Documents.",
            "Microsoft.UI.Xaml.Media.",
            "Microsoft.UI.Xaml.",
            "http://schemas.microsoft.com/client/2007.",
            /* A type the writer marked as coming from a namespace the page imports. */
            "using:",
        ];
        PREFIXES.iter().find_map(|prefix| {
            let rest = name.strip_prefix(prefix)?;
            Self::from_short(rest).or_else(|| rest.rsplit('.').next().and_then(Self::from_short))
        })
    }

    fn from_short(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.short_name() == name)
    }
}
