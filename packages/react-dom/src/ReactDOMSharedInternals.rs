// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::Unknown;
use shared::noop::noop;
use shared::react_dom_shared_internals::HostDispatcher;
use webapi::HTMLFormElement;

// This should line up with NoEventPriority from react-reconciler/src/ReactEventPriorities
// but we can't depend on the react-reconciler from this isomorphic code.
#[cfg_attr(rust_js, rust_js::name = "NoEventPriority")]
pub const NO_EVENT_PRIORITY: i32 = 0;

/// `findDOMNode(componentOrElement)`: the host instance of a class component,
/// an `Element` or a `Text`, if any.
pub type FindDOMNode = dyn Fn(&'static Unknown) -> Option<&'static Unknown>;

pub struct ReactDOMInternals {
    pub d: Cell<&'static HostDispatcher>,
    pub p: Cell<i32>,
    #[cfg_attr(rust_js, rust_js::name = "findDOMNode", rust_js::nullable)]
    pub find_dom_node: Cell<Option<&'static FindDOMNode>>,
}

fn request_form_reset(_element: &HTMLFormElement) {
    panic!(concat!(
        "Invalid form element. requestFormReset must be passed a form that was ",
        "rendered by React."
    ));
}

#[cfg_attr(rust_js, rust_js::name = "DefaultDispatcher")]
const DEFAULT_DISPATCHER: &HostDispatcher = &HostDispatcher {
    f /* flushSyncWork */: &|| None,
    r /* requestFormReset */: &request_form_reset,
    prefetch_dns /* prefetchDNS */: &|_| noop(),
    preconnect /* preconnect */: &|_, _| noop(),
    preload /* preload */: &|_, _, _| noop(),
    m /* preloadModule */: &|_, _| noop(),
    preinit_script /* preinitScript */: &|_, _| noop(),
    preinit_style /* preinitStyle */: &|_, _, _| noop(),
    preinit_module_script /* preinitModuleScript */: &|_, _| noop(),
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "Internals")]
    pub static INTERNALS: ReactDOMInternals = ReactDOMInternals {
        d /* ReactDOMCurrentDispatcher */: Cell::new(DEFAULT_DISPATCHER),
        p /* currentUpdatePriority */: Cell::new(NO_EVENT_PRIORITY),
        find_dom_node: Cell::new(None),
    };
}

js::export_default!(INTERNALS);
