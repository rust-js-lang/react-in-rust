// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Map, Set, Unknown};
use react_dom_bindings::react_dom_container::is_valid_container;
use react_reconciler::react_portal::create_portal as create_portal_impl;
use shared::environment::__DEV__;
use shared::react_types::{ReactNodeList, ReactPortal};
use webapi::console;

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

pub use crate::react_dom_browser::browser;
pub use crate::react_dom_float::{
    preconnect, prefetch_dns, preinit, preinit_module, preload, preload_module,
};
#[cfg_attr(rust_js, rust_js::name = "flushSync")]
pub use crate::react_dom_flush_sync::FLUSH_SYNC;
pub use react_dom_bindings::react_dom_form_actions::{
    request_form_reset, use_form_state, use_form_status,
};

js::on_load! {
    if __DEV__
        && (!js::is_function(js::class::<Map<Unknown, ()>>)
            || js::loose_equals(js::class::<Map<Unknown, ()>>().prototype(), js::null::<&Unknown>())
            || !js::is_function((
                js::class::<Map<Unknown, ()>>().prototype(),
                Map::<Unknown, ()>::for_each
                    as fn(&Map<Unknown, ()>, fn((), &'static Unknown, &Map<Unknown, ()>)),
            ))
            || !js::is_function(js::class::<Set<Unknown>>)
            || js::loose_equals(js::class::<Set<Unknown>>().prototype(), js::null::<&Unknown>())
            || !js::is_function((js::class::<Set<Unknown>>().prototype(), Set::<Unknown>::clear))
            || !js::is_function((
                js::class::<Set<Unknown>>().prototype(),
                Set::<Unknown>::for_each as fn(&Set<Unknown>, fn(&'static Unknown)),
            )))
    {
        console::error((concat!(
            "React depends on Map and Set built-in types. Make sure that you load a ",
            "polyfill in older browsers. https://reactjs.org/link/react-polyfills"
        ),));
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_batchedUpdates")]
pub fn batched_updates<A, R>(r#fn: impl FnOnce(A) -> R, a: A) -> R {
    // batchedUpdates is now just a passthrough noop
    r#fn(a)
}

pub fn create_portal(
    children: Option<&'static ReactNodeList>,
    // An `Element` or a `DocumentFragment`.
    container: Option<&'static Unknown>,
    key: Option<&'static Unknown>,
) -> ReactPortal {
    if !is_valid_container(container) {
        panic!("Target container is not a DOM element.");
    }

    // TODO: pass ReactDOM portal implementation as third argument
    create_portal_impl(children, container, js::null(), key)
}
