//! Storyboards: the animation data a page or a style carries, sampled at a time.
//!
//! A XAML storyboard is a list of animations; each names a target element, a property and
//! either a from/to pair or key frames. This module turns that tree into tracks and reads a
//! track at a time, which is all a still renderer needs to show a frame of it. The curves
//! come from `opencomp_core::animation`, the same sampler the composition API uses.
//!
//! What a track can move here is what the element model stores: opacity, a brush colour and
//! a render transform. A target the tree does not have is left alone; the storyboard's own
//! data is not changed to fit.

use opencomp_core::Easing;

use crate::element::{Element, Kind, Value};
use crate::property;

/// One animated property: where it points and the values it takes over time.
#[derive(Clone, Debug)]
pub struct Track {
    pub target: String,
    pub property: String,
    frames: Vec<Frame>,
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    time: f32,
    value: Key,
    /// The curve that reaches this frame from the one before it.
    easing: Easing,
    /// A discrete frame holds the previous value until its own time.
    discrete: bool,
}

/// A sampled value. A storyboard animates numbers and brushes, and an object animation
/// that swaps a brush is a colour here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Number(f32),
    Color([f32; 4]),
}

/// Read the storyboard a deferred stream holds: its direct children are the animations.
pub fn tracks(storyboard: &xbf::Element) -> Vec<Track> {
    let mut tracks = Vec::new();
    for animation in children(storyboard).iter() {
        let Some(target) = string(animation, "TargetName") else {
            continue;
        };
        let Some(property) = string(animation, "TargetProperty") else {
            continue;
        };
        let property = member(&property);
        let duration = number(animation, "Duration").unwrap_or(0.0);
        let frames = if let Some(keyframes) = child(animation, "KeyFrames") {
            keyframes.iter().filter_map(key_frame).collect::<Vec<_>>()
        } else {
            from_to(animation, duration)
        };
        if frames.is_empty() {
            continue;
        }
        tracks.push(Track {
            target: target.clone(),
            property: property.to_string(),
            frames,
        });
    }
    tracks
}

/// Sample every track at `at` seconds and write the value onto the elements it names.
/// Returns how many elements were written, which measures whether a storyboard has
/// anything to move in this tree.
pub fn apply(tree: &mut Element, tracks: &[Track], at: f32) -> usize {
    let mut written = 0;
    for track in tracks {
        let Some(key) = sample(&track.frames, at) else {
            continue;
        };
        for element in named(tree, &track.target) {
            store(element, &track.property, key);
            written += 1;
        }
    }
    written
}

fn sample(frames: &[Frame], at: f32) -> Option<Key> {
    let first = frames.first()?;
    if at <= first.time {
        return Some(first.value);
    }
    let mut previous = first;
    for frame in frames.iter().skip(1) {
        if at < frame.time {
            if frame.discrete {
                return Some(previous.value);
            }
            let span = (frame.time - previous.time).max(f32::EPSILON);
            let progress = (at - previous.time) / span;
            let eased = frame.easing.apply(progress);
            return Some(mix(previous.value, frame.value, eased));
        }
        previous = frame;
    }
    Some(previous.value)
}

fn mix(from: Key, to: Key, progress: f32) -> Key {
    match (from, to) {
        (Key::Number(from), Key::Number(to)) => Key::Number(from + (to - from) * progress),
        (Key::Color(from), Key::Color(to)) => {
            let mut out = [0.0; 4];
            for index in 0..4 {
                out[index] = from[index] + (to[index] - from[index]) * progress;
            }
            Key::Color(out)
        }
        (from, _) => from,
    }
}

fn store(element: &mut Element, property_name: &str, key: Key) {
    let (property, value) = match (property_name, key) {
        ("Opacity", Key::Number(number)) => (&property::OPACITY, Value::Number(number)),
        ("Background", Key::Color(color)) => (&property::BACKGROUND, Value::Color(color)),
        ("BorderBrush", Key::Color(color)) => (&property::BORDER_BRUSH, Value::Color(color)),
        ("X" | "TranslateX", Key::Number(number)) => {
            (&property::TRANSLATE_X, Value::Number(number))
        }
        ("Y" | "TranslateY", Key::Number(number)) => {
            (&property::TRANSLATE_Y, Value::Number(number))
        }
        ("ScaleX", Key::Number(number)) => (&property::SCALE_X, Value::Number(number)),
        ("ScaleY", Key::Number(number)) => (&property::SCALE_Y, Value::Number(number)),
        _ => return,
    };
    let _ = element.set(property, value);
}

