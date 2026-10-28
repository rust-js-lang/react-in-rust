// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};
use shared::check_string_coercion::check_form_field_value_string_coercion;
use shared::environment::__DEV__;

/// A value that's safe to make text of: a boolean, a number, a bigint, an
/// object, text, `null` or `undefined`, only had of `getToStringValue`.
pub type ToStringValue = Option<&'static Unknown>;

// Flow does not allow string concatenation of most non-string types. To work
// around this limitation, we use an opaque type that can only be obtained by
// passing the value through getToStringValue first.
pub fn to_string(value: ToStringValue) -> String {
    // The coercion safety check is performed in getToStringValue().
    // eslint-disable-next-line react-internal/safe-string-coercion
    js::concat_text(value).unwrap()
}

pub fn get_to_string_value(value: Option<&'static Unknown>) -> ToStringValue {
    match js::classify(value) {
        Kind::BigInt(_) | Kind::Bool(_) | Kind::Number(_) | Kind::String(_) | Kind::Undefined => {
            value
        }
        Kind::Object(_) | Kind::Array(_) | Kind::Null => {
            if __DEV__ {
                check_form_field_value_string_coercion(value);
            }
            value
        }
        // function, symbol are assigned as empty strings
        _ => Some(js::unknown("")),
    }
}
