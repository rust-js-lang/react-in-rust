// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Map, Set, Unknown, reg_exp};
use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;
use shared::environment::__DEV__;
use shared::execution_environment::CAN_USE_DOM;
use shared::react_dom_shared_internals::REACT_DOM_SHARED_INTERNALS as INTERNALS;
use webapi::{console, window};

use crate::ensure_correct_isomorphic_react_version::ensure_correct_isomorphic_react_version;
pub use crate::react_dom_root::{create_root, hydrate_root};

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

js::on_load! {
    ensure_correct_isomorphic_react_version();

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
            "polyfill in older browsers. https://react.dev/link/react-polyfills"
        ),));
    }
}

#[cfg_attr(rust_js, rust_js::name = "findDOMNode")]
fn find_dom_node(
    // A class component.
    component_or_element: &'static Unknown,
) -> Option<&'static Unknown> {
    ReactFiberReconciler.find_host_instance(component_or_element)
}

js::on_load! {
    // Expose findDOMNode on internals
    INTERNALS.set_find_dom_node(Some(&find_dom_node));
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
