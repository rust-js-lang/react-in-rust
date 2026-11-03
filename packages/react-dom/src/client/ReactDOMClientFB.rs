// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Map, Set, Unknown, reg_exp};
use shared::environment::__DEV__;
use shared::react_types::{ReactNodeList, ReactPortal};
use webapi::{console, window};

use shared::react_feature_flags::DISABLE_LEGACY_MODE;
use react_dom_bindings::react_dom_container::is_valid_container;
#[cfg_attr(rust_js, rust_js::name = "unstable_createEventHandle")]
pub use react_dom_bindings::react_dom_event_handle::create_event_handle as unstable_create_event_handle;
#[cfg_attr(rust_js, rust_js::name = "unstable_runWithPriority")]
pub use react_dom_bindings::react_dom_update_priority::run_with_priority as unstable_run_with_priority;
use crate::react_dom_flush_sync::{FLUSH_SYNC as FLUSH_SYNC_ISOMORPHIC, FlushSync};

use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;
use react_reconciler::react_portal::create_portal as create_portal_impl;
use shared::execution_environment::CAN_USE_DOM;
#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

use crate::ensure_correct_isomorphic_react_version::ensure_correct_isomorphic_react_version;

js::on_load! {
    ensure_correct_isomorphic_react_version();
}

pub use crate::react_dom_browser::browser;
use react_dom_bindings::react_dom_component_tree::{
    get_fiber_current_props_from_node, get_instance_from_node, get_node_from_instance,
};
use react_dom_bindings::react_dom_controlled_component::{
    enqueue_state_restore, restore_state_if_needed,
};
use crate::react_dom_shared_internals_fb::INTERNALS;

pub use crate::react_dom_float::{
    preconnect, prefetch_dns, preinit, preinit_module, preload, preload_module,
};
pub use react_dom_bindings::react_dom_form_actions::{
    request_form_reset, use_form_state, use_form_status,
};

js::on_load! {
    if __DEV__
        && (!js::is_function(js::class::<Map<Unknown, ()>>)
            // $FlowFixMe[prop-missing] Flow incorrectly thinks Map has no prototype
            || js::loose_equals(js::class::<Map<Unknown, ()>>().prototype(), js::null::<&Unknown>())
            || !js::is_function((
                js::class::<Map<Unknown, ()>>().prototype(),
                Map::<Unknown, ()>::for_each
                    as fn(&Map<Unknown, ()>, fn((), &'static Unknown, &Map<Unknown, ()>)),
            ))
            || !js::is_function(js::class::<Set<Unknown>>)
            // $FlowFixMe[prop-missing] Flow incorrectly thinks Set has no prototype
            || js::loose_equals(js::class::<Set<Unknown>>().prototype(), js::null::<&Unknown>())
            || !js::is_function((js::class::<Set<Unknown>>().prototype(), Set::<Unknown>::clear))
            || !js::is_function((
                js::class::<Set<Unknown>>().prototype(),
                Set::<Unknown>::for_each as fn(&Set<Unknown>, fn(&'static Unknown)),
            )))
    {
        console::error((concat!(
            "React depends on Map and Set built-in types. Make sure that you load a ",
            "polyfill in older browsers. https://react.dev/link/react-polyfills"
        ),));
    }
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
    // $FlowFixMe[incompatible-type] The Flow type is opaque but there's no way to actually create it.
    create_portal_impl(children, container, js::null(), key)
}

fn flush_sync_from_reconciler(
    r#fn: Option<&dyn Fn() -> Option<&'static Unknown>>,
) -> Option<&'static Unknown> {
    if __DEV__ && ReactFiberReconciler.is_already_rendering() {
        console::error((concat!(
            "flushSync was called from inside a lifecycle method. React cannot ",
            "flush when React is already rendering. Consider moving this call to ",
            "a scheduler task or micro task."
        ),));
    }
    // $FlowFixMe[incompatible-type]
    ReactFiberReconciler.flush_sync_from_reconciler(r#fn).flatten()
}

#[cfg_attr(rust_js, rust_js::name = "flushSync")]
pub const FLUSH_SYNC: FlushSync = if DISABLE_LEGACY_MODE {
    FLUSH_SYNC_ISOMORPHIC
} else {
    flush_sync_from_reconciler
};

#[cfg_attr(rust_js, rust_js::name = "findDOMNode")]
fn find_dom_node(
    // A class component.
    component_or_element: &'static Unknown,
) -> Option<&'static Unknown> {
    ReactFiberReconciler.find_host_instance(component_or_element)
}

js::on_load! {
    // Expose findDOMNode on internals
    INTERNALS.with(|internals| internals.find_dom_node.set(Some(&find_dom_node)));
}

#[cfg_attr(rust_js, rust_js::name = "unstable_batchedUpdates")]
pub fn unstable_batched_updates<A, R>(r#fn: fn(A) -> R, a: A) -> R {
    // batchedUpdates was a legacy mode feature that is a no-op outside of
    // legacy mode. In 19, we made it an actual no-op, but we're keeping it
    // for now since there may be libraries that still include it.
    r#fn(a)
}

js::on_load! {
    // Keep in sync with ReactTestUtils.js.
    // This is an array for better minification.
    INTERNALS.with(|internals| {
        internals.events /* Events */.set(Some(js::unknown(&(
            get_instance_from_node,
            get_node_from_instance,
            get_fiber_current_props_from_node,
            enqueue_state_restore,
            restore_state_if_needed,
            unstable_batched_updates::<Option<&'static Unknown>, Option<&'static Unknown>>,
        ))))
    });
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "foundDevTools")]
    static FOUND_DEV_TOOLS: bool = ReactFiberReconciler.inject_into_dev_tools();
}

js::on_load! {
    if __DEV__
        && !FOUND_DEV_TOOLS.with(|&found_dev_tools| found_dev_tools)
        && CAN_USE_DOM.with(|&can_use_dom| can_use_dom)
        && window.top() == Some(window.self_())
    {
        // If we're in Chrome or Firefox, provide a download link if not installed.
        if (webapi::navigator.user_agent().contains("Chrome")
            && !webapi::navigator.user_agent().contains("Edge"))
            || webapi::navigator.user_agent().contains("Firefox")
        {
            let protocol = window.location().protocol();
            // Don't warn in exotic cases like chrome-extension://.
            if reg_exp::new("^(https?|file):$", "").test(&protocol) {
                // eslint-disable-next-line react-internal/no-production-logging
                console::info((
                    format!(
                        "%cDownload the React DevTools for a better development experience: https://react.dev/link/react-devtools{}",
                        if protocol == "file:" {
                            "\nYou might need to use a local HTTP server (instead of file://): https://react.dev/link/react-devtools-faq"
                        } else {
                            ""
                        }
                    ),
                    "font-weight:bold",
                ));
            }
        }
    }
}