/// The elements a target name reaches. A template can be instantiated more than once, so
/// one name is a list rather than a single element.
fn named<'a>(element: &'a mut Element, name: &str) -> Vec<&'a mut Element> {
    if element.name() == Some(name) {
        return vec![element];
    }
    let mut found = Vec::new();
    for child in element.children_mut() {
        found.extend(named(child, name));
    }
    found
}

fn key_frame(frame: &xbf::Element) -> Option<Frame> {
    let time = number(frame, "KeyTime")?;
    let value = match member_value(frame, "Value")? {
        xbf::model::Value::Float(number) => Key::Number(*number),
        xbf::model::Value::Signed(number) => Key::Number(*number as f32),
        xbf::model::Value::Resource { key, .. } => Key::Color(crate::theme::palette::color(key)?),
        _ => return None,
    };
    let spline = string(frame, "KeySpline").and_then(|text| spline(&text));
    let discrete = frame.type_name.contains("Discrete");
    Some(Frame {
        time,
        value,
        easing: spline.map_or(Easing::Linear, |points| {
            Easing::CubicBezier(points[0], points[1], points[2], points[3])
        }),
        discrete,
    })
}

/// `DoubleAnimation` with a from and a to: two frames, linear between them unless the
/// animation carries an easing function.
fn from_to(animation: &xbf::Element, duration: f32) -> Vec<Frame> {
    let Some(from) = number(animation, "From") else {
        return Vec::new();
    };
    let Some(to) = number(animation, "To") else {
        return Vec::new();
    };
    let easing = easing(animation);
    vec![
        Frame {
            time: 0.0,
            value: Key::Number(from),
            easing: Easing::Linear,
            discrete: false,
        },
        Frame {
            time: duration,
            value: Key::Number(to),
            easing,
            discrete: false,
        },
    ]
}

/// The page's easing function: `CircleEase`, `CubicEase` and their relatives, as a cubic
/// bezier. The curves below are the ease-out shapes WinUI uses by default; `EasingMode` is
/// read by the runtime and is not applied here, and any function not named is linear.
fn easing(animation: &xbf::Element) -> Easing {
    let Some(function) = child(animation, "EasingFunction") else {
        return Easing::Linear;
    };
    let Some(function) = function.first() else {
        return Easing::Linear;
    };
    let short = function.type_name.rsplit('.').next().unwrap_or("");
    match short {
        "CircleEase" => Easing::CubicBezier(0.55, 0.0, 1.0, 0.45),
        "CubicEase" => Easing::CubicBezier(0.215, 0.61, 0.355, 1.0),
        "QuadraticEase" => Easing::CubicBezier(0.11, 0.0, 0.5, 0.0),
        "SineEase" => Easing::CubicBezier(0.37, 0.0, 0.63, 1.0),
        _ => Easing::Linear,
    }
}

fn spline(text: &str) -> Option<[f32; 4]> {
    let mut points = [0.0; 4];
    let values: Vec<f32> = text
        .split([' ', ','])
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect();
    if values.len() != 4 {
        return None;
    }
    points.copy_from_slice(&values);
    Some(points)
}

/// `0:0:0.5` or a plain number of seconds.
fn duration_text(text: &str) -> Option<f32> {
    let parts: Vec<&str> = text.split(':').collect();
    let seconds = match parts.as_slice() {
        [hours, minutes, seconds] => {
            hours.parse::<f32>().ok()? * 3600.0
                + minutes.parse::<f32>().ok()? * 60.0
                + seconds.parse::<f32>().ok()?
        }
        [seconds] => seconds.parse().ok()?,
        _ => return None,
    };
    Some(seconds)
}

fn children(element: &xbf::Element) -> Vec<xbf::Element> {
    match member_value(element, "Children") {
        Some(xbf::model::Value::Element(one)) => vec![(**one).clone()],
        Some(xbf::model::Value::Elements(many)) => many.clone(),
        _ => Vec::new(),
    }
}

fn child(element: &xbf::Element, name: &str) -> Option<Vec<xbf::Element>> {
    match member_value(element, name)? {
        xbf::model::Value::Element(one) => Some(vec![(**one).clone()]),
        xbf::model::Value::Elements(many) => Some(many.clone()),
        _ => None,
    }
}

fn member_value<'a>(element: &'a xbf::Element, name: &str) -> Option<&'a xbf::model::Value> {
    element
        .properties
        .iter()
        .find(|property| member(&property.name) == name)
        .map(|property| &property.value)
}

