// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{RegExp, reg_exp};

// When passing user input into querySelector(All) the embedded string must not alter
// the semantics of the query. This escape function is safe to use when we know the
// provided value is going to be wrapped in double quotes as part of an attribute selector
// Do not use it anywhere else
// we escape double quotes and backslashes
thread_local! {
    static ESCAPE_SELECTOR_ATTRIBUTE_VALUE_INSIDE_DOUBLE_QUOTES_REGEX: &'static RegExp =
        reg_exp::new(r#"[\n\"\\]"#, "g");
}

js::export_default! {
    pub fn escape_selector_attribute_value_inside_double_quotes(value: &str) -> String {
        ESCAPE_SELECTOR_ATTRIBUTE_VALUE_INSIDE_DOUBLE_QUOTES_REGEX.with(|regex| {
            reg_exp::replace(value, regex, |ch: &str| {
                "\\".to_owned()
                    + &js::number::to_string(js::string::char_code_at(ch, 0), Some(16))
                    + " "
            })
        })
    }
}
