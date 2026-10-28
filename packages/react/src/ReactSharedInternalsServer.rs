// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use js::{Map, Set, WeakMap};
use shared::environment::__DEV__;
use shared::react_feature_flags::ENABLE_TAINT;
use shared::react_shared_internals::{AsyncDispatcher, Dispatcher};

use crate::react_taint_registry::{
    EntryValue, Reference, RequestCleanupQueue, TAINT_REGISTRY_BYTE_LENGTHS,
    TAINT_REGISTRY_OBJECTS, TAINT_REGISTRY_PENDING_REQUESTS, TAINT_REGISTRY_VALUES, TaintEntry,
};

pub struct SharedStateServer {
    // ReactCurrentDispatcher for Hooks
    #[cfg_attr(rust_js, rust_js::name = "H")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub h: Cell<Option<&'static Dispatcher>>,
    // ReactCurrentCache for Cache
    #[cfg_attr(rust_js, rust_js::name = "A")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub a: Cell<Option<&'static AsyncDispatcher>>,

    // enableTaint
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryObjects")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub taint_registry_objects: Cell<Option<&'static WeakMap<Reference, String>>>,
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryValues")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub taint_registry_values: Cell<Option<&'static Map<EntryValue, Rc<TaintEntry>>>>,
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryByteLengths")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub taint_registry_byte_lengths: Cell<Option<&'static Set<f64>>>,
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryPendingRequests")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub taint_registry_pending_requests:
        RefCell<Option<Rc<RefCell<HashSet<&'static RequestCleanupQueue>>>>>,

    // DEV-only

    // ReactDebugCurrentFrame
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub get_current_stack: Cell<Option<fn() -> String>>,

    // ReactOwnerStackReset
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub recently_created_owner_stacks: Cell<Option<f64>>,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInternals")]
    pub static REACT_SHARED_INTERNALS: SharedStateServer = SharedStateServer {
        h: Cell::new(None),
        a: Cell::new(None),
        taint_registry_objects: Cell::new(None),
        taint_registry_values: Cell::new(None),
        taint_registry_byte_lengths: Cell::new(None),
        taint_registry_pending_requests: RefCell::new(None),
        get_current_stack: Cell::new(None),
        recently_created_owner_stacks: Cell::new(None),
    };
}

js::on_load! {
    REACT_SHARED_INTERNALS.with(|react_shared_internals| {
        if ENABLE_TAINT.with(|&enabled| enabled) {
            react_shared_internals
                .taint_registry_objects
                .set(Some(TAINT_REGISTRY_OBJECTS.with(|&objects| objects)));
            react_shared_internals
                .taint_registry_values
                .set(Some(TAINT_REGISTRY_VALUES.with(|&values| values)));
            react_shared_internals
                .taint_registry_byte_lengths
                .set(Some(TAINT_REGISTRY_BYTE_LENGTHS.with(|&byte_lengths| byte_lengths)));
            react_shared_internals
                .taint_registry_pending_requests
                .replace(Some(TAINT_REGISTRY_PENDING_REQUESTS.with(Rc::clone)));
        }

        if __DEV__ {
            // Stack implementation injected by the current renderer.
            react_shared_internals.get_current_stack.set(None);
            react_shared_internals.recently_created_owner_stacks.set(Some(0.0));
        }
    });
}

js::export_default!(REACT_SHARED_INTERNALS);
