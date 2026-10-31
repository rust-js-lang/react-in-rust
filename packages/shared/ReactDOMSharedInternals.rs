// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

use js::Unknown;
use webapi::HTMLFormElement;

/// The state React DOM shares with its renderers, the `react-dom` package's own.
pub struct ReactDOMInternals(PhantomData<js::JsObject>);

/// `preinitStyle(href, precedence, options)`: the precedence and options are
/// what the caller gave.
pub type PreinitStyle = dyn Fn(&str, Option<&'static Unknown>, Option<&'static Unknown>);

/// React DOM's `HostDispatcher`, `ReactDOMSharedInternals.d`: what
/// `flushSync`, a form's reset and the resource hints call, the current
/// renderer's, which calls the one it replaced.
pub struct HostDispatcher {
    /// `flushSyncWork`: whether any root was rendering.
    pub f: &'static dyn Fn() -> bool,
    /// `requestFormReset`.
    pub r: &'static dyn Fn(&HTMLFormElement),
    /// `prefetchDNS(href)`.
    #[cfg_attr(rust_js, rust_js::name = "D")]
    pub prefetch_dns: &'static dyn Fn(&str),
    /// `preconnect(href, crossOrigin)`.
    #[cfg_attr(rust_js, rust_js::name = "C")]
    pub preconnect: &'static dyn Fn(&str, Option<&'static Unknown>),
    /// `preload(href, as, options)`.
    #[cfg_attr(rust_js, rust_js::name = "L")]
    pub preload: &'static dyn Fn(&str, &str, Option<&'static Unknown>),
    /// `preloadModule(href, options)`.
    pub m: &'static dyn Fn(&str, Option<&'static Unknown>),
    /// `preinitScript(src, options)`.
    #[cfg_attr(rust_js, rust_js::name = "X")]
    pub preinit_script: &'static dyn Fn(&str, Option<&'static Unknown>),
    /// `preinitStyle(href, precedence, options)`.
    #[cfg_attr(rust_js, rust_js::name = "S")]
    pub preinit_style: &'static PreinitStyle,
    /// `preinitModuleScript(src, options)`.
    #[cfg_attr(rust_js, rust_js::name = "M")]
    pub preinit_module_script: &'static dyn Fn(&str, Option<&'static Unknown>),
}

unsafe impl js::Defined for HostDispatcher {}

impl ReactDOMInternals {
    /// `d`, the current dispatcher (`ReactDOMCurrentDispatcher`).
    #[cfg_attr(rust_js, rust_js::link_name = "get d")]
    pub fn d(&self) -> &'static HostDispatcher {
        unreachable!()
    }

    /// `d = dispatcher`: the renderer's, in place of the one before.
    #[cfg_attr(rust_js, rust_js::link_name = "set d")]
    pub fn set_d(&self, d: HostDispatcher) {
        let _ = d;
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

unsafe extern "Rust" {
    #[cfg_attr(rust_js, rust_js::name = "ReactDOMSharedInternals")]
    #[link_name = "react-dom#* as ReactDOM.__DOM_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    pub safe static REACT_DOM_SHARED_INTERNALS: &'static ReactDOMInternals;
}

js::export_default!(REACT_DOM_SHARED_INTERNALS);
