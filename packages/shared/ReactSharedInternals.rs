// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::marker::PhantomData;

/// The state React shares with its renderers, the `react` package's own.
pub struct SharedStateClient(PhantomData<js::JsObject>);

/// The reconciler's hooks dispatcher, `ReactInternalTypes`' `Dispatcher`,
/// which shared only holds and puts back.
pub struct Dispatcher(PhantomData<js::JsObject>);

/// The renderer's cache dispatcher, `ReactInternalTypes`' `AsyncDispatcher`.
pub struct AsyncDispatcher(PhantomData<js::JsObject>);

impl AsyncDispatcher {
    /// What `resource_type` made for the current cache, made once of it.
    #[cfg_attr(rust_js, rust_js::link_name = "getCacheForType")]
    pub fn get_cache_for_type<T>(&self, resource_type: fn() -> T) -> T {
        let _ = resource_type;
        unreachable!()
    }

    /// The signal aborted when the current cache is done with, `None`
    /// where it has none.
    #[cfg_attr(rust_js, rust_js::link_name = "cacheSignal")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn cache_signal(&self) -> Option<&'static webapi::AbortSignal> {
        unreachable!()
    }

    /// The component rendering now, a Fiber or a Server Component's
    /// `ReactComponentInfo`, `None` where none is (DEV only).
    #[cfg_attr(rust_js, rust_js::link_name = "getOwner")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn get_owner(&self) -> Option<&'static js::Unknown> {
        unreachable!()
    }
}

impl SharedStateClient {
    /// The current hooks dispatcher, `None` outside a render.
    #[cfg_attr(rust_js, rust_js::link_name = "get H")]
    pub fn h(&self) -> Option<&'static Dispatcher> {
        unreachable!()
    }

    /// The current cache dispatcher, `None` outside a render.
    #[cfg_attr(rust_js, rust_js::link_name = "get A")]
    pub fn a(&self) -> Option<&'static AsyncDispatcher> {
        unreachable!()
    }

    /// The hooks dispatcher from now on, `None` `null`.
    #[cfg_attr(rust_js, rust_js::link_name = "set H")]
    #[cfg_attr(rust_js, rust_js::nullable(dispatcher))]
    pub fn set_h(&self, dispatcher: Option<&Dispatcher>) {
        let _ = dispatcher;
        unreachable!()
    }

    /// The renderer's stack of what it's rendering, `None` where none is
    /// injected.
    #[cfg_attr(rust_js, rust_js::link_name = "get getCurrentStack")]
    pub fn get_current_stack(&self) -> Option<&'static dyn Fn() -> String> {
        unreachable!()
    }

    /// The renderer's stack of what it's rendering from now on, `None`
    /// `null`.
    #[cfg_attr(rust_js, rust_js::link_name = "set getCurrentStack")]
    #[cfg_attr(rust_js, rust_js::nullable(get_current_stack))]
    pub fn set_get_current_stack(&self, get_current_stack: Option<fn() -> String>) {
        let _ = get_current_stack;
        unreachable!()
    }

    /// Whether a legacy `act` is batching its updates (DEV only).
    #[cfg_attr(rust_js, rust_js::link_name = "get isBatchingLegacy")]
    pub fn is_batching_legacy(&self) -> bool {
        unreachable!()
    }

    /// Records that a legacy root scheduled an update inside `act` (DEV only).
    #[cfg_attr(rust_js, rust_js::link_name = "set didScheduleLegacyUpdate")]
    pub fn set_did_schedule_legacy_update(&self, did_schedule_legacy_update: bool) {
        let _ = did_schedule_legacy_update;
        unreachable!()
    }

    /// What an `act` scope's renderers do next, `None` outside an `act` (DEV
    /// only): the `react` package's `ActQueue`.
    #[cfg_attr(rust_js, rust_js::link_name = "get actQueue")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn act_queue(&self) -> Option<&'static js::Unknown> {
        unreachable!()
    }

    /// The errors thrown and not caught inside the current `act` (DEV only),
    /// which it rethrows into the test.
    #[cfg_attr(rust_js, rust_js::link_name = "get thrownErrors")]
    pub fn thrown_errors(&self) -> &'static RefCell<Vec<Option<&'static js::Unknown>>> {
        unreachable!()
    }

    /// How many async Transitions are still running, `0` outside an async
    /// `startTransition`.
    #[cfg_attr(rust_js, rust_js::link_name = "get asyncTransitions")]
    pub fn async_transitions(&self) -> f64 {
        unreachable!()
    }

    /// How many async Transitions are still running from now on.
    #[cfg_attr(rust_js, rust_js::link_name = "set asyncTransitions")]
    pub fn set_async_transitions(&self, count: f64) {
        let _ = count;
        unreachable!()
    }

    /// How many owner stacks were made since the limit was last reset.
    #[cfg_attr(rust_js, rust_js::link_name = "get recentlyCreatedOwnerStacks")]
    pub fn recently_created_owner_stacks(&self) -> f64 {
        unreachable!()
    }

    /// How many owner stacks were made since the limit was last reset, from
    /// now on.
    #[cfg_attr(rust_js, rust_js::link_name = "set recentlyCreatedOwnerStacks")]
    pub fn set_recently_created_owner_stacks(&self, count: f64) {
        let _ = count;
        unreachable!()
    }
}

unsafe extern "Rust" {
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInternals")]
    #[link_name = "react#*.__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    pub safe static REACT_SHARED_INTERNALS: &'static SharedStateClient;
}

js::export_default!(REACT_SHARED_INTERNALS);
