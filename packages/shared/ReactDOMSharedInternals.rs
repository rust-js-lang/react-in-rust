// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

/// The state React DOM shares with its renderers, the `react-dom` package's own.
pub struct ReactDOMInternals(PhantomData<js::JsObject>);

/// React DOM's `HostDispatcher`, `ReactDOMSharedInternals.d`.
pub struct HostDispatcher(PhantomData<js::JsObject>);

impl ReactDOMInternals {
    /// `d`, the current dispatcher (`ReactDOMCurrentDispatcher`).
    #[cfg_attr(rust_js, rust_js::link_name = "get d")]
    pub fn d(&self) -> &'static HostDispatcher {
        unreachable!()
    }

    /// `p`, the current update priority: the reconciler's `EventPriority`, a lane.
    #[cfg_attr(rust_js, rust_js::link_name = "get p")]
    pub fn p(&self) -> i32 {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "set p")]
    pub fn set_p(&self, current_update_priority: i32) {
        let _ = current_update_priority;
        unreachable!()
    }
}

impl HostDispatcher {
    /// `r`, `requestFormReset`.
    #[cfg_attr(rust_js, rust_js::link_name = "r")]
    pub fn r(&self, form: &webapi::HTMLFormElement) {
        let _ = form;
        unreachable!()
    }
}

unsafe extern "Rust" {
    #[cfg_attr(rust_js, rust_js::name = "ReactDOMSharedInternals")]
    #[link_name = "react-dom#* as ReactDOM.__DOM_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    pub safe static REACT_DOM_SHARED_INTERNALS: &'static ReactDOMInternals;
}

js::export_default!(REACT_DOM_SHARED_INTERNALS);