fn string(element: &xbf::Element, name: &str) -> Option<String> {
    match member_value(element, name)? {
        xbf::model::Value::String(text) => Some(text.clone()),
        _ => None,
    }
}

fn number(element: &xbf::Element, name: &str) -> Option<f32> {
    match member_value(element, name)? {
        xbf::model::Value::Float(number) => Some(*number),
        xbf::model::Value::String(text) if name == "Duration" => duration_text(text),
        xbf::model::Value::String(text) if name == "KeyTime" => duration_text(text),
        _ => None,
    }
}

/// `Storyboard_Children` names the member `Children`, and a target property names its own:
/// `(UIElement.Opacity)` and `FrameworkElement.Width` both end at the member.
fn member(name: &str) -> &str {
    let name = name.trim_matches(['(', ')']);
    name.rsplit(['_', '.']).next().unwrap_or(name)
}

/// The kind of element a track moves, if the transform is one this renderer applies.
pub fn is_transform(kind: Kind) -> bool {
    matches!(kind, Kind::TranslateTransform | Kind::ScaleTransform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xbf::model::{Element as Wire, Property as WireProperty, Value as WireValue};

    fn element(type_name: &str, properties: Vec<(&str, WireValue)>) -> Wire {
        Wire {
            type_name: type_name.to_string(),
            namespace: String::new(),
            properties: properties
                .into_iter()
                .map(|(name, value)| WireProperty {
                    name: name.to_string(),
                    namespace: String::new(),
                    value,
                })
                .collect(),
            children: Vec::new(),
            custom: None,
            custom_substream: None,
            setters: Vec::new(),
        }
    }

    fn frame(kind: &str, time: f32, value: f32) -> Wire {
        element(
            kind,
            vec![
                ("DoubleKeyFrame_KeyTime", WireValue::Float(time)),
                ("DoubleKeyFrame_Value", WireValue::Float(value)),
            ],
        )
    }

    fn storyboard(frames: Vec<Wire>) -> Wire {
        let animation = element(
            "DoubleAnimationUsingKeyFrames",
            vec![
                ("Storyboard_TargetName", WireValue::String("Bar".into())),
                (
                    "Storyboard_TargetProperty",
                    WireValue::String("(UIElement.Opacity)".into()),
                ),
                ("Timeline_Duration", WireValue::String("0:0:1".into())),
                (
                    "DoubleAnimationUsingKeyFrames_KeyFrames",
                    WireValue::Elements(frames),
                ),
            ],
        );
        element(
            "Storyboard",
            vec![(
                "Storyboard_Children",
                WireValue::Element(Box::new(animation)),
            )],
        )
    }

    #[test]
    fn linear_frames_interpolate() {
        let tracks = tracks(&storyboard(vec![
            frame("LinearDoubleKeyFrame", 0.0, 0.0),
            frame("LinearDoubleKeyFrame", 1.0, 1.0),
        ]));
        let sampled = sample(&tracks[0].frames, 0.25).unwrap();
        assert_eq!(sampled, Key::Number(0.25));
    }

    #[test]
    fn a_discrete_frame_holds_the_value_before_it() {
        let tracks = tracks(&storyboard(vec![
            frame("DiscreteDoubleKeyFrame", 0.0, 0.0),
            frame("DiscreteDoubleKeyFrame", 1.0, 1.0),
        ]));
        assert_eq!(sample(&tracks[0].frames, 0.9).unwrap(), Key::Number(0.0));
        assert_eq!(sample(&tracks[0].frames, 1.0).unwrap(), Key::Number(1.0));
    }

    #[test]
    fn a_track_names_the_member_the_page_wrote() {
        let tracks = tracks(&storyboard(vec![
            frame("LinearDoubleKeyFrame", 0.0, 0.0),
            frame("LinearDoubleKeyFrame", 1.0, 1.0),
        ]));
        assert_eq!(tracks[0].target, "Bar");
        assert_eq!(tracks[0].property, "Opacity");
    }

    #[test]
    fn a_storyboard_without_values_has_no_track() {
        let animation = element(
            "DoubleAnimation",
            vec![
                ("Storyboard_TargetName", WireValue::String("Bar".into())),
                ("Storyboard_TargetProperty", WireValue::String("X".into())),
            ],
        );
        let storyboard = element(
            "Storyboard",
            vec![(
                "Storyboard_Children",
                WireValue::Element(Box::new(animation)),
            )],
        );
        assert!(tracks(&storyboard).is_empty());
    }
}
