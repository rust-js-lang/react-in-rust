// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, Unknown, dict};
use shared::environment::__DEV__;
use webapi::console;

thread_local! {
    static HAS_READ_ONLY_VALUE: &'static Dict<bool> = dict::from_entries(vec![
        ("button", true),
        ("checkbox", true),
        ("image", true),
        ("hidden", true),
        ("radio", true),
        ("reset", true),
        ("submit", true),
    ]);
}

pub fn check_controlled_value_props(tag_name: &str, props: &Unknown) {
    if __DEV__ {
        if !(HAS_READ_ONLY_VALUE.with(|has_read_only_value| {
            dict::get(has_read_only_value, &js::string(js::get(props, "type")))
                .is_some_and(|read_only| *read_only)
        }) || js::truthy(js::get(props, "onChange"))
            || js::truthy(js::get(props, "onInput"))
            || js::truthy(js::get(props, "readOnly"))
            || js::truthy(js::get(props, "disabled"))
            || js::get(props, "value").is_none())
        {
            if tag_name == "select" {
                console::error((concat!(
                    "You provided a `value` prop to a form field without an ",
                    "`onChange` handler. This will render a read-only field. If ",
                    "the field should be mutable use `defaultValue`. Otherwise, set `onChange`."
                ),));
            } else {
                console::error((concat!(
                    "You provided a `value` prop to a form field without an ",
                    "`onChange` handler. This will render a read-only field. If ",
                    "the field should be mutable use `defaultValue`. Otherwise, set either `onChange` or `readOnly`."
                ),));
            }
        }

        if !(js::truthy(js::get(props, "onChange"))
            || js::truthy(js::get(props, "readOnly"))
            || js::truthy(js::get(props, "disabled"))
            || js::get(props, "checked").is_none())
        {
            console::error((concat!(
                "You provided a `checked` prop to a form field without an ",
                "`onChange` handler. This will render a read-only field. If ",
                "the field should be mutable use `defaultChecked`. Otherwise, ",
                "set either `onChange` or `readOnly`."
            ),));
        }
    }
}
