// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

/// The state React DOM shares with its renderers, the `react-dom` package's own.
pub struct ReactDOMInternals(PhantomData<js::JsObject>);

unsafe extern "Rust" {
    #[cfg_attr(rust_js, rust_js::name = "ReactDOMSharedInternals")]
    #[link_name = "react-dom#* as ReactDOM.__DOM_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    pub safe static REACT_DOM_SHARED_INTERNALS: &'static ReactDOMInternals;
}

js::export_default!(REACT_DOM_SHARED_INTERNALS);
