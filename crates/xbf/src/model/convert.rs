//! The conversions a wire value needs on its way into the model: a type-converted
//! constant's text form, and the key a resource reference names.

use crate::node::Value as WireValue;

use super::Value;
use super::resolve::convert;

/*
 * A type-converted constant is the text form of the property's type: "8" for a
 * CornerRadius, "8,8,0,0" for its four corners. The wire carries no other shape
 * information, so one number becomes a scalar and four become a thickness.
 */
pub(super) fn converted(value: &WireValue) -> Value {
    let resolved = convert(value);
    let Value::String(text) = &resolved else {
        return resolved;
    };
    let parts: Vec<f32> = text
        .split(',')
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    match parts.len() {
        1 => Value::Float(parts[0]),
        4 => Value::Thickness([parts[0], parts[1], parts[2], parts[3]]),
        _ => resolved,
    }
}

pub(super) fn key_of(value: &WireValue) -> String {
    match value {
        WireValue::SharedString(value) | WireValue::UniqueString(value) => value.clone(),
        other => other.to_string(),
    }
}
