// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::Unknown;
use react_isomorphic::react_client::CHILDREN;
use shared::environment::__DEV__;
use webapi::{Element, console};

thread_local! {
    static DID_WARN_SELECTED_SET_ON_OPTION: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_INVALID_CHILD: Cell<bool> = const { Cell::new(false) };
    #[cfg_attr(rust_js, rust_js::name = "didWarnInvalidInnerHTML")]
    static DID_WARN_INVALID_INNER_HTML: Cell<bool> = const { Cell::new(false) };
}

/// Implements an <option> host component that warns when `selected` is set.
pub fn validate_option_props(_element: &Element, props: &Unknown) {
    if __DEV__ {
        // If a value is not provided, then the children must be simple.
        if js::get(props, "value").is_none() {
            if js::type_of(js::get(props, "children")) == "object"
                && js::get(props, "children").is_some()
            {
                CHILDREN.with(|children| {
                    (children.for_each)(
                        js::get(props, "children"),
                        &|child| {
                            if child.is_none() {
                                return;
                            }
                            if js::type_of(child) == "string"
                                || js::type_of(child) == "number"
                                || js::type_of(child) == "bigint"
                            {
                                return;
                            }
                            if !DID_WARN_INVALID_CHILD.get() {
                                DID_WARN_INVALID_CHILD.set(true);
                                console::error((concat!(
                                    "Cannot infer the option value of complex children. ",
                                    "Pass a `value` prop or use a plain string as children to <option>."
                                ),));
                            }
                        },
                        None,
                    );
                });
            } else if js::get(props, "dangerouslySetInnerHTML").is_some()
                && !DID_WARN_INVALID_INNER_HTML.get()
            {
                DID_WARN_INVALID_INNER_HTML.set(true);
                console::error((concat!(
                    "Pass a `value` prop if you set dangerouslyInnerHTML so React knows ",
                    "which value should be selected."
                ),));
            }
        }

        // TODO: Remove support for `selected` in <option>.
        if js::get(props, "selected").is_some() && !DID_WARN_SELECTED_SET_ON_OPTION.get() {
            console::error((concat!(
                "Use the `defaultValue` or `value` props on <select> instead of ",
                "setting `selected` on <option>."
            ),));
            DID_WARN_SELECTED_SET_ON_OPTION.set(true);
        }
    }
}
