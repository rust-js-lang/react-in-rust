// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Kind, Unknown};
use shared::environment::__DEV__;
use webapi::console;

thread_local! {
    static DID_WARN_VALUE_NULL: Cell<bool> = const { Cell::new(false) };
}

pub fn validate_properties(r#type: &str, props: Option<&Unknown>) {
    if __DEV__ {
        if r#type != "input" && r#type != "textarea" && r#type != "select" {
            return;
        }

        if let Some(props) = props
            && matches!(js::classify(js::get(props, "value")), Kind::Null)
            && !DID_WARN_VALUE_NULL.get()
        {
            DID_WARN_VALUE_NULL.set(true);
            if r#type == "select" && js::truthy(js::get(props, "multiple")) {
                console::error((
                    concat!(
                        "`value` prop on `%s` should not be null. ",
                        "Consider using an empty array when `multiple` is set to `true` ",
                        "to clear the component or `undefined` for uncontrolled components."
                    ),
                    r#type,
                ));
            } else {
                console::error((
                    concat!(
                        "`value` prop on `%s` should not be null. ",
                        "Consider using an empty string to clear the component or `undefined` ",
                        "for uncontrolled components."
                    ),
                    r#type,
                ));
            }
        }
    }
}
