//! The element kinds the loader can construct, and the WinRT names they answer to.
//!
//! Split from `element.rs` so that the list of kinds and the tree that holds them are
//! two files: one grows with the Gallery's controls, the other does not.

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
    /* An application control whose XAML carries no visual tree: its content is built
     * in code, which a preview does not run. */
    Unrealized,
    ColorPicker,
    ImplicitAnimationSet,
    MenuFlyoutSeparator,
    ImageIconSource,
    NavigationView,
    NavigationViewItem,
    DropDownButton,
    AppBarButton,
    AppBarToggleButton,
    Frame,
    FlipView,
    ProgressBar,
    TeachingTip,
    CommandBar,
    MenuFlyout,
    MenuFlyoutItem,
    TabViewItem,
    NumberBox,
    RadioButtons,
    Slider,
    BreadcrumbBar,
    RelativePanel,
    Canvas,
    UniformGridLayout,
    PersonPicture,
    ItemsControl,
    WrapPanel,
    WebView2,
    UniformGrid,
    ItemsPanelTemplate,
    ControlTemplate,
    DataTemplateSelector,
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
            Self::Unrealized => "Microsoft.UI.Xaml.Unrealized",
            Self::NumberBox => "Microsoft.UI.Xaml.Controls.NumberBox",
            Self::RadioButtons => "Microsoft.UI.Xaml.Controls.RadioButtons",
            Self::Slider => "Microsoft.UI.Xaml.Controls.Slider",
            Self::BreadcrumbBar => "Microsoft.UI.Xaml.Controls.BreadcrumbBar",
            Self::RelativePanel => "Microsoft.UI.Xaml.Controls.RelativePanel",
            Self::Canvas => "Microsoft.UI.Xaml.Controls.Canvas",
            Self::UniformGridLayout => "Microsoft.UI.Xaml.Controls.UniformGridLayout",
            Self::ColorPicker => "Microsoft.UI.Xaml.Controls.ColorPicker",
            Self::PersonPicture => "Microsoft.UI.Xaml.Controls.PersonPicture",
            Self::ItemsControl => "Microsoft.UI.Xaml.Controls.ItemsControl",
            Self::WrapPanel => "Microsoft.UI.Xaml.Controls.WrapPanel",
            Self::WebView2 => "Microsoft.UI.Xaml.Controls.WebView2",
            Self::UniformGrid => "Microsoft.UI.Xaml.Controls.UniformGrid",
            Self::ItemsPanelTemplate => "Microsoft.UI.Xaml.Controls.ItemsPanelTemplate",
            Self::ControlTemplate => "Microsoft.UI.Xaml.Controls.ControlTemplate",
            Self::DataTemplateSelector => "Microsoft.UI.Xaml.Controls.DataTemplateSelector",
            Self::NavigationView => "Microsoft.UI.Xaml.Controls.NavigationView",
            Self::NavigationViewItem => "Microsoft.UI.Xaml.Controls.NavigationViewItem",
            Self::DropDownButton => "Microsoft.UI.Xaml.Controls.DropDownButton",
            Self::AppBarButton => "Microsoft.UI.Xaml.Controls.AppBarButton",
            Self::AppBarToggleButton => "Microsoft.UI.Xaml.Controls.AppBarToggleButton",
            Self::Frame => "Microsoft.UI.Xaml.Controls.Frame",
            Self::FlipView => "Microsoft.UI.Xaml.Controls.FlipView",
            Self::ProgressBar => "Microsoft.UI.Xaml.Controls.ProgressBar",
            Self::TeachingTip => "Microsoft.UI.Xaml.Controls.TeachingTip",
            Self::CommandBar => "Microsoft.UI.Xaml.Controls.CommandBar",
            Self::MenuFlyout => "Microsoft.UI.Xaml.Controls.MenuFlyout",
            Self::MenuFlyoutItem => "Microsoft.UI.Xaml.Controls.MenuFlyoutItem",
            Self::TabViewItem => "Microsoft.UI.Xaml.Controls.TabViewItem",
            Self::ImplicitAnimationSet => "Microsoft.UI.Xaml.Media.ImplicitAnimationSet",
            Self::MenuFlyoutSeparator => "Microsoft.UI.Xaml.Controls.MenuFlyoutSeparator",
            Self::ImageIconSource => "Microsoft.UI.Xaml.Media.ImageIconSource",
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
            "ContentDialog" | "Microsoft.UI.Xaml.Controls.ContentDialog" => {
                Some(Self::ContentDialog)
            }
            "ResourceDictionary" | "Microsoft.UI.Xaml.ResourceDictionary" => {
                Some(Self::ResourceDictionary)
            }
            "Application" | "Microsoft.UI.Xaml.Application" => Some(Self::Application),
            "Style" | "Microsoft.UI.Xaml.Style" => Some(Self::Style),
            "DataTemplate" | "Microsoft.UI.Xaml.DataTemplate" => Some(Self::DataTemplate),
            "Paragraph" | "Microsoft.UI.Xaml.Documents.Paragraph" => Some(Self::Paragraph),
            "Run" | "Microsoft.UI.Xaml.Documents.Run" => Some(Self::Run),
            "Span" | "Microsoft.UI.Xaml.Documents.Span" => Some(Self::Span),
            "Button" | "Microsoft.UI.Xaml.Controls.Button" => Some(Self::Button),
            "ScrollViewer" | "Microsoft.UI.Xaml.Controls.ScrollViewer" => Some(Self::ScrollViewer),
            "TextBox" | "Microsoft.UI.Xaml.Controls.TextBox" => Some(Self::TextBox),
            "RichTextBlock" | "Microsoft.UI.Xaml.Controls.RichTextBlock" => {
                Some(Self::RichTextBlock)
            }
            "InfoBar" | "Microsoft.UI.Xaml.Controls.InfoBar" => Some(Self::InfoBar),
            "TitleBar" | "Microsoft.UI.Xaml.Controls.TitleBar" => Some(Self::TitleBar),
            "SplitButton" | "Microsoft.UI.Xaml.Controls.SplitButton" => Some(Self::SplitButton),
            "ContentPresenter" | "Microsoft.UI.Xaml.Controls.ContentPresenter" => {
                Some(Self::ContentPresenter)
            }
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
            "ItemsRepeater" | "Microsoft.UI.Xaml.Controls.ItemsRepeater" => {
                Some(Self::ItemsRepeater)
            }
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
            "HyperlinkButton" | "Microsoft.UI.Xaml.Controls.HyperlinkButton" => {
                Some(Self::HyperlinkButton)
            }
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
            "SelectorBarItem" | "Microsoft.UI.Xaml.Controls.SelectorBarItem" => {
                Some(Self::SelectorBarItem)
            }
            "ColorPicker" | "Microsoft.UI.Xaml.Controls.ColorPicker" => Some(Self::ColorPicker),
            "PersonPicture" | "Microsoft.UI.Xaml.Controls.PersonPicture" => {
                Some(Self::PersonPicture)
            }
            "ItemsControl" | "Microsoft.UI.Xaml.Controls.ItemsControl" => Some(Self::ItemsControl),
            "WrapPanel" | "Microsoft.UI.Xaml.Controls.WrapPanel" => Some(Self::WrapPanel),
            "WebView2" | "Microsoft.UI.Xaml.Controls.WebView2" => Some(Self::WebView2),
            "UniformGrid" | "Microsoft.UI.Xaml.Controls.UniformGrid" => Some(Self::UniformGrid),
            "ItemsPanelTemplate" | "Microsoft.UI.Xaml.Controls.ItemsPanelTemplate" => {
                Some(Self::ItemsPanelTemplate)
            }
            "ControlTemplate" | "Microsoft.UI.Xaml.Controls.ControlTemplate" => {
                Some(Self::ControlTemplate)
            }
            "DataTemplateSelector" | "Microsoft.UI.Xaml.Controls.DataTemplateSelector" => {
                Some(Self::DataTemplateSelector)
            }
            "NavigationView" | "Microsoft.UI.Xaml.Controls.NavigationView" => {
                Some(Self::NavigationView)
            }
            "NavigationViewItem" | "Microsoft.UI.Xaml.Controls.NavigationViewItem" => {
                Some(Self::NavigationViewItem)
            }
            "DropDownButton" | "Microsoft.UI.Xaml.Controls.DropDownButton" => {
                Some(Self::DropDownButton)
            }
            "AppBarButton" | "Microsoft.UI.Xaml.Controls.AppBarButton" => Some(Self::AppBarButton),
            "AppBarToggleButton" | "Microsoft.UI.Xaml.Controls.AppBarToggleButton" => {
                Some(Self::AppBarToggleButton)
            }
            "Frame" | "Microsoft.UI.Xaml.Controls.Frame" => Some(Self::Frame),
            "FlipView" | "Microsoft.UI.Xaml.Controls.FlipView" => Some(Self::FlipView),
            "ProgressBar" | "Microsoft.UI.Xaml.Controls.ProgressBar" => Some(Self::ProgressBar),
            "TeachingTip" | "Microsoft.UI.Xaml.Controls.TeachingTip" => Some(Self::TeachingTip),
            "CommandBar" | "Microsoft.UI.Xaml.Controls.CommandBar" => Some(Self::CommandBar),
            "MenuFlyout" | "Microsoft.UI.Xaml.Controls.MenuFlyout" => Some(Self::MenuFlyout),
            "MenuFlyoutItem" | "Microsoft.UI.Xaml.Controls.MenuFlyoutItem" => {
                Some(Self::MenuFlyoutItem)
            }
            "TabViewItem" | "Microsoft.UI.Xaml.Controls.TabViewItem" => Some(Self::TabViewItem),
            "AutoSuggestBox" | "Microsoft.UI.Xaml.Controls.AutoSuggestBox" => {
                Some(Self::AutoSuggestBox)
            }
            "Flyout" | "Microsoft.UI.Xaml.Controls.Flyout" => Some(Self::Flyout),
            "MenuFlyoutSeparator" | "Microsoft.UI.Xaml.Controls.MenuFlyoutSeparator" => {
                Some(Self::MenuFlyoutSeparator)
            }
            "ImageIconSource" | "Microsoft.UI.Xaml.Media.ImageIconSource" => {
                Some(Self::ImageIconSource)
            }
            "Unrealized" | "Microsoft.UI.Xaml.Unrealized" => Some(Self::Unrealized),
            "NumberBox" | "Microsoft.UI.Xaml.Controls.NumberBox" => Some(Self::NumberBox),
            "RadioButtons" | "Microsoft.UI.Xaml.Controls.RadioButtons" => Some(Self::RadioButtons),
            "Slider" | "Microsoft.UI.Xaml.Controls.Slider" => Some(Self::Slider),
            "BreadcrumbBar" | "Microsoft.UI.Xaml.Controls.BreadcrumbBar" => {
                Some(Self::BreadcrumbBar)
            }
            "RelativePanel" | "Microsoft.UI.Xaml.Controls.RelativePanel" => {
                Some(Self::RelativePanel)
            }
            "Canvas" | "Microsoft.UI.Xaml.Controls.Canvas" => Some(Self::Canvas),
            "UniformGridLayout" | "Microsoft.UI.Xaml.Controls.UniformGridLayout" => {
                Some(Self::UniformGridLayout)
            }
            "ImplicitAnimationSet" | "Microsoft.UI.Xaml.Media.ImplicitAnimationSet" => {
                Some(Self::ImplicitAnimationSet)
            }
            _ => None,
        }
    }
}

impl Kind {
    /// Hosts that hold resources, styles or template bodies: nothing below them is
    /// arranged, painted or composed, so the loader does not build their children.
    pub const fn is_non_visual(self) -> bool {
        matches!(
            self,
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
                | Kind::ImplicitAnimationSet
                | Kind::UniformGridLayout
                | Kind::MenuFlyoutSeparator
                | Kind::ImageIconSource
                | Kind::Binding
        )
    }
}
