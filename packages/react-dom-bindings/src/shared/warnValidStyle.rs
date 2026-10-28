// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Dict, Kind, RegExp, Unknown, dict, reg_exp};
use shared::environment::__DEV__;
use webapi::console;

thread_local! {
    // 'msTransform' is correct, but the other prefixes should be capitalized
    static BAD_VENDORED_STYLE_NAME_PATTERN: &'static RegExp = reg_exp::new("^(?:webkit|moz|o)[A-Z]", "");
    static MS_PATTERN: &'static RegExp = reg_exp::new("^-ms-", "");
    static HYPHEN_PATTERN: &'static RegExp = reg_exp::new("-(.)", "g");

    // style values shouldn't contain a semicolon
    static BAD_STYLE_VALUE_WITH_SEMICOLON_PATTERN: &'static RegExp = reg_exp::new(";\\s*$", "");

    static WARNED_STYLE_NAMES: &'static Dict<bool> = dict::new();
    static WARNED_STYLE_VALUES: &'static Dict<bool> = dict::new();
    #[cfg_attr(rust_js, rust_js::name = "warnedForNaNValue")]
    static WARNED_FOR_NAN_VALUE: Cell<bool> = const { Cell::new(false) };
    static WARNED_FOR_INFINITY_VALUE: Cell<bool> = const { Cell::new(false) };
}

fn camelize(string: &str) -> String {
    HYPHEN_PATTERN.with(|hyphen_pattern| {
        reg_exp::replace(
            string,
            hyphen_pattern,
            |_: &str, character: Option<&str>| character.unwrap_or_default().to_uppercase(),
        )
    })
}

fn warn_hyphenated_style_name(name: &str) {
    if __DEV__ {
        if WARNED_STYLE_NAMES.with(|warned_style_names| {
            js::unknown(*warned_style_names).has_own_property(name)
                && dict::get(warned_style_names, name).is_some_and(|warned| *warned)
        }) {
            return;
        }

        WARNED_STYLE_NAMES.with(|warned_style_names| dict::set(warned_style_names, name, true));
        console::error((
            "Unsupported style property %s. Did you mean %s?",
            name,
            // As Andi Smith suggests
            // (http://www.andismith.com/blog/2012/02/modernizr-prefixed/), an `-ms` prefix
            // is converted to lowercase `ms`.
            camelize(&MS_PATTERN.with(|ms_pattern| reg_exp::replace(name, ms_pattern, "ms-"))),
        ));
    }
}

fn warn_bad_vendored_style_name(name: &str) {
    if __DEV__ {
        if WARNED_STYLE_NAMES.with(|warned_style_names| {
            js::unknown(*warned_style_names).has_own_property(name)
                && dict::get(warned_style_names, name).is_some_and(|warned| *warned)
        }) {
            return;
        }

        WARNED_STYLE_NAMES.with(|warned_style_names| dict::set(warned_style_names, name, true));
        console::error((
            "Unsupported vendor-prefixed style property %s. Did you mean %s?",
            name,
            js::string::char_at(name, 0).to_uppercase() + &js::string::slice(name, 1.0, None),
        ));
    }
}

fn warn_style_value_with_semicolon(name: &str, value: &str) {
    if __DEV__ {
        if WARNED_STYLE_VALUES.with(|warned_style_values| {
            js::unknown(*warned_style_values).has_own_property(value)
                && dict::get(warned_style_values, value).is_some_and(|warned| *warned)
        }) {
            return;
        }

        WARNED_STYLE_VALUES.with(|warned_style_values| dict::set(warned_style_values, value, true));
        console::error((
            concat!(
                "Style property values shouldn't contain a semicolon. ",
                "Try \"%s: %s\" instead."
            ),
            name,
            BAD_STYLE_VALUE_WITH_SEMICOLON_PATTERN
                .with(|pattern| reg_exp::replace(value, pattern, "")),
        ));
    }
}

#[cfg_attr(rust_js, rust_js::name = "warnStyleValueIsNaN")]
fn warn_style_value_is_nan(name: &str, _value: f64) {
    if __DEV__ {
        if WARNED_FOR_NAN_VALUE.get() {
            return;
        }

        WARNED_FOR_NAN_VALUE.set(true);
        console::error((
            "`NaN` is an invalid value for the `%s` css style property.",
            name,
        ));
    }
}

fn warn_style_value_is_infinity(name: &str, _value: f64) {
    if __DEV__ {
        if WARNED_FOR_INFINITY_VALUE.get() {
            return;
        }

        WARNED_FOR_INFINITY_VALUE.set(true);
        console::error((
            "`Infinity` is an invalid value for the `%s` css style property.",
            name,
        ));
    }
}

pub fn warn_valid_style(name: &str, value: Option<&Unknown>) {
    if __DEV__ {
        if name.contains('-') {
            warn_hyphenated_style_name(name);
        } else if BAD_VENDORED_STYLE_NAME_PATTERN.with(|pattern| pattern.test(name)) {
            warn_bad_vendored_style_name(name);
        } else if BAD_STYLE_VALUE_WITH_SEMICOLON_PATTERN
            .with(|pattern| pattern.test(&js::string(value)))
        {
            warn_style_value_with_semicolon(name, &js::string(value));
        }

        if let Kind::Number(value) = js::classify(value) {
            if js::is_nan(value) {
                warn_style_value_is_nan(name, value);
            } else if !js::is_finite(value) {
                warn_style_value_is_infinity(name, value);
            }
        }
    }
}

js::export_default!(warn_valid_style);
