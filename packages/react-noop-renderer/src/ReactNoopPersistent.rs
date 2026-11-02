// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This is a renderer of React that doesn't have a render target output.
// It is useful to demonstrate the internals of the reconciler in isolation
// and for testing semantics of reconciliation separate from the host
// environment.

use js::{JsObject, Unknown};
use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;

#[cfg_attr(rust_js, rust_js::link_name = "react-reconciler#default")]
#[cfg_attr(rust_js, rust_js::name = "ReactFiberReconciler")]
fn react_fiber_reconciler(config: &JsObject) -> &'static ReactFiberReconciler {
    let _ = config;
    unreachable!()
}

use crate::create_react_noop::{
    BatchedUpdates, CreatePortal, CreateRoot, DeferredUpdates, DiscreteUpdates, FindInstance,
    Flush, FlushSync, GetOrCreateRootContainer, GetRoot, HostCounters, IdleUpdates, LegacyRoot,
    OfId, OfRoot, ReactNoop, Render, RenderToRootWithId, RootChildren, RootJsx, RunWithPriority,
    SuspenseyThingStatus, create_react_noop,
};

pub struct ReactNoopPersistentExports {
    #[cfg_attr(rust_js, rust_js::name = "_Scheduler")]
    pub scheduler: &'static JsObject,
    pub get_children: Box<OfRoot>,
    pub dangerously_get_children: Box<RootChildren>,
    pub get_pending_children: Box<OfRoot>,
    pub dangerously_get_pending_children: Box<RootChildren>,
    pub get_or_create_root_container: Box<GetOrCreateRootContainer>,
    pub create_root: Box<CreateRoot>,
    pub create_legacy_root: Box<dyn Fn() -> LegacyRoot>,
    #[cfg_attr(rust_js, rust_js::name = "getChildrenAsJSX")]
    pub get_children_as_jsx: Box<RootJsx>,
    #[cfg_attr(rust_js, rust_js::name = "getPendingChildrenAsJSX")]
    pub get_pending_children_as_jsx: Box<RootJsx>,
    pub get_suspensey_thing_status: Box<SuspenseyThingStatus>,
    pub resolve_suspensey_thing: Box<OfId>,
    pub reset_suspensey_thing_cache: Box<dyn Fn()>,
    pub create_portal: Box<CreatePortal>,
    pub render: Box<Render>,
    pub render_legacy_sync_root: Box<Render>,
    #[cfg_attr(rust_js, rust_js::name = "renderToRootWithID")]
    pub render_to_root_with_id: Box<RenderToRootWithId>,
    #[cfg_attr(rust_js, rust_js::name = "unmountRootWithID")]
    pub unmount_root_with_id: Box<OfId>,
    pub find_instance: Box<FindInstance>,
    pub flush_next_yield: Box<Flush>,
    pub start_tracking_host_counters: Box<dyn Fn()>,
    pub stop_tracking_host_counters: Box<dyn Fn() -> HostCounters>,
    pub expire: fn(f64),
    pub flush_expired: Box<Flush>,
    pub batched_updates: Box<BatchedUpdates>,
    pub deferred_updates: Box<DeferredUpdates>,
    pub discrete_updates: Box<DiscreteUpdates>,
    pub idle_updates: Box<IdleUpdates>,
    pub flush_discrete_updates: Option<&'static Unknown>,
    pub flush_sync: Box<FlushSync>,
    pub flush_passive_effects: Box<dyn Fn() -> bool>,
    pub act: Option<&'static Unknown>,
    pub dump_tree: Box<OfRoot>,
    pub get_root: Box<GetRoot>,
    // TODO: Remove this once callers migrate to alternatives.
    // This should only be used by React internals.
    #[cfg_attr(rust_js, rust_js::name = "unstable_runWithPriority")]
    pub unstable_run_with_priority: Box<RunWithPriority>,
    // $FlowFixMe[signature-verification-failure]
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::namespace)]
    pub static REACT_NOOP_PERSISTENT: ReactNoopPersistentExports = {
        let noop: ReactNoop = create_react_noop(
            &react_fiber_reconciler, // reconciler
            false, // useMutation
        );
        ReactNoopPersistentExports {
            scheduler: noop.scheduler,
            get_children: noop.get_children,
            dangerously_get_children: noop.dangerously_get_children,
            get_pending_children: noop.get_pending_children,
            dangerously_get_pending_children: noop.dangerously_get_pending_children,
            get_or_create_root_container: noop.get_or_create_root_container,
            create_root: noop.create_root,
            create_legacy_root: noop.create_legacy_root,
            get_children_as_jsx: noop.get_children_as_jsx,
            get_pending_children_as_jsx: noop.get_pending_children_as_jsx,
            get_suspensey_thing_status: noop.get_suspensey_thing_status,
            resolve_suspensey_thing: noop.resolve_suspensey_thing,
            reset_suspensey_thing_cache: noop.reset_suspensey_thing_cache,
            create_portal: noop.create_portal,
            render: noop.render,
            render_legacy_sync_root: noop.render_legacy_sync_root,
            render_to_root_with_id: noop.render_to_root_with_id,
            unmount_root_with_id: noop.unmount_root_with_id,
            find_instance: noop.find_instance,
            flush_next_yield: noop.flush_next_yield,
            start_tracking_host_counters: noop.start_tracking_host_counters,
            stop_tracking_host_counters: noop.stop_tracking_host_counters,
            expire: noop.expire,
            flush_expired: noop.flush_expired,
            batched_updates: noop.batched_updates,
            deferred_updates: noop.deferred_updates,
            discrete_updates: noop.discrete_updates,
            idle_updates: noop.idle_updates,
            flush_discrete_updates: noop.flush_discrete_updates,
            flush_sync: noop.flush_sync,
            flush_passive_effects: noop.flush_passive_effects,
            act: noop.act,
            dump_tree: noop.dump_tree,
            get_root: noop.get_root,
            unstable_run_with_priority: noop.unstable_run_with_priority,
        }
    };
}
