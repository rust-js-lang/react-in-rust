// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// The `'' + value` pattern (used in perf-sensitive code) throws for Symbol
// and Temporal.* types. See https://github.com/react/react/pull/22064.
//
// The functions in this module will throw an easier-to-understand,
// easier-to-debug exception with a clear errors message message explaining the
// problem. (Instead of a confusing exception thrown inside the implementation
// of the `value` object).

use js::{JsError, Unknown};
use webapi::console;

use crate::environment::__DEV__;

fn type_name(value: Option<&Unknown>) -> String {
    // toStringTag is needed for namespaced types like Temporal.Instant
    let has_to_string_tag = js::is_defined(js::symbol::to_string_tag);
    let r#type = value
        .filter(|_| has_to_string_tag)
        .and_then(|value| js::get(value, js::symbol::to_string_tag))
        .filter(|tag| js::truthy(Some(*tag)))
        .or_else(|| {
            value
                .and_then(|value| js::get(value, "constructor"))
                .and_then(|c| js::get(c, "name"))
        })
        .filter(|name| js::truthy(Some(*name)));
    unsafe { js::cast::<Option<String>>(r#type) }.unwrap_or_else(|| "Object".to_string())
}

fn will_coercion_throw(value: Option<&Unknown>) -> bool {
    test_string_coercion(value).is_err()
}

#[inline(never)]
fn test_string_coercion(value: Option<&Unknown>) -> Result<String, &'static JsError> {
    // If you ended up here by following an exception call stack, here's what's
    // happened: you supplied an object or symbol value to React (as a prop, key,
    // DOM attribute, CSS property, string ref, etc.) and when React tried to
    // coerce it to a string using `'' + value`, an exception was thrown.
    //
    // The most common types that will cause this exception are `Symbol` instances
    // and Temporal objects like `Temporal.Instant`. But any object that has a
    // `valueOf` or `[Symbol.toPrimitive]` method that throws will also cause this
    // exception. (Library authors do this to prevent users from using built-in
    // numeric operators like `+` or comparison operators like `>=` because custom
    // methods are needed to perform accurate arithmetic or comparison.)
    //
    // To fix the problem, coerce this object or symbol value to a string before
    // passing it to React. The most reliable way is usually `String(value)`.
    //
    // To find which value is throwing, check the browser or debugger console.
    // Before this exception was thrown, there should be `console.error` output
    // that shows the type (Symbol, Temporal.PlainDate, etc.) that caused the
    // problem and how that type was used: key, atrribute, input value prop, etc.
    // In most cases, this console output also shows the component and its
    // ancestor components where the exception happened.
    js::concat_text(value)
}

pub fn check_attribute_string_coercion(
    value: Option<&Unknown>,
    attribute_name: &str,
) -> Option<String> {
    if __DEV__ && will_coercion_throw(value) {
        console::error((
            concat!(
                "The provided `%s` attribute is an unsupported type %s.",
                " This value must be coerced to a string before using it here."
            ),
            attribute_name,
            type_name(value),
        ));
        return Some(test_string_coercion(value).unwrap()); // throw (to help callers find troubleshooting comments)
    }
    None
}

pub fn check_key_string_coercion(value: Option<&Unknown>) -> Option<String> {
    if __DEV__ && will_coercion_throw(value) {
        console::error((
            concat!(
                "The provided key is an unsupported type %s.",
                " This value must be coerced to a string before using it here."
            ),
            type_name(value),
        ));
        return Some(test_string_coercion(value).unwrap()); // throw (to help callers find troubleshooting comments)
    }
    None
}

pub fn check_prop_string_coercion(value: Option<&Unknown>, prop_name: &str) -> Option<String> {
    if __DEV__ && will_coercion_throw(value) {
        console::error((
            concat!(
                "The provided `%s` prop is an unsupported type %s.",
                " This value must be coerced to a string before using it here."
            ),
            prop_name,
            type_name(value),
        ));
        return Some(test_string_coercion(value).unwrap()); // throw (to help callers find troubleshooting comments)
    }
    None
}

pub fn check_option_string_coercion(value: Option<&Unknown>, prop_name: &str) -> Option<String> {
    if __DEV__ && will_coercion_throw(value) {
        console::error((
            concat!(
                "The provided `%s` option is an unsupported type %s.",
                " This value must be coerced to a string before using it here."
            ),
            prop_name,
            type_name(value),
        ));
        return Some(test_string_coercion(value).unwrap()); // throw (to help callers find troubleshooting comments)
    }
    None
}

#[cfg_attr(rust_js, rust_js::name = "checkCSSPropertyStringCoercion")]
pub fn check_css_property_string_coercion(
    value: Option<&Unknown>,
    prop_name: &str,
) -> Option<String> {
    if __DEV__ && will_coercion_throw(value) {
        console::error((
            concat!(
                "The provided `%s` CSS property is an unsupported type %s.",
                " This value must be coerced to a string before using it here."
            ),
            prop_name,
            type_name(value),
        ));
        return Some(test_string_coercion(value).unwrap()); // throw (to help callers find troubleshooting comments)
    }
    None
}

pub fn check_html_string_coercion(value: Option<&Unknown>) -> Option<String> {
    if __DEV__ && will_coercion_throw(value) {
        console::error((
            concat!(
                "The provided HTML markup uses a value of unsupported type %s.",
                " This value must be coerced to a string before using it here."
            ),
            type_name(value),
        ));
        return Some(test_string_coercion(value).unwrap()); // throw (to help callers find troubleshooting comments)
    }
    None
}

pub fn check_form_field_value_string_coercion(value: Option<&Unknown>) -> Option<String> {
    if __DEV__ && will_coercion_throw(value) {
        console::error((
            concat!(
                "Form field values (value, checked, defaultValue, or defaultChecked props)",
                " must be strings, not %s.",
                " This value must be coerced to a string before using it here."
            ),
            type_name(value),
        ));
        return Some(test_string_coercion(value).unwrap()); // throw (to help callers find troubleshooting comments)
    }
    None
}
