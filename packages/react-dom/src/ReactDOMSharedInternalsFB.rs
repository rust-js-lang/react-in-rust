// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::Unknown;
use shared::react_dom_shared_internals::HostDispatcher;

use react_reconciler::react_event_priorities::{EventPriority, NO_EVENT_PRIORITY};

use shared::noop::noop;

use crate::react_dom_shared_internals::FindDOMNode;

pub struct ReactDOMInternals {
    // The six functions ReactDOMClientFB sets as it loads, for www.
    #[cfg_attr(rust_js, rust_js::name = "Events", rust_js::nullable)]
    pub events: Cell<Option<&'static Unknown>>,
    pub d: Cell<&'static HostDispatcher>,
    pub p: Cell<EventPriority>,
    #[cfg_attr(rust_js, rust_js::name = "findDOMNode", rust_js::nullable)]
    pub find_dom_node: Cell<Option<&'static FindDOMNode>>,
}

#[cfg_attr(rust_js, rust_js::name = "DefaultDispatcher")]
const DEFAULT_DISPATCHER: &HostDispatcher = &HostDispatcher {
    f /* flushSyncWork */: &|| None,
    r /* requestFormReset */: &|_| noop(),
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
        events: Cell::new(None),
        d /* ReactDOMCurrentDispatcher */: Cell::new(DEFAULT_DISPATCHER),
        p /* currentUpdatePriority */: Cell::new(NO_EVENT_PRIORITY),
        find_dom_node: Cell::new(None),
    };
}

js::export_default!(INTERNALS);
