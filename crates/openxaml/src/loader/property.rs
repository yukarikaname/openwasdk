//! Property conversion: one XBF property into the element's store.
//!
//! Every value is converted to the type its descriptor names, or the loader is told
//! which name it could not convert. A name the type's table does not carry is the
//! work item for the next control, so the error carries it.

use super::value::*;
use super::*;
use crate::element::{Element, Kind, Length, Value};
use crate::property::ValueType;
use crate::support::TypeSupport;

pub(super) fn load_property_with(
    element: &mut Element,
    support: &'static TypeSupport,
    type_name: &str,
    property: &xbf::Property,
    controls: &Controls,
    depth: usize,
) -> Result<(), LoadError> {
    match property.name.as_str() {
        /* Directives are names, not properties; they stay out of the store. */
        "x:Name" => {
            element.set_name(string_value(type_name, property)?);
            Ok(())
        }
        /* A compiler-assigned connection id: it names a node in traces, not a value. */
        "x:ConnectionId" => Ok(()),
        "x:Key" => {
            element.set_key(string_value(type_name, property)?);
            Ok(())
        }
        name => {
            /*
             * XBF names a property by its declaring type: `FrameworkElement_Width`,
             * `UserControl_Content`, `Control_Background`. What a consumer matches is the
             * member the declaring type introduces, so the prefix goes.
             */
            /*
             * The name may be qualified twice: the writer can spell the declaring type as
             * a namespace ("http://schemas.microsoft.com/client/2007.Expander.Header") or
             * join it to the member with an underscore ("FrameworkElement_Width").
             */
            /*
             * An accessibility name is not the element's name: it belongs to the UI
             * automation tree, which no visual consumer reads.
             */
            if name.contains("AutomationProperties") {
                return Ok(());
            }
            let member = name.rsplit_once(['_', '.']).map_or(name, |(_, tail)| tail);
            /*
             * A content property holds the child that a content control shows. It is a
             * child for layout and painting, so it joins the element's children.
             */
            /*
             * `Style="{StaticResource BodyTextBlockStyle}"`: a style reached by key, whose
             * setters give a control its typography. Applied here, before the rest of the
             * element's own values, so a local value still wins.
             */
            if member == "Style" {
                if let xbf::model::Value::Resource { key, .. } = &property.value {
                    let setters = controls.theme.setters_for_key(key);
                    super::apply_setters(element, &setters, &controls.theme);
                }
                return Ok(());
            }
            /* FrameworkElement.Name is x:Name under the spelling the writer picked. */
            if member == "Name" {
                element.set_name(string_value(type_name, property)?);
                return Ok(());
            }
            /*
             * A content property holds its child, and XAML names it "...Content" or
             * "...Items" (MenuItems, TabItems). Only an element value is a child; a scalar
             * such as ToggleSwitch.OnContent is state and is dropped.
             */
            let elements = property_elements(property)?;
            let content_style = matches!(member, "Child" | "Children" | "Items")
                || member.ends_with("Content")
                || (member.ends_with("Items") && !elements.is_empty());
            if content_style {
                for child in elements {
                    element.push(load_element(&child, controls, depth)?);
                }
                /*
                 * A string content is what a content control shows as its text; the runtime
                 * wraps it in a TextBlock, and so does this loader, so `<Button Content="Go"/>`
                 * has something to lay out.
                 */
                if let xbf::model::Value::String(text) = &property.value
                    && member.ends_with("Content")
                {
                    element.push(Element::text_block(text.clone()));
                }
                return Ok(());
            }
            /*
             * Properties this renderer has no consumer for: a resource map, a window
             * title, a style's target. Accepting them keeps the page loadable and drops
             * nothing that would have been drawn.
             */
            if matches!(
                member,
                "Resources"
                    | "ThemeDictionaries"
                    | "SystemBackdrop"
                    | "Title"
                    | "TargetType"
                    | "BasedOn"
                    | "Template"
                    | "SuppressHostScrolling"
                    | "IsTabStop"
                    | "Blocks"
                    | "TextWrapping"
                    | "Style"
                    | "Header"
                    | "ToolTip"
                    | "VerticalContentAlignment"
                    | "TextAlignment"
                    | "HeadingLevel"
                    | "HorizontalScrollBarVisibility"
                    | "VerticalScrollBarVisibility"
                    | "TabFocusNavigation"
                    | "PlaceholderText"
                    | "MergedDictionaries"
                    | "Inlines"
                    | "AutomationId"
                    | "CloseButtonText"
                    | "HorizontalScrollMode"
                    | "VerticalScrollMode"
                    | "FontWeight"
                    /* Layout constraints this renderer does not apply yet: dropping them
                     * changes nothing it draws, but the page still measures as if absent. */
                    | "MinHeight"
                    | "MinWidth"
                    | "MaxWidth"
                    | "MaxHeight"
                    | "ZIndex"
                    | "VisualStateGroups"
                    | "IsHitTestVisible"
                    | "TextTrimming"
                    | "Symbol"
                    | "DefaultButton"
                    /* FontSize is in the text element's list, so it is stored, not dropped. */
                    | "FontFamily"
                    | "FontStyle"
                    | "Foreground"
                    | "Stretch"
                    | "Source"
                    | "Value"
                    | "Glyph"
                    | "AcceptsReturn"
                    | "AccessibilityView"
                    | "HorizontalContentAlignment"
                    | "IsItemClickEnabled"
                    | "PrimaryButtonText"
                    | "SecondaryButtonText"
                    | "IsTextSelectionEnabled"
                    | "SelectionMode"
                    | "ItemContainerStyle"
                    | "ItemTemplate"
                    | "Flyout"
                    | "ContentMargin"
                    | "FocusVisualSecondaryThickness"
                    | "FocusVisualSecondaryBrush"
                    | "FocusVisualPrimaryThickness"
                    | "FocusVisualPrimaryBrush"
                    | "BackgroundSizing"
                    | "RadiusY"
                    | "RadiusX"
                    | "IsTemplateKeyTipTarget"
                    | "IsTemplateFocusTarget"
                    | "ZoomMode"
                    | "IsOpen"
                    | "IsClosable"
                    | "IsCopyButtonVisible"
                    | "CanTearOutTabs"
                    | "IsEnabled"
                    | "Message"
                    | "Severity"
                    | "ElementName"
                    | "SampleType"
                    | "IsPaneToggleButtonVisible"
                    | "Path"
                    | "Subtitle"
                    | "IconSource"
                    | "TabStripHeader"
                    | "XYFocusKeyboardNavigation"
                    | "IsIconVisible"
                    | "MaxLines"
                    | "ChildrenTransitions"
                    | "SmallChange"
                    | "StepFrequency"
                    | "Initials"
                    | "IsHexInputVisible"
                    | "ColorSpectrumShape"
                    | "IsReadOnly"
                    | "SnapsTo"
                    | "ItemSpacing"
                    | "MinItemHeight"
                    | "MinItemWidth"
                    | "Tag"
                    | "Placement"
                    | "RightHeader"
                    | "TabStripFooter"
                    | "Layout"
                    | "QueryIcon"
                    | "KeyboardAcceleratorPlacementMode"
                    | "AcceptsExpression"
                    | "AlignVerticalCenterWith"
                    | "AutomationControlType"
                    | "CharacterSpacing"
                    | "ContentOrientation"
                    | "DayFormat"
                    | "DefaultLabelPosition"
                    | "Description"
                    | "ExpandDirection"
                    | "GroupName"
                    | "HelpText"
                    | "Icon"
                    | "ImageSource"
                    | "IsBackButtonVisible"
                    | "IsChecked"
                    | "IsDirectionReversed"
                    | "IsFocusEngagementEnabled"
                    | "IsIndeterminate"
                    | "IsMoreButtonVisible"
                    | "IsOn"
                    | "IsSelected"
                    | "IsSwipeEnabled"
                    | "IsThreeState"
                    | "IsVerticalScrollChainingEnabled"
                    | "ItemsPanel"
                    | "KeyboardAccelerators"
                    | "LargeChange"
                    | "LiveSetting"
                    | "MaxColumns"
                    | "Maximum"
                    | "MinColumnSpacing"
                    | "Minimum"
                    | "Mode"
                    | "NavigateUri"
                    | "OffContent"
                    | "PaneDisplayMode"
                    | "PasswordChar"
                    | "PrimaryCommands"
                    | "RenderTransform"
                    | "RightOf"
                    | "SelectedIndex"
                    | "SelectedValue"
                    | "SelectionHighlightColor"
                    | "Shadow"
                    | "SpinButtonPlacementMode"
                    | "Stroke"
                    | "TabIndex"
                    | "VerticalOffset"
                    // Everything above is control state, text or a drawing effect this
                    // preview does not apply: it changes nothing it paints, and the page
                    // still measures as the same box.
                    | "IsSpellCheckEnabled"
                    | "Fill"
            ) {
                return Ok(());
            }
            /*
             * A track collection is a property of the panel that consumes it, and its
             * descriptor says so: `Grid.RowDefinitions`.
             */
            let member = match member {
                "RowDefinitions" => "Grid.RowDefinitions",
                "ColumnDefinitions" => "Grid.ColumnDefinitions",
                // A child of a panel names its cell through the panel's attached property.
                "Row" => "Grid.Row",
                "Column" => "Grid.Column",
                "Left" => "Canvas.Left",
                "Top" => "Canvas.Top",
                "RowSpacing" => "Grid.RowSpacing",
                "ColumnSpacing" => "Grid.ColumnSpacing",
                "RowSpan" => "Grid.RowSpan",
                "ColumnSpan" => "Grid.ColumnSpan",
                other => other,
            };
            /*
             * A reference -- a resource, a deferred body, a markup extension result -- is
             * not a value this renderer can resolve: it has no resource map and runs no
             * extension. The property stays at its default, which is what the runtime
             * shows until something resolves it.
             */
            /*
             * A template binding is not a value yet: it says which control property this one
             * reads, and is resolved when the template is instantiated for a control.
             */
            if let xbf::model::Value::TemplateBinding { name, .. } = &property.value {
                let member = name
                    .rsplit_once(['_', '.'])
                    .map_or(name.as_str(), |(_, tail)| tail);
                if let Some(target) = support.property(member) {
                    element.bind_template(target, name.clone());
                }
                return Ok(());
            }
            if matches!(
                property.value,
                xbf::model::Value::Resource { .. }
                    | xbf::model::Value::Deferred { .. }
                    | xbf::model::Value::Extension { .. }
                    | xbf::model::Value::TemplateBinding { .. }
            ) {
                return Ok(());
            }
            let accepted = match support.property(member) {
                Some(accepted) => accepted,
                /*
                 * A cell is an attached property: the panel owns it, any element may carry
                 * it, so it is not in every element's list.
                 */
                None if matches!(
                    member,
                    "Grid.Row"
                        | "Grid.Column"
                        | "Grid.RowSpan"
                        | "Grid.ColumnSpan"
                        /*
                         * Chrome travels: a style or a control body sets a corner radius on
                         * whatever box it decorates, and the writer names the element it was
                         * applied to. Storing it on the element is what the runtime does.
                         */
                        | "CornerRadius"
                        | "BorderThickness"
                        | "BorderBrush"
                        /*
                         * Text travels the same way: any control that shows a string has a
                         * Text member, and the descriptor is the same one everywhere.
                         */
                        | "Text"
                        | "FontSize"
                        // A Canvas child names its offset through the panel's attached property.
                        | "Canvas.Left"
                        | "Canvas.Top"
                        // Opacity is a member of UIElement, so every element has it.
                        | "Opacity"
                ) =>
                {
                    crate::property::PROPERTIES
                        .iter()
                        .copied()
                        .find(|candidate| candidate.name == member)
                        .ok_or_else(|| LoadError::UnsupportedProperty {
                            element: type_name.to_string(),
                            name: member.to_string(),
                        })?
                }
                /*
                 * An unrealized control has whatever members its code declares; the box
                 * draws none of them, so any name is accepted and dropped.
                 */
                None if support.kind == Kind::Unrealized => return Ok(()),
                None => {
                    return Err(LoadError::UnsupportedProperty {
                        element: type_name.to_string(),
                        name: member.to_string(),
                    });
                }
            };
            /*
             * A brush that is not a solid colour (gradient, acrylic, image) has no paint
             * in this renderer; leaving the property unset draws the background as nothing.
             */
            if accepted.value_type == ValueType::SolidColorBrush
                && let xbf::model::Value::Element(brush) = &property.value
                && name_suffix(&brush.type_name).ends_with("Brush")
                && name_suffix(&brush.type_name) != "SolidColorBrush"
            {
                return Ok(());
            }
            /*
             * A brush named by a theme resource resolves against the palette taken from
             * Microsoft's generic.xaml. Anything else a resource names -- a length, a
             * duration, a template -- has no consumer here and is dropped below.
             */
            if let xbf::model::Value::Resource { key, .. } = &property.value {
                if accepted.value_type == ValueType::SolidColorBrush
                    && let Some(color) = crate::theme::palette::color(key)
                    && element.set(accepted, Value::Color(color)).is_ok()
                {
                    return Ok(());
                }
                if std::env::var_os("OPENCOMP_TRACE").is_some() {
                    eprintln!("openxaml: theme key {key:?} on {type_name}");
                }
                return Ok(());
            }
            let value = match convert(type_name, property, accepted.value_type) {
                Ok(value) => value,
                /*
                 * A value this renderer cannot build -- a fill that is not a solid colour, a
                 * track collection that is not a list, text that is not a string -- leaves
                 * the property at its default instead of failing the page.
                 */
                Err(_)
                    if matches!(
                        accepted.value_type,
                        ValueType::SolidColorBrush | ValueType::GridDefinitions | ValueType::Text
                    ) =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            element
                .set(accepted, value)
                .map_err(|_| LoadError::UnsupportedValue {
                    element: type_name.to_string(),
                    property: name.to_string(),
                })?;
            Ok(())
        }
    }
}

fn convert(
    element: &str,
    property: &xbf::Property,
    expected: ValueType,
) -> Result<Value, LoadError> {
    let converted = match expected {
        ValueType::Double => number(&property.value).map(Value::Number),
        ValueType::Length => {
            number(&property.value).map(|value| Value::Length(Length::Pixels(value)))
        }
        ValueType::Thickness => thickness(&property.value).map(Value::Thickness),
        ValueType::CornerRadius => corner_radius(&property.value).map(Value::CornerRadius),
        ValueType::SolidColorBrush => solid_color_brush(&property.value).map(Value::Color),
        ValueType::GridDefinitions => {
            definitions(property).map(|tracks| Value::GridDefinitions(tracks.into()))
        }
        ValueType::Text => text(&property.value).map(Value::text),
        ValueType::Bool => boolean(&property.value).map(Value::Bool),
        ValueType::TypeName => resolved_type(&property.value).map(Value::TypeName),
        ValueType::Enum(name) => enum_value(name, &property.value),
    };
    converted.ok_or_else(|| LoadError::UnsupportedValue {
        element: element.to_string(),
        property: property.name.clone(),
    })
}
