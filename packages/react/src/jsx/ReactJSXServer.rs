// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
use js::Unknown;
use shared::environment::__DEV__;
#[cfg_attr(rust_js, rust_js::name = "Fragment")]
pub use shared::react_symbols::REACT_FRAGMENT_TYPE as FRAGMENT;

#[cfg_attr(rust_js, rust_js::name = "_jsxDEV")]
use crate::react_jsx_element::jsx_dev as _jsx_dev;
use crate::react_jsx_element::{
    jsx_prod, jsx_prod_signature_running_in_dev_with_dynamic_children,
    jsx_prod_signature_running_in_dev_with_static_children,
};

thread_local! {
    pub static JSX: &'static Unknown = if __DEV__ {
        js::unknown_of(jsx_prod_signature_running_in_dev_with_dynamic_children)
    } else {
        js::unknown_of(jsx_prod)
    };
    // we may want to special case jsxs internally to take advantage of static children.
    // for now we can ship identical prod functions
    pub static JSXS: &'static Unknown = if __DEV__ {
        js::unknown_of(jsx_prod_signature_running_in_dev_with_static_children)
    } else {
        js::unknown_of(jsx_prod)
    };

    #[cfg_attr(rust_js, rust_js::name = "jsxDEV")]
    pub static JSX_DEV: Option<&'static Unknown> = if __DEV__ {
        Some(js::unknown_of(_jsx_dev))
    } else {
        None
    };
}
