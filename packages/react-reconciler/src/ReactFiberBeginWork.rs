// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Dict, Error, Kind, SymbolFor, Unknown};
use react_isomorphic::react_context::ReactContext;
use react_isomorphic::react_lazy::LazyComponent;
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::react_feature_flags::{
    DISABLE_LEGACY_CONTEXT, DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS, DISABLE_LEGACY_MODE,
    ENABLE_CPU_SUSPENSE, ENABLE_LEGACY_HIDDEN, ENABLE_PROFILER_COMMIT_HOOKS, ENABLE_PROFILER_TIMER,
    ENABLE_SCHEDULING_PROFILER, ENABLE_SCOPE_API, ENABLE_TRANSITION_TRACING,
    ENABLE_VIEW_TRANSITION,
};
use shared::react_recoverable::REACT_RECOVERABLE_DIGEST;
use shared::react_symbols::{
    REACT_CONTEXT_TYPE, REACT_FORWARD_REF_TYPE, REACT_LAZY_TYPE, REACT_MEMO_TYPE,
};
use shared::shallow_equal::shallow_equal;
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_captured_value::{
    create_captured_value_at_fiber, create_captured_value_from_error,
};
use crate::react_child_fiber::{
    MOUNT_CHILD_FIBERS, RECONCILE_CHILD_FIBERS, clone_child_fibers, validate_suspense_list_children,
};
use crate::react_current_fiber::set_current_fiber;
use crate::react_fiber::{
    PortalStateNode, ProfilerStateNode, ViewTransitionState, create_fiber_from_fragment,
    create_fiber_from_offscreen, create_fiber_from_type_and_props, create_work_in_progress,
    is_function_class_component, is_simple_function_component,
};
use crate::react_fiber_activity_component::ActivityState;
use crate::react_fiber_cache_component::{
    CACHE_CONTEXT, Cache, CacheComponentState, SpawnedCachePool, push_cache_provider,
};
use crate::react_fiber_call_user_space::{
    CALL_COMPONENT_IN_DEV, CALL_RENDER_IN_DEV, ClassInstance,
};
use crate::react_fiber_class_component::{
    ClassComponentUpdater, construct_class_instance, mount_class_instance,
    resolve_class_component_props, resume_mount_class_instance, update_class_instance,
};
use crate::react_fiber_class_update_queue::{
    UpdateQueue, clone_update_queue, enqueue_captured_update, initialize_update_queue,
    process_update_queue, suspend_if_update_read_from_entangled_async_action,
};
use crate::react_fiber_concurrent_updates::enqueue_concurrent_render_for_lane;
use crate::react_fiber_dev_tools_hook::{
    mark_component_render_started, mark_component_render_stopped, set_is_strict_mode_for_devtools,
};
use crate::react_fiber_flags::{
    CALLBACK, CHILD_DELETION, CONTENT_RESET, DID_CAPTURE, DID_DEFER, FORCE_CLIENT_RENDER,
    FORCE_UPDATE_FOR_LEGACY_SUSPENSE, HYDRATING, LAYOUT_STATIC, NO_FLAGS, PASSIVE, PERFORMED_WORK,
    PLACEMENT, PLACEMENT_DEV, REF, REF_STATIC, SHOULD_CAPTURE, STATIC_MASK, UPDATE,
    VIEW_TRANSITION_NAMED_MOUNT, VIEW_TRANSITION_NAMED_STATIC,
};
use crate::react_fiber_hidden_context::{
    HiddenContext, is_current_tree_hidden, push_hidden_context, reuse_hidden_context_on_stack,
};
use crate::react_fiber_hooks::{
    bailout_hooks, check_did_render_id_hook, render_transition_aware_host_component_with_hooks,
    render_with_hooks, replay_suspended_component_with_hooks,
};
use crate::react_fiber_host_context::{
    get_root_host_container, push_host_container, push_host_context,
};
use crate::react_fiber_hot_reloading::resolve_type_for_hot_reloading;
use crate::react_fiber_hydration_context::{
    claim_hydratable_singleton, claim_next_hydratable_activity_instance,
    claim_next_hydratable_suspense_instance, enter_hydration_state, get_is_hydrating,
    queue_hydration_error, reenter_hydration_state_from_dehydrated_activity_instance,
    reenter_hydration_state_from_dehydrated_suspense_instance, reset_hydration_state,
    try_to_claim_next_hydratable_instance, try_to_claim_next_hydratable_text_instance,
    warn_if_hydrating,
};
use crate::react_fiber_lane::{
    DEFAULT_LANE, Lanes, NO_LANE, NO_LANES, OFFSCREEN_LANE, SOME_RETRY_LANE,
    get_bumped_lane_for_hydration, includes_only_retries, includes_some_lane, lane_to_lanes,
    merge_lanes, pick_arbitrary_lane, remove_lanes,
};
use crate::react_fiber_legacy_context::{
    get_masked_context, get_unmasked_context, has_context_changed as has_legacy_context_changed,
    invalidate_context_provider, is_context_provider as is_legacy_context_provider,
    push_context_provider as push_legacy_context_provider, push_top_level_context_object,
};
use crate::react_fiber_new_context::{
    check_if_context_changed, lazily_propagate_parent_context_changes, prepare_to_read_context,
    propagate_context_change, propagate_parent_context_changes_to_deferred_tree, push_provider,
    read_context, schedule_context_work_on_parent_path,
};
use crate::react_fiber_offscreen_component::{
    LegacyHiddenProps, OFFSCREEN_VISIBLE, OffscreenInstance, OffscreenMode, OffscreenProps,
    OffscreenQueue, OffscreenState,
};
use crate::react_fiber_reconciler::{should_error, should_suspend};
use crate::react_fiber_root::RootState;
use crate::react_fiber_suspense_component::{
    SuspenseListRenderState, SuspenseListTailMode, SuspenseState, find_first_suspended,
};
use crate::react_fiber_suspense_context::{
    FORCE_SUSPENSE_FALLBACK, SUSPENSE_STACK_CURSOR, SuspenseContext, has_suspense_list_context,
    pop_suspense_handler, push_dehydrated_activity_suspense_handler,
    push_fallback_tree_suspense_handler, push_offscreen_suspense_handler,
    push_primary_tree_suspense_handler, push_suspense_list_context,
    reuse_suspense_handler_on_stack, set_default_shallow_suspense_list_context,
    set_shallow_suspense_list_context,
};
use crate::react_fiber_thenable::resolve_lazy;
use crate::react_fiber_throw::{create_class_error_update, initialize_class_error_update};
use crate::react_fiber_tracing_marker_component::{
    TRANSITION_TRACING_MARKER, TracingMarkerInstance, get_marker_instances, push_marker_instance,
    push_root_marker_instance,
};
use crate::react_fiber_transition::{
    get_offscreen_deferred_cache, get_pending_transitions, get_suspended_cache,
    push_root_transition, push_transition, request_cache_from_pool,
};
use crate::react_fiber_tree_context::{
    get_forks_at_level, is_forked_child, push_materialized_tree_id, push_tree_id,
};
use crate::react_fiber_view_transition_component::ViewTransitionProps;
use crate::react_fiber_work_loop::{
    get_work_in_progress_root, mark_render_derived_cause, mark_skipped_update_lanes,
    peek_deferred_lane, render_did_suspend_delay_if_possible, schedule_update_on_fiber,
};
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_profiler_timer::stop_profiler_timer_if_running;
use crate::react_strict_mode_warnings::REACT_STRICT_MODE_WARNINGS;
use crate::react_type_of_mode::{
    CONCURRENT_MODE, NO_MODE, PROFILE_MODE, STRICT_LEGACY_MODE, TypeOfMode,
};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CACHE_COMPONENT, CLASS_COMPONENT, CONTEXT_CONSUMER, CONTEXT_PROVIDER,
    FORWARD_REF, FRAGMENT, FUNCTION_COMPONENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL,
    HOST_ROOT, HOST_SINGLETON, HOST_TEXT, INCOMPLETE_CLASS_COMPONENT,
    INCOMPLETE_FUNCTION_COMPONENT, LAZY_COMPONENT, LEGACY_HIDDEN_COMPONENT, MEMO_COMPONENT, MODE,
    OFFSCREEN_COMPONENT, PROFILER, SCOPE_COMPONENT, SIMPLE_MEMO_COMPONENT, SUSPENSE_COMPONENT,
    SUSPENSE_LIST_COMPONENT, THROW, TRACING_MARKER_COMPONENT, VIEW_TRANSITION_COMPONENT,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsHydration"]
    safe static SUPPORTS_HYDRATION: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsResources"]
    safe static SUPPORTS_RESOURCES: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
    #[link_name = "./src/ReactFiberConfig.js#isPrimaryRenderer"]
    safe static IS_PRIMARY_RENDERER: bool;
    // ReactFiberConfig's `ReactContext<TransitionStatus>`.
    #[link_name = "./src/ReactFiberConfig.js#HostTransitionContext"]
    safe static HOST_TRANSITION_CONTEXT: ReactContext<Option<&'static Unknown>>;
}

// ReactFiberConfig's host operations, on its `Type`, `Props`, `Container`,
// `Instance`, `HoistableRoot`'s resources and `SuspenseInstance`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#shouldSetTextContent"
)]
fn should_set_text_content(
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isSuspenseInstancePending"
)]
fn is_suspense_instance_pending(instance: &Unknown) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isSuspenseInstanceFallback"
)]
fn is_suspense_instance_fallback(instance: &Unknown) -> bool {
    unreachable!()
}

/// What the server said of a Suspense boundary it couldn't finish.
struct SuspenseInstanceFallbackErrorDetails {
    #[cfg_attr(rust_js, rust_js::nullable)]
    digest: Option<String>,
    message: Option<String>,
    stack: Option<String>,
    component_stack: Option<String>,
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getSuspenseInstanceFallbackErrorDetails",
    rust_js::gives_plain_data
)]
fn get_suspense_instance_fallback_error_details(
    instance: &Unknown,
) -> SuspenseInstanceFallbackErrorDetails {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#getResource")]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_resource(
    r#type: Option<&'static Unknown>,
    current_props: Option<&'static Unknown>,
    pending_props: Option<&'static Unknown>,
    current_resource: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createHoistableInstance"
)]
fn create_hoistable_instance(
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    root_container_instance: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) -> &'static Unknown {
    unreachable!()
}

/// shared/ReactTypes' `ActivityProps`.
struct ActivityProps {
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    mode: Option<OffscreenMode>,
    // shared/ReactTypes' `ReactNodeList`.
    children: Option<&'static Unknown>,
}

unsafe impl js::Defined for ActivityProps {}

thread_local! {
    // A special exception that's used to unwind the stack when an update flows
    // into a dehydrated boundary.
    #[cfg_attr(rust_js, rust_js::name = "SelectiveHydrationException")]
    pub static SELECTIVE_HYDRATION_EXCEPTION: &'static Error = js::error::new(concat!(
        "This is not a real error. It's an implementation detail of React's ",
        "selective hydration feature. If this leaks into userspace, it's a bug in ",
        "React. Please file an issue."
    ), None);

    static DID_RECEIVE_UPDATE: Cell<bool> = const { Cell::new(false) };

    static DID_WARN_ABOUT_BAD_CLASS: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_CONTEXT_TYPE_ON_FUNCTION_COMPONENT: &'static Dict<bool> =
        js::dict::new();
    static DID_WARN_ABOUT_CONTEXT_TYPES: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_GET_DERIVED_STATE_ON_FUNCTION_COMPONENT: &'static Dict<bool> =
        js::dict::new();
    pub static DID_WARN_ABOUT_REASSIGNING_PROPS: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_ABOUT_REVEAL_ORDER: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_TAIL_OPTIONS: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_CLASS_NAME_ON_VIEW_TRANSITION: &'static Dict<bool> = js::dict::new();
}

pub fn reconcile_children(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    next_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) {
    match current {
        None => {
            // If this is a fresh new component that hasn't been rendered yet, we
            // won't update its child set by applying minimal side-effects. Instead,
            // we will add them all to the child before it gets rendered. That means
            // we can optimize this reconciliation pass by not tracking side-effects.
            work_in_progress
                .child
                .replace(MOUNT_CHILD_FIBERS.with(|mount_child_fibers| {
                    mount_child_fibers(work_in_progress, js::null(), next_children, render_lanes)
                }));
        }
        Some(current) => {
            // If the current child is the same as the work in progress, it means that
            // we haven't yet started any work on these children. Therefore, we use
            // the clone algorithm to create a copy of all the current children.

            // If we had any progressed work already, that is invalid at this point so
            // let's throw it out.
            let current_child = current.child.borrow().clone();
            work_in_progress
                .child
                .replace(RECONCILE_CHILD_FIBERS.with(|reconcile_child_fibers| {
                    reconcile_child_fibers(
                        work_in_progress,
                        current_child,
                        next_children,
                        render_lanes,
                    )
                }));
        }
    }
}

fn force_unmount_current_and_reconcile(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    next_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) {
    // This function is fork of reconcileChildren. It's used in cases where we
    // want to reconcile without matching against the existing set. This has the
    // effect of all current children being unmounted; even if the type and key
    // are the same, the old child is unmounted and a new child is created.
    //
    // To do this, we're going to go through the reconcile algorithm twice. In
    // the first pass, we schedule a deletion for all the current children by
    // passing null.
    let current_child = current.child.borrow().clone();
    work_in_progress
        .child
        .replace(RECONCILE_CHILD_FIBERS.with(|reconcile_child_fibers| {
            reconcile_child_fibers(work_in_progress, current_child, js::null(), render_lanes)
        }));
    // In the second pass, we mount the new children. The trick here is that we
    // pass null in place of where we usually pass the current child set. This has
    // the effect of remounting all children regardless of whether their
    // identities match.
    work_in_progress
        .child
        .replace(RECONCILE_CHILD_FIBERS.with(|reconcile_child_fibers| {
            reconcile_child_fibers(work_in_progress, js::null(), next_children, render_lanes)
        }));
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_forward_ref(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: Option<&'static Unknown>,
    next_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // TODO: current can be non-null here even if the component
    // hasn't yet mounted. This happens after the first render suspends.
    // We'll need to figure out if this is fine or can cause issues.
    let mut render = js::get(component, "render");
    if __DEV__ {
        let resolved_render = resolve_type_for_hot_reloading(render);
        if !js::strict_equals(resolved_render, render) {
            render = resolved_render;
            if current.is_some() {
                DID_RECEIVE_UPDATE.set(true);
            }
        }
    }
    let r#ref = work_in_progress.r#ref.get();

    let props_without_ref;
    if js::has(next_props, "ref") {
        // `ref` is just a prop now, but `forwardRef` expects it to not appear in
        // the props object. This used to happen in the JSX runtime, but now we do
        // it here.
        props_without_ref = Some(js::unknown(js::dict::new::<()>()));
        for key in js::keys_in(next_props) {
            // Since `ref` should only appear in props via the JSX transform, we can
            // assume that this is a plain object. So we don't need a
            // hasOwnProperty check.
            if key != "ref" {
                js::set(props_without_ref, &key, js::get(next_props, &key));
            }
        }
    } else {
        props_without_ref = next_props;
    }

    // The rest is a fork of updateFunctionComponent
    prepare_to_read_context(work_in_progress, render_lanes);
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_started(work_in_progress);
    }

    let next_children = render_with_hooks(
        current,
        work_in_progress,
        unsafe { js::cast(render) },
        props_without_ref,
        r#ref.map(js::unknown_of),
        render_lanes,
    );
    let has_id = check_did_render_id_hook();

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_stopped();
    }

    if let Some(current) = current
        && !DID_RECEIVE_UPDATE.get()
    {
        bailout_hooks(current, work_in_progress, render_lanes);
        return bailout_on_already_finished_work(Some(current), work_in_progress, render_lanes);
    }

    if get_is_hydrating() && has_id {
        push_materialized_tree_id(work_in_progress);
    }

    // React DevTools reads this flag.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | PERFORMED_WORK);
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

/// What a `memo`'s `compare` is: whether its props are equal enough to bail out.
type Compare = dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>) -> bool;

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_memo_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: Option<&'static Unknown>,
    next_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let Some(current) = current else {
        let r#type = js::get(component, "type");
        if is_simple_function_component(r#type)
            && matches!(js::classify(js::get(component, "compare")), Kind::Null)
        {
            let mut resolved_type = r#type;
            if __DEV__ {
                resolved_type = resolve_type_for_hot_reloading(r#type);
            }
            // If this is a plain function component without default props,
            // and with only the default shallow comparison, we upgrade it
            // to a SimpleMemoComponent to allow fast path updates.
            work_in_progress.tag.set(SIMPLE_MEMO_COMPONENT);
            work_in_progress.r#type.set(resolved_type);
            if __DEV__ {
                validate_function_component_in_dev(work_in_progress, r#type);
            }
            return update_simple_memo_component(
                current,
                work_in_progress,
                resolved_type,
                next_props,
                render_lanes,
            );
        }
        let child = create_fiber_from_type_and_props(
            js::get(component, "type"),
            js::null(),
            next_props,
            Some(js::unknown_of(Rc::clone(work_in_progress))),
            work_in_progress.mode.get(),
            render_lanes,
        );
        child.r#ref.set(work_in_progress.r#ref.get());
        child.r#return.replace(Some(Rc::clone(work_in_progress)));
        work_in_progress.child.replace(Some(Rc::clone(&child)));
        return Some(child);
    };
    let current_child = current.child.borrow().clone().unwrap(); // This is always exactly one child
    let has_scheduled_update_or_context = check_scheduled_update_or_context(current, render_lanes);
    if !has_scheduled_update_or_context {
        // This will be the props with resolved defaultProps,
        // unlike current.memoizedProps which will be the unresolved ones.
        let prev_props = current_child.memoized_props.get();
        // Default to shallow comparison
        let compare = js::get(component, "compare");
        let compare: &Compare = match compare {
            Some(compare) => unsafe { js::cast(Some(compare)) },
            None => &shallow_equal,
        };
        if compare(prev_props, next_props)
            && js::strict_equals(current.r#ref.get(), work_in_progress.r#ref.get())
        {
            return bailout_on_already_finished_work(Some(current), work_in_progress, render_lanes);
        }
    }
    // React DevTools reads this flag.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | PERFORMED_WORK);
    let new_child = create_work_in_progress(&current_child, next_props);
    new_child.r#ref.set(work_in_progress.r#ref.get());
    new_child
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    work_in_progress.child.replace(Some(Rc::clone(&new_child)));
    Some(new_child)
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_simple_memo_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: Option<&'static Unknown>,
    mut next_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // TODO: current can be non-null here even if the component
    // hasn't yet mounted. This happens when the inner render suspends.
    // We'll need to figure out if this is fine or can cause issues.
    if let Some(current) = current {
        let prev_props = current.memoized_props.get();
        if shallow_equal(prev_props, next_props)
            && js::strict_equals(current.r#ref.get(), work_in_progress.r#ref.get())
            // Prevent bailout if the implementation changed due to hot reload.
            && (if __DEV__ {
                js::strict_equals(work_in_progress.r#type.get(), current.r#type.get())
            } else {
                true
            })
        {
            DID_RECEIVE_UPDATE.set(false);

            // The props are shallowly equal. Reuse the previous props object, like we
            // would during a normal fiber bailout.
            //
            // We don't have strong guarantees that the props object is referentially
            // equal during updates where we can't bail out anyway — like if the props
            // are shallowly equal, but there's a local state or context update in the
            // same batch.
            //
            // However, as a principle, we should aim to make the behavior consistent
            // across different ways of memoizing a component. For example, React.memo
            // has a different internal Fiber layout if you pass a normal function
            // component (SimpleMemoComponent) versus if you pass a different type
            // like forwardRef (MemoComponent). But this is an implementation detail.
            // Wrapping a component in forwardRef (or React.lazy, etc) shouldn't
            // affect whether the props object is reused during a bailout.
            next_props = prev_props;
            work_in_progress.pending_props.set(next_props);

            if !check_scheduled_update_or_context(current, render_lanes) {
                // The pending lanes were cleared at the beginning of beginWork. We're
                // about to bail out, but there might be other lanes that weren't
                // included in the current render. Usually, the priority level of the
                // remaining updates is accumulated during the evaluation of the
                // component (i.e. when processing the update queue). But since since
                // we're bailing out early *without* evaluating the component, we need
                // to account for it here, too. Reset to the value of the current fiber.
                // NOTE: This only applies to SimpleMemoComponent, not MemoComponent,
                // because a MemoComponent fiber does not have hooks or an update queue;
                // rather, it wraps around an inner component, which may or may not
                // contains hooks.
                // TODO: Move the reset at in beginWork out of the common path so that
                // this is no longer necessary.
                work_in_progress.lanes.set(current.lanes.get());
                return bailout_on_already_finished_work(
                    Some(current),
                    work_in_progress,
                    render_lanes,
                );
            } else if (current.flags.get() & FORCE_UPDATE_FOR_LEGACY_SUSPENSE) != NO_FLAGS {
                // This is a special case that only exists for legacy mode.
                // See https://github.com/react/react/pull/19216.
                DID_RECEIVE_UPDATE.set(true);
            }
        }
    }
    update_function_component(
        current,
        work_in_progress,
        component,
        next_props,
        render_lanes,
    )
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_offscreen_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
    next_props: &OffscreenProps,
) -> Option<Rc<Fiber>> {
    let next_children = next_props.children;

    let prev_state: Option<&OffscreenState> = match current {
        Some(current) => unsafe { js::cast(current.memoized_state.get()) },
        None => js::null(),
    };

    if current.is_none() && work_in_progress.state_node.get().is_none() {
        // We previously reset the work-in-progress.
        // We need to create a new Offscreen instance.
        let primary_child_instance = OffscreenInstance {
            visibility: Cell::new(OFFSCREEN_VISIBLE),
            pending_markers: Cell::new(js::null()),
            retry_cache: Cell::new(js::null()),
            transitions: Cell::new(js::null()),
        };
        work_in_progress
            .state_node
            .set(Some(js::unknown_of(primary_child_instance)));
    }

    if next_props.mode == Some(OffscreenMode::Hidden)
        || (ENABLE_LEGACY_HIDDEN
            && next_props.mode == Some(OffscreenMode::UnstableDeferWithoutHiding))
    {
        // Rendering a hidden tree.

        let did_suspend = (work_in_progress.flags.get() & DID_CAPTURE) != NO_FLAGS;
        if did_suspend {
            // Something suspended inside a hidden tree

            // Include the base lanes from the last render
            let next_base_lanes = match prev_state {
                Some(prev_state) => merge_lanes(prev_state.base_lanes, render_lanes),
                None => render_lanes,
            };

            let remaining_child_lanes;
            if let Some(current) = current {
                // Reset to the current children
                let mut current_child = current.child.borrow().clone();
                work_in_progress.child.replace(current_child.clone());

                // The current render suspended, but there may be other lanes with
                // pending work. We can't read `childLanes` from the current Offscreen
                // fiber because we reset it when it was deferred; however, we can read
                // the pending lanes from the child fibers.
                let mut current_child_lanes: Lanes = NO_LANES;
                while let Some(child) = current_child {
                    current_child_lanes = merge_lanes(
                        merge_lanes(current_child_lanes, child.lanes.get()),
                        child.child_lanes.get(),
                    );
                    current_child = child.sibling.borrow().clone();
                }
                let lanes_we_just_attempted = next_base_lanes;
                remaining_child_lanes = remove_lanes(current_child_lanes, lanes_we_just_attempted);
            } else {
                remaining_child_lanes = NO_LANES;
                work_in_progress.child.replace(None);
            }

            return defer_hidden_offscreen_component(
                current,
                work_in_progress,
                next_base_lanes,
                render_lanes,
                remaining_child_lanes,
            );
        }

        if !DISABLE_LEGACY_MODE && (work_in_progress.mode.get() & CONCURRENT_MODE) == NO_MODE {
            // In legacy sync mode, don't defer the subtree. Render it now.
            // TODO: Consider how Offscreen should work with transitions in the future
            let next_state = OffscreenState {
                base_lanes: NO_LANES,
                cache_pool: js::null(),
            };
            work_in_progress
                .memoized_state
                .set(Some(js::unknown_of(next_state)));
            // push the cache pool even though we're going to bail out
            // because otherwise there'd be a context mismatch
            if current.is_some() {
                push_transition(work_in_progress, js::null(), js::null());
            }
            reuse_hidden_context_on_stack(work_in_progress);
            push_offscreen_suspense_handler(work_in_progress);
        } else if !includes_some_lane(render_lanes, OFFSCREEN_LANE) {
            // We're hidden, and we're not rendering at Offscreen. We will bail out
            // and resume this tree later.

            // Schedule this fiber to re-render at Offscreen priority

            let remaining_child_lanes = lane_to_lanes(OFFSCREEN_LANE);
            work_in_progress.lanes.set(remaining_child_lanes);

            // Include the base lanes from the last render
            let next_base_lanes = match prev_state {
                Some(prev_state) => merge_lanes(prev_state.base_lanes, render_lanes),
                None => render_lanes,
            };

            return defer_hidden_offscreen_component(
                current,
                work_in_progress,
                next_base_lanes,
                render_lanes,
                remaining_child_lanes,
            );
        } else {
            // This is the second render. The surrounding visible content has already
            // committed. Now we resume rendering the hidden tree.

            // Rendering at offscreen, so we can clear the base lanes.
            let next_state = OffscreenState {
                base_lanes: NO_LANES,
                cache_pool: js::null(),
            };
            work_in_progress
                .memoized_state
                .set(Some(js::unknown_of(next_state)));
            if current.is_some() {
                // If the render that spawned this one accessed the cache pool, resume
                // using the same cache. Unless the parent changed, since that means
                // there was a refresh.
                let prev_cache_pool: Option<&SpawnedCachePool> = match prev_state {
                    Some(prev_state) => unsafe { js::cast(prev_state.cache_pool) },
                    None => js::null(),
                };
                // TODO: Consider if and how Offscreen pre-rendering should
                // be attributed to the transition that spawned it
                push_transition(work_in_progress, prev_cache_pool, js::null());
            }

            // Push the lanes that were skipped when we bailed out.
            if let Some(prev_state) = prev_state {
                push_hidden_context(work_in_progress, unsafe {
                    js::cast(Some(js::unknown(prev_state)))
                });
            } else {
                reuse_hidden_context_on_stack(work_in_progress);
            }
            push_offscreen_suspense_handler(work_in_progress);
        }
    } else {
        // Rendering a visible tree.
        if let Some(prev_state) = prev_state {
            // We're going from hidden -> visible.
            // If the render that spawned this one accessed the cache pool, resume
            // using the same cache. Unless the parent changed, since that means
            // there was a refresh.
            let prev_cache_pool: Option<&SpawnedCachePool> =
                unsafe { js::cast(prev_state.cache_pool) };

            let mut transitions = js::null();
            if ENABLE_TRANSITION_TRACING {
                // We have now gone from hidden to visible, so any transitions should
                // be added to the stack to get added to any Offscreen/suspense children
                let instance: Option<&OffscreenInstance> =
                    unsafe { js::cast(work_in_progress.state_node.get()) };
                if let Some(instance) = instance
                    && let Some(instance_transitions) = instance.transitions.get()
                {
                    transitions = Some(Rc::new(instance_transitions.values().collect()));
                }
            }

            push_transition(work_in_progress, prev_cache_pool, transitions);

            // Push the lanes that were skipped when we bailed out.
            push_hidden_context(work_in_progress, unsafe {
                js::cast::<Rc<HiddenContext>>(Some(js::unknown(prev_state)))
            });
            reuse_suspense_handler_on_stack(work_in_progress);

            // Since we're not hidden anymore, reset the state
            work_in_progress.memoized_state.set(js::null());
        } else {
            // We weren't previously hidden, and we still aren't, so there's nothing
            // special to do. Need to push to the stack regardless, though, to avoid
            // a push/pop misalignment.

            // If the render that spawned this one accessed the cache pool, resume
            // using the same cache. Unless the parent changed, since that means
            // there was a refresh.
            if current.is_some() {
                push_transition(work_in_progress, js::null(), js::null());
            }

            // We're about to bail out, but we need to push this to the stack anyway
            // to avoid a push/pop misalignment.
            reuse_hidden_context_on_stack(work_in_progress);
            reuse_suspense_handler_on_stack(work_in_progress);
        }
    }

    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn bailout_offscreen_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
) -> Option<Rc<Fiber>> {
    if current.is_none_or(|current| current.tag.get() != OFFSCREEN_COMPONENT)
        && work_in_progress.state_node.get().is_none()
    {
        let primary_child_instance = OffscreenInstance {
            visibility: Cell::new(OFFSCREEN_VISIBLE),
            pending_markers: Cell::new(js::null()),
            retry_cache: Cell::new(js::null()),
            transitions: Cell::new(js::null()),
        };
        work_in_progress
            .state_node
            .set(Some(js::unknown_of(primary_child_instance)));
    }

    work_in_progress.sibling.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn defer_hidden_offscreen_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    next_base_lanes: Lanes,
    render_lanes: Lanes,
    remaining_child_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let next_state = OffscreenState {
        base_lanes: next_base_lanes,
        // Save the cache pool so we can resume later.
        cache_pool: get_offscreen_deferred_cache().map(js::unknown_of),
    };
    work_in_progress
        .memoized_state
        .set(Some(js::unknown_of(next_state)));
    // push the cache pool even though we're going to bail out
    // because otherwise there'd be a context mismatch
    if current.is_some() {
        push_transition(work_in_progress, js::null(), js::null());
    }

    // We're about to bail out, but we need to push this to the stack anyway
    // to avoid a push/pop misalignment.
    reuse_hidden_context_on_stack(work_in_progress);

    push_offscreen_suspense_handler(work_in_progress);

    if let Some(current) = current {
        // Since this tree will resume rendering in a separate render, we need
        // to propagate parent contexts now so we don't lose track of which
        // ones changed.
        propagate_parent_context_changes_to_deferred_tree(current, work_in_progress, render_lanes);
    }

    // We override the remaining child lanes to be the subset that we computed
    // on the outside. We need to do this after propagating the context
    // because propagateParentContextChangesToDeferredTree may schedule
    // work which bubbles all the way up to the root and updates our child lanes.
    // We want to dismiss that since we're not going to work on it yet.
    work_in_progress.child_lanes.set(remaining_child_lanes);

    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_legacy_hidden_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let next_props: &LegacyHiddenProps = unsafe { js::cast(work_in_progress.pending_props.get()) };
    // Note: These happen to have identical begin phases, for now. We shouldn't hold
    // ourselves to this constraint, though. If the behavior diverges, we should
    // fork the function.
    // This just works today because it has the same Props.
    update_offscreen_component(current, work_in_progress, render_lanes, unsafe {
        js::cast(Some(js::unknown(next_props)))
    })
}

fn mount_activity_children(
    work_in_progress: &Rc<Fiber>,
    next_props: &ActivityProps,
    render_lanes: Lanes,
) -> Rc<Fiber> {
    if __DEV__ {
        let hidden_prop = js::get(js::unknown(next_props), "hidden");
        if !matches!(js::classify(hidden_prop), Kind::Undefined) {
            console::error((
                concat!(
                    "<Activity> doesn't accept a hidden prop. Use mode=\"hidden\" instead.\n",
                    "- <Activity %s>\n",
                    "+ <Activity %s>"
                ),
                if js::strict_equals(hidden_prop, true) {
                    "hidden"
                } else if js::strict_equals(hidden_prop, false) {
                    "hidden={false}"
                } else {
                    "hidden={...}"
                },
                if js::truthy(hidden_prop) {
                    "mode=\"hidden\""
                } else {
                    "mode=\"visible\""
                },
            ));
        }
    }
    let next_children = next_props.children;
    let next_mode = next_props.mode;
    let mode = work_in_progress.mode.get();
    let offscreen_child_props = OffscreenProps {
        mode: next_mode,
        children: next_children,
    };
    let primary_child_fragment =
        mount_work_in_progress_offscreen_fiber(offscreen_child_props, mode, render_lanes);
    primary_child_fragment
        .r#ref
        .set(work_in_progress.r#ref.get());
    work_in_progress
        .child
        .replace(Some(Rc::clone(&primary_child_fragment)));
    primary_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    primary_child_fragment
}

fn retry_activity_component_without_hydrating(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Rc<Fiber> {
    // Falling back to client rendering. Because this has performance
    // implications, it's considered a recoverable error, even though the user
    // likely won't observe anything wrong with the UI.

    // This will add the old fiber to the deletion list
    let current_child = current.child.borrow().clone();
    RECONCILE_CHILD_FIBERS.with(|reconcile_child_fibers| {
        reconcile_child_fibers(work_in_progress, current_child, js::null(), render_lanes)
    });

    // We're now not suspended nor dehydrated.
    let next_props: &ActivityProps = unsafe { js::cast(work_in_progress.pending_props.get()) };
    let primary_child_fragment =
        mount_activity_children(work_in_progress, next_props, render_lanes);
    // Needs a placement effect because the parent (the Activity boundary) already
    // mounted but this is a new fiber.
    primary_child_fragment
        .flags
        .set(primary_child_fragment.flags.get() | PLACEMENT);

    // If we're not going to hydrate we can't leave it dehydrated if something
    // suspends. In that case we want that to bubble to the nearest parent boundary
    // so we need to pop our own handler that we just pushed.
    pop_suspense_handler(work_in_progress);

    work_in_progress.memoized_state.set(js::null());

    primary_child_fragment
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn mount_dehydrated_activity_component(
    work_in_progress: &Fiber,
    _activity_instance: &'static Unknown,
    _render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // During the first pass, we'll bail out and not drill into the children.
    // Instead, we'll leave the content in place and try to hydrate it later.
    // We'll continue hydrating the rest at offscreen priority since we'll already
    // be showing the right content coming from the server, it is no rush.
    work_in_progress.lanes.set(lane_to_lanes(OFFSCREEN_LANE));
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_dehydrated_activity_component(
    current: &Rc<Fiber>,
    work_in_progress: &Rc<Fiber>,
    did_suspend: bool,
    next_props: &ActivityProps,
    activity_instance: &'static Unknown,
    activity_state: &ActivityState,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // We'll handle suspending since if something suspends we can just leave
    // it dehydrated. We push early and then pop if we enter non-dehydrated attempts.
    push_dehydrated_activity_suspense_handler(work_in_progress);
    if !did_suspend {
        // This is the first render pass. Attempt to hydrate.

        // We should never be hydrating at this point because it is the first pass,
        // but after we've already committed once.
        warn_if_hydrating();

        if includes_some_lane(render_lanes, OFFSCREEN_LANE) {
            // If we're rendering Offscreen and we're entering the activity then it's possible
            // that the only reason we rendered was because this boundary left work. Provide
            // it as a cause if another one doesn't already exist.
            mark_render_derived_cause(work_in_progress);
        }

        if
        // TODO: Factoring is a little weird, since we check this right below, too.
        !DID_RECEIVE_UPDATE.get() {
            // We need to check if any children have context before we decide to bail
            // out, so propagate the changes now.
            lazily_propagate_parent_context_changes(current, work_in_progress, render_lanes);
        }

        // We use lanes to indicate that a child might depend on context, so if
        // any context has changed, we need to treat is as if the input might have changed.
        let has_context_changed = includes_some_lane(render_lanes, current.child_lanes.get());
        if DID_RECEIVE_UPDATE.get() || has_context_changed {
            // This boundary has changed since the first render. This means that we are now unable to
            // hydrate it. We might still be able to hydrate it using a higher priority lane.
            if is_current_tree_hidden() {
                // This boundary is inside a hidden subtree, where all work is
                // deferred until the tree is revealed. Selective hydration works by
                // rendering the boundary at a higher priority before the update
                // applies, so it can't make progress here; delaying the commit to
                // wait for it would deadlock. Replacing hidden content isn't
                // visible, so give up and client render.
                return Some(retry_activity_component_without_hydrating(
                    current,
                    work_in_progress,
                    render_lanes,
                ));
            }
            let root = get_work_in_progress_root();
            if let Some(root) = root {
                let attempt_hydration_at_lane = get_bumped_lane_for_hydration(&root, render_lanes);
                if attempt_hydration_at_lane != NO_LANE
                    && attempt_hydration_at_lane != activity_state.retry_lane.get()
                {
                    // Intentionally mutating since this render will get interrupted. This
                    // is one of the very rare times where we mutate the current tree
                    // during the render phase.
                    activity_state.retry_lane.set(attempt_hydration_at_lane);
                    enqueue_concurrent_render_for_lane(current, attempt_hydration_at_lane);
                    schedule_update_on_fiber(&root, current, attempt_hydration_at_lane);

                    // Throw a special object that signals to the work loop that it should
                    // interrupt the current render.
                    //
                    // Because we're inside a React-only execution stack, we don't
                    // strictly need to throw here — we could instead modify some internal
                    // work loop state. But using an exception means we don't need to
                    // check for this case on every iteration of the work loop. So doing
                    // it this way moves the check out of the fast path.
                    SELECTIVE_HYDRATION_EXCEPTION
                        .with(|exception| js::throw(exception.as_unknown()));
                } else {
                    // We have already tried to ping at a higher priority than we're rendering with
                    // so if we got here, we must have failed to hydrate at those levels. We must
                    // now give up. Instead, we're going to delete the whole subtree and instead inject
                    // a new real Activity boundary to take its place. This might suspend for a while
                    // and if it does we might still have an opportunity to hydrate before this pass
                    // commits.
                }
            }

            // If we did not selectively hydrate, we'll continue rendering without
            // hydrating. Mark this tree as suspended to prevent it from committing
            // outside a transition.
            //
            // This path should only happen if the hydration lane already suspended.
            render_did_suspend_delay_if_possible();
            Some(retry_activity_component_without_hydrating(
                current,
                work_in_progress,
                render_lanes,
            ))
        } else {
            // This is the first attempt.

            reenter_hydration_state_from_dehydrated_activity_instance(
                work_in_progress,
                activity_instance,
                activity_state.tree_context.as_ref(),
            );

            let primary_child_fragment =
                mount_activity_children(work_in_progress, next_props, render_lanes);
            // Mark the children as hydrating. This is a fast path to know whether this
            // tree is part of a hydrating tree. This is used to determine if a child
            // node has fully mounted yet, and for scheduling event replaying.
            // Conceptually this is similar to Placement in that a new subtree is
            // inserted into the React tree here. It just happens to not need DOM
            // mutations because it already exists.
            // We should still treat it as a newly inserted Fiber to double invoke Strict Effects.
            primary_child_fragment
                .flags
                .set(primary_child_fragment.flags.get() | HYDRATING | PLACEMENT_DEV);
            Some(primary_child_fragment)
        }
    } else {
        // This is the second render pass. We already attempted to hydrated, but
        // something either suspended or errored.

        if (work_in_progress.flags.get() & FORCE_CLIENT_RENDER) != 0 {
            // Something errored during hydration. Try again without hydrating.
            // The error should've already been logged in throwException.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() & !FORCE_CLIENT_RENDER);
            Some(retry_activity_component_without_hydrating(
                current,
                work_in_progress,
                render_lanes,
            ))
        } else if work_in_progress.memoized_state.get().is_some() {
            // Something suspended and we should still be in dehydrated mode.
            // Leave the existing child in place.

            let child = current.child.borrow().clone();
            work_in_progress.child.replace(child);
            // The dehydrated completion pass expects this flag to be there
            // but the normal offscreen pass doesn't.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | DID_CAPTURE);
            None
        } else {
            // We called retryActivityComponentWithoutHydrating and tried client rendering
            // but now we suspended again. We should never arrive here because we should
            // not have pushed a suspense handler during that second pass and it should
            // instead have suspended above.
            panic!("Client rendering an Activity suspended it again. This is a bug in React.");
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_activity_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let next_props: &ActivityProps = unsafe { js::cast(work_in_progress.pending_props.get()) };

    // Check if the first pass suspended.
    let did_suspend = (work_in_progress.flags.get() & DID_CAPTURE) != NO_FLAGS;
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() & !DID_CAPTURE);

    match current {
        None => {
            // Initial mount

            // Special path for hydration
            // If we're currently hydrating, try to hydrate this boundary.
            // Hidden Activity boundaries are not emitted on the server.
            if get_is_hydrating() {
                if next_props.mode == Some(OffscreenMode::Hidden) {
                    // SSR doesn't render hidden Activity so it shouldn't hydrate,
                    // even at offscreen lane. Defer to a client rendered offscreen lane.
                    let primary_child_fragment =
                        mount_activity_children(work_in_progress, next_props, render_lanes);
                    work_in_progress.lanes.set(lane_to_lanes(OFFSCREEN_LANE));
                    // This tree hasn't been mounted yet so there are no baseLanes to carry over.
                    let next_state = OffscreenState {
                        base_lanes: NO_LANES,
                        cache_pool: js::null(),
                    };
                    primary_child_fragment
                        .memoized_state
                        .set(Some(js::unknown_of(next_state)));

                    return bailout_offscreen_component(js::null(), &primary_child_fragment);
                } else {
                    // We must push the suspense handler context *before* attempting to
                    // hydrate, to avoid a mismatch in case it errors.
                    push_dehydrated_activity_suspense_handler(work_in_progress);
                    let dehydrated = claim_next_hydratable_activity_instance(work_in_progress);
                    return mount_dehydrated_activity_component(
                        work_in_progress,
                        dehydrated,
                        render_lanes,
                    );
                }
            }

            Some(mount_activity_children(
                work_in_progress,
                next_props,
                render_lanes,
            ))
        }
        Some(current) => {
            // This is an update.

            // Special path for hydration
            let prev_state: Option<&ActivityState> =
                unsafe { js::cast(current.memoized_state.get()) };

            if let Some(prev_state) = prev_state {
                let dehydrated = prev_state.dehydrated;
                return update_dehydrated_activity_component(
                    current,
                    work_in_progress,
                    did_suspend,
                    next_props,
                    dehydrated,
                    prev_state,
                    render_lanes,
                );
            }

            let current_child = current.child.borrow().clone().unwrap();

            let next_children = next_props.children;
            let next_mode = next_props.mode;
            let offscreen_child_props = OffscreenProps {
                mode: next_mode,
                children: next_children,
            };

            if includes_some_lane(render_lanes, OFFSCREEN_LANE)
                && includes_some_lane(render_lanes, current.lanes.get())
            {
                // If we're rendering Offscreen and we're entering the activity then it's possible
                // that the only reason we rendered was because this boundary left work. Provide
                // it as a cause if another one doesn't already exist.
                mark_render_derived_cause(work_in_progress);
            }

            let primary_child_fragment =
                update_work_in_progress_offscreen_fiber(&current_child, offscreen_child_props);

            primary_child_fragment
                .r#ref
                .set(work_in_progress.r#ref.get());
            work_in_progress
                .child
                .replace(Some(Rc::clone(&primary_child_fragment)));
            primary_child_fragment
                .r#return
                .replace(Some(Rc::clone(work_in_progress)));
            Some(primary_child_fragment)
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_cache_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    prepare_to_read_context(work_in_progress, render_lanes);
    let parent_cache = CACHE_CONTEXT.with(read_context).unwrap();

    match current {
        None => {
            // Initial mount. Request a fresh cache from the pool.
            let fresh_cache = request_cache_from_pool(render_lanes);
            let initial_state = CacheComponentState {
                parent: parent_cache,
                cache: Rc::clone(&fresh_cache),
            };
            work_in_progress
                .memoized_state
                .set(Some(js::unknown_of(initial_state)));
            initialize_update_queue(work_in_progress);
            push_cache_provider(work_in_progress, fresh_cache);
        }
        Some(current) => {
            // Check for updates
            if includes_some_lane(current.lanes.get(), render_lanes) {
                clone_update_queue(current, work_in_progress);
                process_update_queue(work_in_progress, js::null(), js::null(), render_lanes);
                suspend_if_update_read_from_entangled_async_action();
            }
            let prev_state: &CacheComponentState =
                unsafe { js::cast(current.memoized_state.get()) };
            let next_state: &CacheComponentState =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };

            // Compare the new parent cache to the previous to see detect there was
            // a refresh.
            if !Rc::ptr_eq(&prev_state.parent, &parent_cache) {
                // Refresh in parent. Update the parent.
                let derived_state = js::unknown_of(CacheComponentState {
                    parent: Rc::clone(&parent_cache),
                    cache: Rc::clone(&parent_cache),
                });

                // Copied from getDerivedStateFromProps implementation. Once the update
                // queue is empty, persist the derived state onto the base state.
                work_in_progress.memoized_state.set(Some(derived_state));
                if work_in_progress.lanes.get() == NO_LANES {
                    let update_queue: &UpdateQueue =
                        unsafe { js::cast(work_in_progress.update_queue.get()) };
                    update_queue.base_state.set(Some(derived_state));
                    work_in_progress.memoized_state.set(Some(derived_state));
                }

                push_cache_provider(work_in_progress, parent_cache);
                // No need to propagate a context change because the refreshed parent
                // already did.
            } else {
                // The parent didn't refresh. Now check if this cache did.
                let next_cache = Rc::clone(&next_state.cache);
                push_cache_provider(work_in_progress, Rc::clone(&next_cache));
                if !Rc::ptr_eq(&next_cache, &prev_state.cache) {
                    // This cache refreshed. Propagate a context change.
                    CACHE_CONTEXT.with(|cache_context| {
                        propagate_context_change(work_in_progress, cache_context, render_lanes)
                    });
                }
            }
        }
    }

    // shared/ReactTypes' `CacheProps`.
    let next_props = work_in_progress.pending_props.get();

    let next_children = js::get(next_props, "children");
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

// This should only be called if the name changes
#[cfg_attr(rust_js, rust_js::nullable)]
fn update_tracing_marker_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if !ENABLE_TRANSITION_TRACING {
        return None;
    }

    // shared/ReactTypes' `TracingMarkerProps`.
    let next_props = work_in_progress.pending_props.get();

    // TODO: (luna) Only update the tracing marker if it's newly rendered or it's name changed.
    // A tracing marker is only associated with the transitions that rendered
    // or updated it, so we can create a new set of transitions each time
    match current {
        None => {
            let current_transitions = get_pending_transitions();
            if let Some(current_transitions) = current_transitions {
                let marker_instance = Rc::new(TracingMarkerInstance {
                    tag: Some(TRANSITION_TRACING_MARKER),
                    transitions: Cell::new(Some(js::set::new(Some(&*current_transitions)))),
                    pending_boundaries: Cell::new(js::null()),
                    name: RefCell::new(unsafe { js::cast(js::get(next_props, "name")) }),
                    aborts: RefCell::new(js::null()),
                });
                work_in_progress
                    .state_node
                    .set(Some(js::unknown_of(marker_instance)));

                // We call the marker complete callback when all child suspense boundaries resolve.
                // We do this in the commit phase on Offscreen. If the marker has no child suspense
                // boundaries, we need to schedule a passive effect to make sure we call the marker
                // complete callback.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | PASSIVE);
            }
        }
        Some(current) => {
            if __DEV__
                && !js::strict_equals(
                    js::get(current.memoized_props.get(), "name"),
                    js::get(next_props, "name"),
                )
            {
                console::error((concat!(
                    "Changing the name of a tracing marker after mount is not supported. ",
                    "To remount the tracing marker, pass it a new key."
                ),));
            }
        }
    }

    let instance: Option<Rc<TracingMarkerInstance>> =
        unsafe { js::cast(work_in_progress.state_node.get()) };
    if let Some(instance) = instance {
        push_marker_instance(work_in_progress, instance);
    }
    let next_children = js::get(next_props, "children");
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_fragment(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let next_children = work_in_progress.pending_props.get();
    mark_ref(current.map(Rc::as_ref), work_in_progress);
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_mode(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let next_children = js::get(work_in_progress.pending_props.get(), "children");
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_profiler(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | UPDATE);

        if ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled) {
            // Schedule a passive effect for this Profiler to call onPostCommit hooks.
            // This effect should be scheduled even if there is no onPostCommit callback for this Profiler,
            // because the effect is also where times bubble to parent Profilers.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | PASSIVE);
            // Reset effect durations for the next eventual effect phase.
            // These are reset during render to allow the DevTools commit hook a chance to read them,
            let state_node: &ProfilerStateNode =
                unsafe { js::cast(work_in_progress.state_node.get()) };
            state_node.effect_duration.set(-0.0);
            state_node.passive_effect_duration.set(-0.0);
        }
    }
    // shared/ReactTypes' `ProfilerProps`.
    let next_props = work_in_progress.pending_props.get();
    let next_children = js::get(next_props, "children");
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

fn mark_ref(current: Option<&Fiber>, work_in_progress: &Fiber) {
    // TODO: Check props.ref instead of fiber.ref when enableRefAsProp is on.
    let r#ref = work_in_progress.r#ref.get();
    match r#ref {
        None => {
            if current.is_some_and(|current| current.r#ref.get().is_some()) {
                // Schedule a Ref effect
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | REF | REF_STATIC);
            }
        }
        Some(r#ref) => {
            if js::type_of(Some(js::unknown_of(r#ref))) != "function"
                && js::type_of(Some(js::unknown_of(r#ref))) != "object"
            {
                panic!(
                    "Expected ref to be a function, an object returned by React.createRef(), or undefined/null."
                );
            }
            if current.is_none_or(|current| !js::strict_equals(current.r#ref.get(), Some(r#ref))) {
                // Schedule a Ref effect
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | REF | REF_STATIC);
            }
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn mount_incomplete_function_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: Option<&'static Unknown>,
    next_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    reset_suspended_current_on_mount_in_legacy_mode(current.map(Rc::as_ref), work_in_progress);

    work_in_progress.tag.set(FUNCTION_COMPONENT);

    update_function_component(
        js::null(),
        work_in_progress,
        component,
        next_props,
        render_lanes,
    )
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_function_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: Option<&'static Unknown>,
    next_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if __DEV__ {
        if js::truthy(js::get(component, "prototype"))
            && js::type_of(js::get(js::get(component, "prototype"), "render")) == "function"
        {
            let component_name = get_component_name_from_type(component)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Unknown".to_string());

            if !DID_WARN_ABOUT_BAD_CLASS.with(|did_warn| {
                js::dict::get(did_warn, &component_name).is_some_and(|&warned| warned)
            }) {
                console::error((
                    concat!(
                        "The <%s /> component appears to have a render method, but doesn't extend React.Component. ",
                        "This is likely to cause errors. Change %s to extend React.Component instead."
                    ),
                    &component_name,
                    &component_name,
                ));
                DID_WARN_ABOUT_BAD_CLASS
                    .with(|did_warn| js::dict::set(did_warn, &component_name, true));
            }
        }

        if (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
            REACT_STRICT_MODE_WARNINGS.with(|react_strict_mode_warnings| {
                (react_strict_mode_warnings
                    .record_legacy_context_warning
                    .get())(work_in_progress, js::null());
            });
        }

        if current.is_none() {
            // Some validations were previously done in mountIndeterminateComponent however and are now run
            // in updateFuntionComponent but only on mount
            validate_function_component_in_dev(work_in_progress, work_in_progress.r#type.get());

            if js::truthy(js::get(component, "contextTypes")) {
                let component_name = get_component_name_from_type(component)
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "Unknown".to_string());

                if !DID_WARN_ABOUT_CONTEXT_TYPES.with(|did_warn| {
                    js::dict::get(did_warn, &component_name).is_some_and(|&warned| warned)
                }) {
                    DID_WARN_ABOUT_CONTEXT_TYPES
                        .with(|did_warn| js::dict::set(did_warn, &component_name, true));
                    if DISABLE_LEGACY_CONTEXT {
                        console::error((
                            concat!(
                                "%s uses the legacy contextTypes API which was removed in React 19. ",
                                "Use React.createContext() with React.useContext() instead. ",
                                "(https://react.dev/link/legacy-context)"
                            ),
                            &component_name,
                        ));
                    } else {
                        console::error((
                            concat!(
                                "%s uses the legacy contextTypes API which will be removed soon. ",
                                "Use React.createContext() with React.useContext() instead. ",
                                "(https://react.dev/link/legacy-context)"
                            ),
                            &component_name,
                        ));
                    }
                }
            }
        }
    }

    let mut context = None;
    if !DISABLE_LEGACY_CONTEXT && !DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS {
        let unmasked_context =
            get_unmasked_context(work_in_progress, unsafe { js::cast(component) }, true);
        context = Some(get_masked_context(work_in_progress, unmasked_context));
    }

    prepare_to_read_context(work_in_progress, render_lanes);
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_started(work_in_progress);
    }
    let next_children = render_with_hooks(
        current,
        work_in_progress,
        unsafe { js::cast(component) },
        next_props,
        context.map(js::unknown),
        render_lanes,
    );
    let has_id = check_did_render_id_hook();
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_stopped();
    }

    if let Some(current) = current
        && !DID_RECEIVE_UPDATE.get()
    {
        bailout_hooks(current, work_in_progress, render_lanes);
        return bailout_on_already_finished_work(Some(current), work_in_progress, render_lanes);
    }

    if get_is_hydrating() && has_id {
        push_materialized_tree_id(work_in_progress);
    }

    // React DevTools reads this flag.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | PERFORMED_WORK);
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn replay_function_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    next_props: Option<&'static Unknown>,
    component: Option<&'static Unknown>,
    second_arg: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // This function is used to replay a component that previously suspended,
    // after its data resolves. It's a simplified version of
    // updateFunctionComponent that reuses the hooks from the previous attempt.

    prepare_to_read_context(work_in_progress, render_lanes);
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_started(work_in_progress);
    }
    let next_children = replay_suspended_component_with_hooks(
        current,
        work_in_progress,
        unsafe { js::cast(component) },
        next_props,
        second_arg,
    );
    let has_id = check_did_render_id_hook();
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_stopped();
    }

    if let Some(current) = current
        && !DID_RECEIVE_UPDATE.get()
    {
        bailout_hooks(current, work_in_progress, render_lanes);
        return bailout_on_already_finished_work(Some(current), work_in_progress, render_lanes);
    }

    if get_is_hydrating() && has_id {
        push_materialized_tree_id(work_in_progress);
    }

    // React DevTools reads this flag.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | PERFORMED_WORK);
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

struct ExitStrictModeForDevtools;

impl Drop for ExitStrictModeForDevtools {
    fn drop(&mut self) {
        set_is_strict_mode_for_devtools(false);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_class_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: &'static Unknown,
    next_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if __DEV__ {
        // This is used by DevTools to force a boundary to error.
        match should_error(work_in_progress) {
            Some(false) => {
                // We previously simulated an error on this boundary
                // so the instance must have been constructed in a previous
                // commit.
                let instance = work_in_progress.state_node.get();
                let ctor = work_in_progress.r#type.get();
                // TODO This way of resetting the error boundary state is a hack.
                // Is there a better way to do this?
                let temp_instance = js::new(
                    unsafe { js::cast::<&Unknown>(ctor) },
                    (
                        work_in_progress.memoized_props.get(),
                        js::get(instance, "context"),
                    ),
                );
                let state = js::get(temp_instance, "state");
                (unsafe { js::cast::<&ClassComponentUpdater>(js::get(instance, "updater")) }
                    .enqueue_set_state)(
                    unsafe { js::cast(instance) }, state, js::null()
                );
            }
            Some(true) => {
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | DID_CAPTURE);
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | SHOULD_CAPTURE);
                // eslint-disable-next-line react-internal/prod-error-codes
                let error = js::error::new("Simulated error coming from DevTools", None);
                let lane = pick_arbitrary_lane(render_lanes);
                work_in_progress
                    .lanes
                    .set(merge_lanes(work_in_progress.lanes.get(), lane));
                // Schedule the error boundary to re-render using updated state
                let root = get_work_in_progress_root();
                let Some(root) = root else {
                    panic!(
                        "Expected a work-in-progress root. This is a bug in React. Please file an issue."
                    );
                };
                let update = create_class_error_update(lane);
                initialize_class_error_update(
                    &update,
                    &root,
                    work_in_progress,
                    &create_captured_value_at_fiber(error.as_unknown(), work_in_progress),
                );
                enqueue_captured_update(work_in_progress, &update);
            }
            None => {}
        }
    }

    // Push context providers early to prevent context stack mismatches.
    // During mounting we don't know the child context yet as the instance doesn't exist.
    // We will invalidate the child context in finishClassComponent() right after rendering.
    let has_context;
    if is_legacy_context_provider(Some(component)) {
        has_context = true;
        push_legacy_context_provider(work_in_progress);
    } else {
        has_context = false;
    }
    prepare_to_read_context(work_in_progress, render_lanes);

    let instance = work_in_progress.state_node.get();
    let should_update;
    if instance.is_none() {
        reset_suspended_current_on_mount_in_legacy_mode(current.map(Rc::as_ref), work_in_progress);

        // In the initial pass we might need to construct the instance.
        construct_class_instance(work_in_progress, component, next_props);
        mount_class_instance(work_in_progress, component, next_props, render_lanes);
        should_update = true;
    } else {
        match current {
            None => {
                // In a resume, we'll already have an instance we can reuse.
                should_update = resume_mount_class_instance(
                    work_in_progress,
                    component,
                    next_props,
                    render_lanes,
                );
            }
            Some(current) => {
                should_update = update_class_instance(
                    current,
                    work_in_progress,
                    component,
                    next_props,
                    render_lanes,
                );
            }
        }
    }
    let next_unit_of_work = finish_class_component(
        current,
        work_in_progress,
        component,
        should_update,
        has_context,
        render_lanes,
    );
    if __DEV__ {
        let inst = work_in_progress.state_node.get();
        if should_update && !js::strict_equals(js::get(inst, "props"), next_props) {
            if !DID_WARN_ABOUT_REASSIGNING_PROPS.get() {
                console::error((
                    concat!(
                        "It looks like %s is reassigning its own `this.props` while rendering. ",
                        "This is not supported and can lead to confusing bugs."
                    ),
                    get_component_name_from_fiber(work_in_progress)
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "a component".to_string()),
                ));
            }
            DID_WARN_ABOUT_REASSIGNING_PROPS.set(true);
        }
    }
    next_unit_of_work
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn finish_class_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: &'static Unknown,
    should_update: bool,
    has_context: bool,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // Refs should update even if shouldComponentUpdate returns false
    mark_ref(current.map(Rc::as_ref), work_in_progress);

    let did_capture_error = (work_in_progress.flags.get() & DID_CAPTURE) != NO_FLAGS;

    if !should_update && !did_capture_error {
        // Context providers should defer to sCU for rendering
        if has_context {
            invalidate_context_provider(work_in_progress, component, false);
        }

        return bailout_on_already_finished_work(current, work_in_progress, render_lanes);
    }

    let instance: &ClassInstance = unsafe { js::cast(work_in_progress.state_node.get()) };

    // Rerender
    if __DEV__ {
        set_current_fiber(Some(Rc::clone(work_in_progress)));
    }
    let next_children;
    if did_capture_error
        && js::type_of(js::get(component, "getDerivedStateFromError")) != "function"
    {
        // If we captured an error, but getDerivedStateFromError is not defined,
        // unmount all the children. componentDidCatch will schedule an update to
        // re-render a fallback. This is temporary until we migrate everyone to
        // the new API.
        // TODO: Warn in a future release.
        next_children = js::null();

        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            stop_profiler_timer_if_running(work_in_progress);
        }
    } else {
        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_component_render_started(work_in_progress);
        }
        if __DEV__ {
            next_children =
                CALL_RENDER_IN_DEV.with(|call_render_in_dev| call_render_in_dev.unwrap()(instance));
            if (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
                set_is_strict_mode_for_devtools(true);
                let _exit = ExitStrictModeForDevtools;
                CALL_RENDER_IN_DEV.with(|call_render_in_dev| call_render_in_dev.unwrap()(instance));
            }
        } else {
            next_children = instance.render();
        }
        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_component_render_stopped();
        }
    }

    // React DevTools reads this flag.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | PERFORMED_WORK);
    if let Some(current) = current
        && did_capture_error
    {
        // If we're recovering from an error, reconcile without reusing any of
        // the existing children. Conceptually, the normal children and the children
        // that are shown on error are two different sets, so we shouldn't reuse
        // normal children even if their identities match.
        force_unmount_current_and_reconcile(current, work_in_progress, next_children, render_lanes);
    } else {
        reconcile_children(current, work_in_progress, next_children, render_lanes);
    }

    // Memoize state using the values we just used to render.
    // TODO: Restructure so we never read values from the instance.
    work_in_progress
        .memoized_state
        .set(js::get(instance, "state"));

    // The context might have changed so we need to recalculate it.
    if has_context {
        invalidate_context_provider(work_in_progress, component, true);
    }

    work_in_progress.child.borrow().clone()
}

fn push_host_root_context(work_in_progress: &Rc<Fiber>) {
    let root: &FiberRoot = unsafe { js::cast(work_in_progress.state_node.get()) };
    if let Some(pending_context) = root.pending_context.get() {
        push_top_level_context_object(
            work_in_progress,
            unsafe { js::cast(Some(js::unknown(pending_context))) },
            !js::strict_equals(root.pending_context.get(), root.context.get()),
        );
    } else if let Some(context) = root.context.get() {
        // Should always be set
        push_top_level_context_object(
            work_in_progress,
            unsafe { js::cast(Some(js::unknown(context))) },
            false,
        );
    }
    push_host_container(work_in_progress, root.container_info);
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_host_root(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    push_host_root_context(work_in_progress);

    let Some(current) = current else {
        panic!("Should have a current fiber. This is a bug in React.");
    };

    let next_props = work_in_progress.pending_props.get();
    let prev_state: &RootState = unsafe { js::cast(work_in_progress.memoized_state.get()) };
    let prev_children = prev_state.element;
    clone_update_queue(current, work_in_progress);
    process_update_queue(work_in_progress, next_props, js::null(), render_lanes);

    let next_state: &RootState = unsafe { js::cast(work_in_progress.memoized_state.get()) };
    let root: &FiberRoot = unsafe { js::cast(work_in_progress.state_node.get()) };
    push_root_transition(work_in_progress, root, render_lanes);

    if ENABLE_TRANSITION_TRACING {
        push_root_marker_instance(work_in_progress);
    }

    let next_cache: &Rc<Cache> = &next_state.cache;
    push_cache_provider(work_in_progress, Rc::clone(next_cache));
    if !Rc::ptr_eq(next_cache, &prev_state.cache) {
        // The root cache refreshed.
        CACHE_CONTEXT.with(|cache_context| {
            propagate_context_change(work_in_progress, cache_context, render_lanes)
        });
    }

    // This would ideally go inside processUpdateQueue, but because it suspends,
    // it needs to happen after the `pushCacheProvider` call above to avoid a
    // context stack mismatch. A bit unfortunate.
    suspend_if_update_read_from_entangled_async_action();

    // Caution: React DevTools currently depends on this property
    // being called "element".
    let next_children = next_state.element;
    if SUPPORTS_HYDRATION && prev_state.is_dehydrated {
        // This is a hydration root whose shell has not yet hydrated. We should
        // attempt to hydrate.

        // Flip isDehydrated to false to indicate that when this render
        // finishes, the root will no longer be dehydrated.
        let override_state = js::unknown_of(RootState {
            element: next_children,
            is_dehydrated: false,
            cache: Rc::clone(&next_state.cache),
        });
        let update_queue: &UpdateQueue = unsafe { js::cast(work_in_progress.update_queue.get()) };
        // `baseState` can always be the last state because the root doesn't
        // have reducer functions so it doesn't need rebasing.
        update_queue.base_state.set(Some(override_state));
        work_in_progress.memoized_state.set(Some(override_state));

        if (work_in_progress.flags.get() & FORCE_CLIENT_RENDER) != 0 {
            // Something errored during a previous attempt to hydrate the shell, so we
            // forced a client render. We should have a recoverable error already scheduled.
            return mount_host_root_without_hydrating(
                current,
                work_in_progress,
                next_children,
                render_lanes,
            );
        } else if !js::strict_equals(next_children, prev_children) {
            let recoverable_error = create_captured_value_at_fiber(
                js::error::new(
                    concat!(
                        "This root received an early update, before anything was able ",
                        "hydrate. Switched the entire root to client rendering."
                    ),
                    None,
                )
                .as_unknown(),
                work_in_progress,
            );
            queue_hydration_error(recoverable_error);
            return mount_host_root_without_hydrating(
                current,
                work_in_progress,
                next_children,
                render_lanes,
            );
        } else {
            // The outermost shell has not hydrated yet. Start hydrating.
            enter_hydration_state(work_in_progress);

            let child = MOUNT_CHILD_FIBERS.with(|mount_child_fibers| {
                mount_child_fibers(work_in_progress, js::null(), next_children, render_lanes)
            });
            work_in_progress.child.replace(child.clone());

            let mut node = child;
            while let Some(current_node) = node {
                // Mark each child as hydrating. This is a fast path to know whether this
                // tree is part of a hydrating tree. This is used to determine if a child
                // node has fully mounted yet, and for scheduling event replaying.
                // Conceptually this is similar to Placement in that a new subtree is
                // inserted into the React tree here. It just happens to not need DOM
                // mutations because it already exists.
                // We should still treat it as a newly inserted Fiber to double invoke Strict Effects.
                current_node
                    .flags
                    .set((current_node.flags.get() & !PLACEMENT) | HYDRATING | PLACEMENT_DEV);
                node = current_node.sibling.borrow().clone();
            }
        }
    } else {
        // Root is not dehydrated. Either this is a client-only root, or it
        // already hydrated.
        reset_hydration_state();
        if js::strict_equals(next_children, prev_children) {
            return bailout_on_already_finished_work(Some(current), work_in_progress, render_lanes);
        }
        reconcile_children(Some(current), work_in_progress, next_children, render_lanes);
    }
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn mount_host_root_without_hydrating(
    current: &Rc<Fiber>,
    work_in_progress: &Rc<Fiber>,
    next_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // Revert to client rendering.
    reset_hydration_state();

    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | FORCE_CLIENT_RENDER);

    reconcile_children(Some(current), work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_host_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if current.is_none() {
        try_to_claim_next_hydratable_instance(work_in_progress);
    }

    push_host_context(work_in_progress);

    let r#type = work_in_progress.r#type.get();
    let next_props = work_in_progress.pending_props.get();
    let prev_props = match current {
        Some(current) => current.memoized_props.get(),
        None => js::null(),
    };

    let mut next_children = js::get(next_props, "children");
    let is_direct_text_child = should_set_text_content(r#type, next_props);

    if is_direct_text_child {
        // We special case a direct text child of a host node. This is a common
        // case. We won't handle it as a reified child. We will instead handle
        // this in the host environment that also has access to this prop. That
        // avoids allocating another HostText fiber and traversing it.
        next_children = js::null();
    } else if prev_props.is_some() && should_set_text_content(r#type, prev_props) {
        // If we're switching from a direct text child to a normal child, or to
        // empty, we need to schedule the text content to be reset.
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | CONTENT_RESET);
    }

    let memoized_state = work_in_progress.memoized_state.get();
    if memoized_state.is_some() {
        // This fiber has been upgraded to a stateful component. The only way
        // happens currently is for form actions. We use hooks to track the
        // pending and error state of the form.
        //
        // Once a fiber is upgraded to be stateful, it remains stateful for the
        // rest of its lifetime.
        let new_state = render_transition_aware_host_component_with_hooks(
            current,
            work_in_progress,
            render_lanes,
        );

        // If the transition state changed, propagate the change to all the
        // descendents. We use Context as an implementation detail for this.
        //
        // We need to update it here because
        // pushHostContext gets called before we process the state hook, to avoid
        // a state mismatch in the event that something suspends.
        //
        // NOTE: This assumes that there cannot be nested transition providers,
        // because the only renderer that implements this feature is React DOM,
        // and forms cannot be nested. If we did support nested providers, then
        // we would need to push a context value even for host fibers that
        // haven't been upgraded yet.
        if IS_PRIMARY_RENDERER {
            HOST_TRANSITION_CONTEXT.current_value.replace(new_state);
        } else {
            HOST_TRANSITION_CONTEXT.current_value2.replace(new_state);
        }
    }

    mark_ref(current.map(Rc::as_ref), work_in_progress);
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_host_hoistable(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    _render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    mark_ref(current.map(Rc::as_ref), work_in_progress);

    match current {
        None => {
            let resource = get_resource(
                work_in_progress.r#type.get(),
                js::null(),
                work_in_progress.pending_props.get(),
                js::null(),
            );
            if let Some(resource) = resource {
                work_in_progress.memoized_state.set(Some(resource));
            } else if !get_is_hydrating() {
                // This is not a Resource Hoistable and we aren't hydrating so we construct the instance.
                work_in_progress
                    .state_node
                    .set(Some(create_hoistable_instance(
                        work_in_progress.r#type.get(),
                        work_in_progress.pending_props.get(),
                        get_root_host_container(),
                        work_in_progress,
                    )));
            }
        }
        Some(current) => {
            // Get Resource may or may not return a resource. either way we stash the result
            // on memoized state.
            work_in_progress.memoized_state.set(get_resource(
                work_in_progress.r#type.get(),
                current.memoized_props.get(),
                work_in_progress.pending_props.get(),
                current.memoized_state.get(),
            ));
        }
    }

    // Resources never have reconciler managed children. It is possible for
    // the host implementation of getResource to consider children in the
    // resource construction but they will otherwise be discarded. In practice
    // this precludes all but the simplest children and Host specific warnings
    // should be implemented to warn when children are passsed when otherwise not
    // expected
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_host_singleton(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    push_host_context(work_in_progress);

    if current.is_none() {
        claim_hydratable_singleton(work_in_progress);
    }

    let next_children = js::get(work_in_progress.pending_props.get(), "children");
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    mark_ref(current.map(Rc::as_ref), work_in_progress);
    if current.is_none() {
        // We mark Singletons with a static flag to more efficiently manage their
        // ownership of the singleton host instance when in offscreen trees including Suspense
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | LAYOUT_STATIC);
    }
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_host_text(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
) -> Option<Rc<Fiber>> {
    if current.is_none() {
        try_to_claim_next_hydratable_text_instance(work_in_progress);
    }
    // Nothing to do here. This is terminal. We'll do the completion step
    // immediately after.
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn mount_lazy_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    element_type: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    reset_suspended_current_on_mount_in_legacy_mode(current.map(Rc::as_ref), work_in_progress);

    let props = work_in_progress.pending_props.get();
    let lazy_component: &LazyComponent = unsafe { js::cast(element_type) };
    let mut component = resolve_lazy(lazy_component);
    if __DEV__ {
        component = resolve_type_for_hot_reloading(component);
    }
    // Store the unwrapped component in the type.
    work_in_progress.r#type.set(component);

    if js::type_of(component) == "function" {
        if is_function_class_component(unsafe { js::cast(component) }) {
            let resolved_props =
                resolve_class_component_props(component, unsafe { js::cast(props) });
            work_in_progress.tag.set(CLASS_COMPONENT);
            return update_class_component(
                js::null(),
                work_in_progress,
                unsafe { js::cast(component) },
                Some(resolved_props),
                render_lanes,
            );
        } else {
            work_in_progress.tag.set(FUNCTION_COMPONENT);
            if __DEV__ {
                validate_function_component_in_dev(work_in_progress, component);
            }
            return update_function_component(
                js::null(),
                work_in_progress,
                component,
                props,
                render_lanes,
            );
        }
    } else if component.is_some() {
        let r#typeof = SymbolFor::from_value(js::get(component, "$$typeof"));
        if r#typeof == Some(REACT_FORWARD_REF_TYPE) {
            work_in_progress.tag.set(FORWARD_REF);
            return update_forward_ref(
                js::null(),
                work_in_progress,
                component,
                props,
                render_lanes,
            );
        } else if r#typeof == Some(REACT_MEMO_TYPE) {
            work_in_progress.tag.set(MEMO_COMPONENT);
            return update_memo_component(
                js::null(),
                work_in_progress,
                component,
                props,
                render_lanes,
            );
        } else if r#typeof == Some(REACT_CONTEXT_TYPE) {
            work_in_progress.tag.set(CONTEXT_PROVIDER);
            work_in_progress.r#type.set(component);
            return update_context_provider(js::null(), work_in_progress, render_lanes);
        }
    }

    let mut hint = "";
    if __DEV__
        && component.is_some()
        && js::type_of(component) == "object"
        && SymbolFor::from_value(js::get(component, "$$typeof")) == Some(REACT_LAZY_TYPE)
    {
        hint = " Did you wrap a component in React.lazy() more than once?";
    }

    let logged_component = get_component_name_from_type(component)
        .filter(|name| !name.is_empty())
        .map_or_else(|| js::shown(component), js::shown);

    // This message intentionally doesn't mention ForwardRef or MemoComponent
    // because the fact that it's a separate type of work is an
    // implementation detail.
    panic!(
        "Element type is invalid. Received a promise that resolves to: {logged_component}. Lazy element type must resolve to a class or function.{hint}"
    );
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn mount_incomplete_class_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: &'static Unknown,
    next_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    reset_suspended_current_on_mount_in_legacy_mode(current.map(Rc::as_ref), work_in_progress);

    // Promote the fiber to a class and try rendering again.
    work_in_progress.tag.set(CLASS_COMPONENT);

    // The rest of this function is a fork of `updateClassComponent`

    // Push context providers early to prevent context stack mismatches.
    // During mounting we don't know the child context yet as the instance doesn't exist.
    // We will invalidate the child context in finishClassComponent() right after rendering.
    let has_context;
    if is_legacy_context_provider(Some(component)) {
        has_context = true;
        push_legacy_context_provider(work_in_progress);
    } else {
        has_context = false;
    }
    prepare_to_read_context(work_in_progress, render_lanes);

    construct_class_instance(work_in_progress, component, next_props);
    mount_class_instance(work_in_progress, component, next_props, render_lanes);

    finish_class_component(
        js::null(),
        work_in_progress,
        component,
        true,
        has_context,
        render_lanes,
    )
}

fn validate_function_component_in_dev(
    _work_in_progress: &Fiber,
    component: Option<&'static Unknown>,
) {
    if __DEV__ {
        if js::truthy(component) && js::truthy(js::get(component, "childContextTypes")) {
            console::error((
                concat!(
                    "childContextTypes cannot be defined on a function component.\n",
                    "  %s.childContextTypes = ..."
                ),
                js::get(component, "displayName")
                    .filter(|name| js::truthy(Some(*name)))
                    .or_else(|| js::get(component, "name"))
                    .filter(|name| js::truthy(Some(*name)))
                    .map_or_else(|| "Component".to_string(), |name| js::string(Some(name))),
            ));
        }

        if js::type_of(js::get(component, "getDerivedStateFromProps")) == "function" {
            let component_name = get_component_name_from_type(component)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Unknown".to_string());

            if !DID_WARN_ABOUT_GET_DERIVED_STATE_ON_FUNCTION_COMPONENT.with(|did_warn| {
                js::dict::get(did_warn, &component_name).is_some_and(|&warned| warned)
            }) {
                console::error((
                    "%s: Function components do not support getDerivedStateFromProps.",
                    &component_name,
                ));
                DID_WARN_ABOUT_GET_DERIVED_STATE_ON_FUNCTION_COMPONENT
                    .with(|did_warn| js::dict::set(did_warn, &component_name, true));
            }
        }

        if js::type_of(js::get(component, "contextType")) == "object"
            && js::get(component, "contextType").is_some()
        {
            let component_name = get_component_name_from_type(component)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Unknown".to_string());

            if !DID_WARN_ABOUT_CONTEXT_TYPE_ON_FUNCTION_COMPONENT.with(|did_warn| {
                js::dict::get(did_warn, &component_name).is_some_and(|&warned| warned)
            }) {
                console::error((
                    "%s: Function components do not support contextType.",
                    &component_name,
                ));
                DID_WARN_ABOUT_CONTEXT_TYPE_ON_FUNCTION_COMPONENT
                    .with(|did_warn| js::dict::set(did_warn, &component_name, true));
            }
        }
    }
}

thread_local! {
    // ReactFiberSuspenseComponent's `SuspenseState`.
    #[cfg_attr(rust_js, rust_js::name = "SUSPENDED_MARKER")]
    static SUSPENDED_MARKER: &'static Unknown = js::unknown_of(SuspenseState {
        dehydrated: js::null(),
        tree_context: js::null(),
        retry_lane: Cell::new(NO_LANE),
        hydration_errors: RefCell::new(js::null()),
    });
}

fn mount_suspense_offscreen_state(render_lanes: Lanes) -> OffscreenState {
    OffscreenState {
        base_lanes: render_lanes,
        cache_pool: get_suspended_cache().map(js::unknown_of),
    }
}

fn update_suspense_offscreen_state(
    prev_offscreen_state: &OffscreenState,
    render_lanes: Lanes,
) -> OffscreenState {
    let cache_pool: Option<&'static Unknown>;
    let prev_cache_pool: Option<&SpawnedCachePool> =
        unsafe { js::cast(prev_offscreen_state.cache_pool) };
    if let Some(prev_cache_pool) = prev_cache_pool {
        let parent_cache = CACHE_CONTEXT
            .with(|cache_context| {
                if IS_PRIMARY_RENDERER {
                    cache_context.current_value.borrow().clone()
                } else {
                    cache_context.current_value2.borrow().clone()
                }
            })
            .unwrap();
        if !Rc::ptr_eq(&prev_cache_pool.parent, &parent_cache) {
            // Detected a refresh in the parent. This overrides any previously
            // suspended cache.
            cache_pool = Some(js::unknown_of(SpawnedCachePool {
                parent: Rc::clone(&parent_cache),
                pool: parent_cache,
            }));
        } else {
            // We can reuse the cache from last time. The only thing that would have
            // overridden it is a parent refresh, which we checked for above.
            cache_pool = Some(js::unknown(prev_cache_pool));
        }
    } else {
        // If there's no previous cache pool, grab the current one.
        cache_pool = get_suspended_cache().map(js::unknown_of);
    }
    OffscreenState {
        base_lanes: merge_lanes(prev_offscreen_state.base_lanes, render_lanes),
        cache_pool,
    }
}

// TODO: Probably should inline this back
fn should_remain_on_fallback(
    current: Option<&Fiber>,
    _work_in_progress: &Fiber,
    _render_lanes: Lanes,
) -> bool {
    // If we're already showing a fallback, there are cases where we need to
    // remain on that fallback regardless of whether the content has resolved.
    // For example, SuspenseList coordinates when nested content appears.
    // TODO: For compatibility with offscreen prerendering, this should also check
    // whether the current fiber (if it exists) was visible in the previous tree.
    if let Some(current) = current {
        let suspense_state: Option<&SuspenseState> =
            unsafe { js::cast(current.memoized_state.get()) };
        if suspense_state.is_none() {
            // Currently showing content. Don't hide it, even if ForceSuspenseFallback
            // is true. More precise name might be "ForceRemainSuspenseFallback".
            // Note: This is a factoring smell. Can't remain on a fallback if there's
            // no fallback to remain on.
            return false;
        }
    }

    // Not currently showing content. Consult the Suspense context.
    let suspense_context: SuspenseContext =
        SUSPENSE_STACK_CURSOR.with(|cursor| *cursor.current.borrow());
    has_suspense_list_context(suspense_context, FORCE_SUSPENSE_FALLBACK)
}

fn get_remaining_work_in_primary_tree(
    current: Option<&Fiber>,
    primary_tree_did_defer: bool,
    render_lanes: Lanes,
) -> Lanes {
    let mut remaining_lanes = match current {
        Some(current) => remove_lanes(current.child_lanes.get(), render_lanes),
        None => NO_LANES,
    };
    if primary_tree_did_defer {
        // A useDeferredValue hook spawned a deferred task inside the primary tree.
        // Ensure that we retry this component at the deferred priority.
        // TODO: We could make this a per-subtree value instead of a global one.
        // Would need to track it on the context stack somehow, similar to what
        // we'd have to do for resumable contexts.
        remaining_lanes = merge_lanes(remaining_lanes, peek_deferred_lane());
    }
    remaining_lanes
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_suspense_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // shared/ReactTypes' `SuspenseProps`.
    let next_props = work_in_progress.pending_props.get();

    // This is used by DevTools to force a boundary to suspend.
    if __DEV__ && should_suspend(work_in_progress) {
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | DID_CAPTURE);
    }

    let mut show_fallback = false;
    let did_suspend = (work_in_progress.flags.get() & DID_CAPTURE) != NO_FLAGS;
    if did_suspend
        || should_remain_on_fallback(current.map(Rc::as_ref), work_in_progress, render_lanes)
    {
        // Something in this boundary's subtree already suspended. Switch to
        // rendering the fallback children.
        show_fallback = true;
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() & !DID_CAPTURE);
    }

    // Check if the primary children spawned a deferred task (useDeferredValue)
    // during the first pass.
    let did_primary_children_defer = (work_in_progress.flags.get() & DID_DEFER) != NO_FLAGS;
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() & !DID_DEFER);

    // OK, the next part is confusing. We're about to reconcile the Suspense
    // boundary's children. This involves some custom reconciliation logic. Two
    // main reasons this is so complicated.
    //
    // First, Legacy Mode has different semantics for backwards compatibility. The
    // primary tree will commit in an inconsistent state, so when we do the
    // second pass to render the fallback, we do some exceedingly, uh, clever
    // hacks to make that not totally break. Like transferring effects and
    // deletions from hidden tree. In Concurrent Mode, it's much simpler,
    // because we bailout on the primary tree completely and leave it in its old
    // state, no effects. Same as what we do for Offscreen (except that
    // Offscreen doesn't have the first render pass).
    //
    // Second is hydration. During hydration, the Suspense fiber has a slightly
    // different layout, where the child points to a dehydrated fragment, which
    // contains the DOM rendered by the server.
    //
    // Third, even if you set all that aside, Suspense is like error boundaries in
    // that we first we try to render one tree, and if that fails, we render again
    // and switch to a different tree. Like a try/catch block. So we have to track
    // which branch we're currently rendering. Ideally we would model this using
    // a stack.
    match current {
        None => {
            // Initial mount

            // Special path for hydration
            // If we're currently hydrating, try to hydrate this boundary.
            if get_is_hydrating() {
                // We must push the suspense handler context *before* attempting to
                // hydrate, to avoid a mismatch in case it errors.
                if show_fallback {
                    push_primary_tree_suspense_handler(work_in_progress);
                } else {
                    push_fallback_tree_suspense_handler(work_in_progress);
                }
                // This throws if we fail to hydrate.
                let dehydrated = claim_next_hydratable_suspense_instance(work_in_progress);
                return mount_dehydrated_suspense_component(
                    work_in_progress,
                    dehydrated,
                    render_lanes,
                );
            }

            let next_primary_children = js::get(next_props, "children");
            let next_fallback_children = js::get(next_props, "fallback");

            if show_fallback {
                push_fallback_tree_suspense_handler(work_in_progress);

                mount_suspense_fallback_children(
                    work_in_progress,
                    next_primary_children,
                    next_fallback_children,
                    render_lanes,
                );
                let primary_child_fragment = work_in_progress.child.borrow().clone().unwrap();
                primary_child_fragment
                    .memoized_state
                    .set(Some(js::unknown_of(mount_suspense_offscreen_state(
                        render_lanes,
                    ))));
                primary_child_fragment
                    .child_lanes
                    .set(get_remaining_work_in_primary_tree(
                        current.map(Rc::as_ref),
                        did_primary_children_defer,
                        render_lanes,
                    ));
                work_in_progress
                    .memoized_state
                    .set(Some(SUSPENDED_MARKER.with(|marker| *marker)));
                if ENABLE_TRANSITION_TRACING {
                    let current_transitions = get_pending_transitions();
                    if let Some(current_transitions) = current_transitions {
                        let parent_marker_instances = get_marker_instances();
                        let offscreen_queue: Option<&OffscreenQueue> =
                            unsafe { js::cast(primary_child_fragment.update_queue.get()) };
                        match offscreen_queue {
                            None => {
                                let new_offscreen_queue = OffscreenQueue {
                                    transitions: RefCell::new(Some(current_transitions)),
                                    marker_instances: RefCell::new(parent_marker_instances),
                                    retry_queue: Cell::new(js::null()),
                                };
                                primary_child_fragment
                                    .update_queue
                                    .set(Some(js::unknown_of(new_offscreen_queue)));
                            }
                            Some(offscreen_queue) => {
                                offscreen_queue
                                    .transitions
                                    .replace(Some(current_transitions));
                                offscreen_queue
                                    .marker_instances
                                    .replace(parent_marker_instances);
                            }
                        }
                    }
                }

                bailout_offscreen_component(js::null(), &primary_child_fragment)
            } else if ENABLE_CPU_SUSPENSE.with(|&enabled| enabled)
                && js::strict_equals(js::get(next_props, "defer"), true)
            {
                // This is a CPU-bound tree. Skip this tree and show a placeholder to
                // unblock the surrounding content. Then immediately retry after the
                // initial commit.
                push_fallback_tree_suspense_handler(work_in_progress);
                mount_suspense_fallback_children(
                    work_in_progress,
                    next_primary_children,
                    next_fallback_children,
                    render_lanes,
                );
                let primary_child_fragment = work_in_progress.child.borrow().clone().unwrap();
                primary_child_fragment
                    .memoized_state
                    .set(Some(js::unknown_of(mount_suspense_offscreen_state(
                        render_lanes,
                    ))));
                primary_child_fragment
                    .child_lanes
                    .set(get_remaining_work_in_primary_tree(
                        current.map(Rc::as_ref),
                        did_primary_children_defer,
                        render_lanes,
                    ));
                work_in_progress
                    .memoized_state
                    .set(Some(SUSPENDED_MARKER.with(|marker| *marker)));

                // TODO: Transition Tracing is not yet implemented for CPU Suspense.

                // Since nothing actually suspended, there will nothing to ping this to
                // get it started back up to attempt the next item. While in terms of
                // priority this work has the same priority as this current render, it's
                // not part of the same transition once the transition has committed. If
                // it's sync, we still want to yield so that it can be painted.
                // Conceptually, this is really the same as pinging. We can use any
                // RetryLane even if it's the one currently rendering since we're leaving
                // it behind on this node.
                work_in_progress.lanes.set(SOME_RETRY_LANE);
                bailout_offscreen_component(js::null(), &primary_child_fragment)
            } else {
                push_primary_tree_suspense_handler(work_in_progress);
                Some(mount_suspense_primary_children(
                    work_in_progress,
                    next_primary_children,
                    render_lanes,
                ))
            }
        }
        Some(current) => {
            // This is an update.

            // Special path for hydration
            let prev_state: Option<&SuspenseState> =
                unsafe { js::cast(current.memoized_state.get()) };
            if let Some(prev_state) = prev_state {
                let dehydrated = prev_state.dehydrated;
                if let Some(dehydrated) = dehydrated {
                    return update_dehydrated_suspense_component(
                        current,
                        work_in_progress,
                        did_suspend,
                        did_primary_children_defer,
                        next_props,
                        dehydrated,
                        prev_state,
                        render_lanes,
                    );
                }
            }

            if show_fallback {
                push_fallback_tree_suspense_handler(work_in_progress);

                let next_fallback_children = js::get(next_props, "fallback");
                let next_primary_children = js::get(next_props, "children");
                update_suspense_fallback_children(
                    current,
                    work_in_progress,
                    next_primary_children,
                    next_fallback_children,
                    render_lanes,
                );
                let primary_child_fragment = work_in_progress.child.borrow().clone().unwrap();
                let prev_offscreen_state: Option<&OffscreenState> = unsafe {
                    js::cast(
                        current
                            .child
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .memoized_state
                            .get(),
                    )
                };
                primary_child_fragment
                    .memoized_state
                    .set(Some(js::unknown_of(match prev_offscreen_state {
                        None => mount_suspense_offscreen_state(render_lanes),
                        Some(prev_offscreen_state) => {
                            update_suspense_offscreen_state(prev_offscreen_state, render_lanes)
                        }
                    })));
                if ENABLE_TRANSITION_TRACING {
                    let current_transitions = get_pending_transitions();
                    if let Some(current_transitions) = current_transitions {
                        let parent_marker_instances = get_marker_instances();
                        let offscreen_queue: Option<&OffscreenQueue> =
                            unsafe { js::cast(primary_child_fragment.update_queue.get()) };
                        let current_offscreen_queue: Option<&OffscreenQueue> =
                            unsafe { js::cast(current.update_queue.get()) };
                        match offscreen_queue {
                            None => {
                                let new_offscreen_queue = OffscreenQueue {
                                    transitions: RefCell::new(Some(current_transitions)),
                                    marker_instances: RefCell::new(parent_marker_instances),
                                    retry_queue: Cell::new(js::null()),
                                };
                                primary_child_fragment
                                    .update_queue
                                    .set(Some(js::unknown_of(new_offscreen_queue)));
                            }
                            Some(offscreen_queue)
                                if current_offscreen_queue.is_some_and(
                                    |current_offscreen_queue| {
                                        std::ptr::eq(offscreen_queue, current_offscreen_queue)
                                    },
                                ) =>
                            {
                                // If the work-in-progress queue is the same object as current, we
                                // can't modify it without cloning it first.
                                let new_offscreen_queue = OffscreenQueue {
                                    transitions: RefCell::new(Some(current_transitions)),
                                    marker_instances: RefCell::new(parent_marker_instances),
                                    retry_queue: Cell::new(match current_offscreen_queue {
                                        Some(current_offscreen_queue) => {
                                            current_offscreen_queue.retry_queue.get()
                                        }
                                        None => js::null(),
                                    }),
                                };
                                primary_child_fragment
                                    .update_queue
                                    .set(Some(js::unknown_of(new_offscreen_queue)));
                            }
                            Some(offscreen_queue) => {
                                offscreen_queue
                                    .transitions
                                    .replace(Some(current_transitions));
                                offscreen_queue
                                    .marker_instances
                                    .replace(parent_marker_instances);
                            }
                        }
                    }
                }
                primary_child_fragment
                    .child_lanes
                    .set(get_remaining_work_in_primary_tree(
                        Some(current),
                        did_primary_children_defer,
                        render_lanes,
                    ));
                work_in_progress
                    .memoized_state
                    .set(Some(SUSPENDED_MARKER.with(|marker| *marker)));
                let current_child = current.child.borrow().clone();
                bailout_offscreen_component(current_child.as_ref(), &primary_child_fragment)
            } else {
                if prev_state.is_some()
                    && includes_only_retries(render_lanes)
                    && includes_some_lane(render_lanes, current.lanes.get())
                {
                    // If we're rendering Retry lanes and we're entering the primary content then it's possible
                    // that the only reason we rendered was because we left this boundary to be warmed up but
                    // nothing else scheduled an update. If so, use it as the cause of the render.
                    mark_render_derived_cause(work_in_progress);
                }

                push_primary_tree_suspense_handler(work_in_progress);

                let next_primary_children = js::get(next_props, "children");
                let primary_child_fragment = update_suspense_primary_children(
                    current,
                    work_in_progress,
                    next_primary_children,
                    render_lanes,
                );
                work_in_progress.memoized_state.set(js::null());
                Some(primary_child_fragment)
            }
        }
    }
}

fn mount_suspense_primary_children(
    work_in_progress: &Rc<Fiber>,
    primary_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Rc<Fiber> {
    let mode = work_in_progress.mode.get();
    let primary_child_props = OffscreenProps {
        mode: Some(OffscreenMode::Visible),
        children: primary_children,
    };
    let primary_child_fragment =
        mount_work_in_progress_offscreen_fiber(primary_child_props, mode, render_lanes);
    primary_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    work_in_progress
        .child
        .replace(Some(Rc::clone(&primary_child_fragment)));
    primary_child_fragment
}

fn mount_suspense_fallback_children(
    work_in_progress: &Rc<Fiber>,
    primary_children: Option<&'static Unknown>,
    fallback_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Rc<Fiber> {
    let mode = work_in_progress.mode.get();
    let progressed_primary_fragment = work_in_progress.child.borrow().clone();

    let primary_child_props = OffscreenProps {
        mode: Some(OffscreenMode::Hidden),
        children: primary_children,
    };

    let primary_child_fragment;
    let fallback_child_fragment;
    if !DISABLE_LEGACY_MODE
        && (mode & CONCURRENT_MODE) == NO_MODE
        && let Some(progressed_primary_fragment) = progressed_primary_fragment
    {
        // In legacy mode, we commit the primary tree as if it successfully
        // completed, even though it's in an inconsistent state.
        primary_child_fragment = progressed_primary_fragment;
        primary_child_fragment.child_lanes.set(NO_LANES);
        primary_child_fragment
            .pending_props
            .set(Some(js::unknown_of(primary_child_props)));

        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && (work_in_progress.mode.get() & PROFILE_MODE) != 0
        {
            // Reset the durations from the first pass so they aren't included in the
            // final amounts. This seems counterintuitive, since we're intentionally
            // not measuring part of the render phase, but this makes it match what we
            // do in Concurrent Mode.
            primary_child_fragment.actual_duration.set(Some(-0.0));
            primary_child_fragment.actual_start_time.set(Some(-1.1));
            primary_child_fragment.self_base_duration.set(Some(-0.0));
            primary_child_fragment.tree_base_duration.set(Some(-0.0));
        }

        fallback_child_fragment =
            create_fiber_from_fragment(fallback_children, mode, render_lanes, js::null());
    } else {
        primary_child_fragment =
            mount_work_in_progress_offscreen_fiber(primary_child_props, mode, NO_LANES);
        fallback_child_fragment =
            create_fiber_from_fragment(fallback_children, mode, render_lanes, js::null());
    }

    primary_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    fallback_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    primary_child_fragment
        .sibling
        .replace(Some(Rc::clone(&fallback_child_fragment)));
    work_in_progress.child.replace(Some(primary_child_fragment));
    fallback_child_fragment
}

fn mount_work_in_progress_offscreen_fiber(
    offscreen_props: OffscreenProps,
    mode: TypeOfMode,
    _render_lanes: Lanes,
) -> Rc<Fiber> {
    // The props argument to `createFiberFromOffscreen` is `any` typed, so we use
    // this wrapper function to constrain it.
    create_fiber_from_offscreen(
        unsafe { js::cast(Some(js::unknown_of(offscreen_props))) },
        mode,
        NO_LANES,
        js::null(),
    )
}

fn update_work_in_progress_offscreen_fiber(
    current: &Rc<Fiber>,
    offscreen_props: OffscreenProps,
) -> Rc<Fiber> {
    // The props argument to `createWorkInProgress` is `any` typed, so we use this
    // wrapper function to constrain it.
    create_work_in_progress(current, Some(js::unknown_of(offscreen_props)))
}

fn update_suspense_primary_children(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    primary_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Rc<Fiber> {
    let current_primary_child_fragment = current.child.borrow().clone().unwrap();
    let current_fallback_child_fragment = current_primary_child_fragment.sibling.borrow().clone();

    let primary_child_fragment = update_work_in_progress_offscreen_fiber(
        &current_primary_child_fragment,
        OffscreenProps {
            mode: Some(OffscreenMode::Visible),
            children: primary_children,
        },
    );
    if !DISABLE_LEGACY_MODE && (work_in_progress.mode.get() & CONCURRENT_MODE) == NO_MODE {
        primary_child_fragment.lanes.set(render_lanes);
    }
    primary_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    primary_child_fragment.sibling.replace(None);
    if let Some(current_fallback_child_fragment) = current_fallback_child_fragment {
        // Delete the fallback child fragment
        let deletions = work_in_progress.deletions.borrow().clone();
        match deletions {
            None => {
                work_in_progress
                    .deletions
                    .replace(Some(Rc::new(RefCell::new(vec![
                        current_fallback_child_fragment,
                    ]))));
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | CHILD_DELETION);
            }
            Some(deletions) => deletions.borrow_mut().push(current_fallback_child_fragment),
        }
    }

    work_in_progress
        .child
        .replace(Some(Rc::clone(&primary_child_fragment)));
    primary_child_fragment
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_suspense_fallback_children(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    primary_children: Option<&'static Unknown>,
    fallback_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let mode = work_in_progress.mode.get();
    let current_primary_child_fragment = current.child.borrow().clone().unwrap();
    let current_fallback_child_fragment = current_primary_child_fragment.sibling.borrow().clone();

    let primary_child_props = OffscreenProps {
        mode: Some(OffscreenMode::Hidden),
        children: primary_children,
    };

    let primary_child_fragment;
    if
    // In legacy mode, we commit the primary tree as if it successfully
    // completed, even though it's in an inconsistent state.
    !DISABLE_LEGACY_MODE
        && (mode & CONCURRENT_MODE) == NO_MODE
        // Make sure we're on the second pass, i.e. the primary child fragment was
        // already cloned. In legacy mode, the only case where this isn't true is
        // when DevTools forces us to display a fallback; we skip the first render
        // pass entirely and go straight to rendering the fallback. (In Concurrent
        // Mode, SuspenseList can also trigger this scenario, but this is a legacy-
        // only codepath.)
        && work_in_progress.child.borrow().as_ref() != Some(&current_primary_child_fragment)
    {
        let progressed_primary_fragment = work_in_progress.child.borrow().clone().unwrap();
        primary_child_fragment = progressed_primary_fragment;
        primary_child_fragment.child_lanes.set(NO_LANES);
        primary_child_fragment
            .pending_props
            .set(Some(js::unknown_of(primary_child_props)));

        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && (work_in_progress.mode.get() & PROFILE_MODE) != 0
        {
            // Reset the durations from the first pass so they aren't included in the
            // final amounts. This seems counterintuitive, since we're intentionally
            // not measuring part of the render phase, but this makes it match what we
            // do in Concurrent Mode.
            primary_child_fragment.actual_duration.set(Some(-0.0));
            primary_child_fragment.actual_start_time.set(Some(-1.1));
            primary_child_fragment
                .self_base_duration
                .set(current_primary_child_fragment.self_base_duration.get());
            primary_child_fragment
                .tree_base_duration
                .set(current_primary_child_fragment.tree_base_duration.get());
        }

        // The fallback fiber was added as a deletion during the first pass.
        // However, since we're going to remain on the fallback, we no longer want
        // to delete it.
        work_in_progress.deletions.replace(js::null());
    } else {
        primary_child_fragment = update_work_in_progress_offscreen_fiber(
            &current_primary_child_fragment,
            primary_child_props,
        );
        // Since we're reusing a current tree, we need to reuse the flags, too.
        // (We don't do this in legacy mode, because in legacy mode we don't re-use
        // the current tree; see previous branch.)
        primary_child_fragment
            .subtree_flags
            .set(current_primary_child_fragment.subtree_flags.get() & STATIC_MASK);
    }
    let fallback_child_fragment;
    if let Some(current_fallback_child_fragment) = current_fallback_child_fragment {
        fallback_child_fragment =
            create_work_in_progress(&current_fallback_child_fragment, fallback_children);
    } else {
        fallback_child_fragment =
            create_fiber_from_fragment(fallback_children, mode, render_lanes, js::null());
        // Needs a placement effect because the parent (the Suspense boundary) already
        // mounted but this is a new fiber.
        fallback_child_fragment
            .flags
            .set(fallback_child_fragment.flags.get() | PLACEMENT);
    }

    fallback_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    primary_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    primary_child_fragment
        .sibling
        .replace(Some(fallback_child_fragment));
    work_in_progress
        .child
        .replace(Some(Rc::clone(&primary_child_fragment)));

    bailout_offscreen_component(js::null(), &primary_child_fragment)
}

fn retry_suspense_component_without_hydrating(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Rc<Fiber> {
    // Falling back to client rendering. Because this has performance
    // implications, it's considered a recoverable error, even though the user
    // likely won't observe anything wrong with the UI.

    // This will add the old fiber to the deletion list
    let current_child = current.child.borrow().clone();
    RECONCILE_CHILD_FIBERS.with(|reconcile_child_fibers| {
        reconcile_child_fibers(work_in_progress, current_child, js::null(), render_lanes)
    });

    // We're now not suspended nor dehydrated.
    let next_props = work_in_progress.pending_props.get();
    let primary_children = js::get(next_props, "children");
    let primary_child_fragment =
        mount_suspense_primary_children(work_in_progress, primary_children, render_lanes);
    // Needs a placement effect because the parent (the Suspense boundary) already
    // mounted but this is a new fiber.
    primary_child_fragment
        .flags
        .set(primary_child_fragment.flags.get() | PLACEMENT);
    work_in_progress.memoized_state.set(js::null());

    primary_child_fragment
}

fn mount_suspense_fallback_after_retry_without_hydrating(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    primary_children: Option<&'static Unknown>,
    fallback_children: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> Rc<Fiber> {
    let fiber_mode = work_in_progress.mode.get();
    let primary_child_props = OffscreenProps {
        mode: Some(OffscreenMode::Visible),
        children: primary_children,
    };
    let primary_child_fragment =
        mount_work_in_progress_offscreen_fiber(primary_child_props, fiber_mode, NO_LANES);
    let fallback_child_fragment =
        create_fiber_from_fragment(fallback_children, fiber_mode, render_lanes, js::null());
    // Needs a placement effect because the parent (the Suspense
    // boundary) already mounted but this is a new fiber.
    fallback_child_fragment
        .flags
        .set(fallback_child_fragment.flags.get() | PLACEMENT);

    primary_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    fallback_child_fragment
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    primary_child_fragment
        .sibling
        .replace(Some(Rc::clone(&fallback_child_fragment)));
    work_in_progress.child.replace(Some(primary_child_fragment));

    if DISABLE_LEGACY_MODE || (work_in_progress.mode.get() & CONCURRENT_MODE) != NO_MODE {
        // We will have dropped the effect list which contains the
        // deletion. We need to reconcile to delete the current child.
        let current_child = current.child.borrow().clone();
        RECONCILE_CHILD_FIBERS.with(|reconcile_child_fibers| {
            reconcile_child_fibers(work_in_progress, current_child, js::null(), render_lanes)
        });
    }

    fallback_child_fragment
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn mount_dehydrated_suspense_component(
    work_in_progress: &Fiber,
    suspense_instance: &'static Unknown,
    _render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // During the first pass, we'll bail out and not drill into the children.
    // Instead, we'll leave the content in place and try to hydrate it later.
    if is_suspense_instance_fallback(suspense_instance) {
        // This is a client-only boundary. Since we won't get any content from the server
        // for this, we need to schedule that at a higher priority based on when it would
        // have timed out. In theory we could render it in this pass but it would have the
        // wrong priority associated with it and will prevent hydration of parent path.
        // Instead, we'll leave work left on it to render it in a separate commit.
        // Schedule a normal pri update to render this content.
        work_in_progress.lanes.set(lane_to_lanes(DEFAULT_LANE));
    } else {
        // We'll continue hydrating the rest at offscreen priority since we'll already
        // be showing the right content coming from the server, it is no rush.
        work_in_progress.lanes.set(lane_to_lanes(OFFSCREEN_LANE));
    }
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_dehydrated_suspense_component(
    current: &Rc<Fiber>,
    work_in_progress: &Rc<Fiber>,
    did_suspend: bool,
    did_primary_children_defer: bool,
    next_props: Option<&'static Unknown>,
    suspense_instance: &'static Unknown,
    suspense_state: &SuspenseState,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if !did_suspend {
        // This is the first render pass. Attempt to hydrate.
        push_primary_tree_suspense_handler(work_in_progress);

        // We should never be hydrating at this point because it is the first pass,
        // but after we've already committed once.
        warn_if_hydrating();

        if includes_some_lane(render_lanes, OFFSCREEN_LANE) {
            // If we're rendering Offscreen and we're entering the activity then it's possible
            // that the only reason we rendered was because this boundary left work. Provide
            // it as a cause if another one doesn't already exist.
            mark_render_derived_cause(work_in_progress);
        }

        if is_suspense_instance_fallback(suspense_instance) {
            // This boundary is in a permanent fallback state. In this case, we'll never
            // get an update and we'll never be able to hydrate the final content. Let's just try the
            // client side render instead.
            let digest: Option<String>;
            let mut message = None;
            let mut stack = js::null();
            let mut component_stack = js::null();
            if __DEV__ {
                SuspenseInstanceFallbackErrorDetails {
                    digest,
                    message,
                    stack,
                    component_stack,
                } = get_suspense_instance_fallback_error_details(suspense_instance);
            } else {
                SuspenseInstanceFallbackErrorDetails { digest, .. } =
                    get_suspense_instance_fallback_error_details(suspense_instance);
            }

            // This is unreachable in renderers that do not support hydration.
            if digest.as_deref() != Some(REACT_RECOVERABLE_DIGEST) {
                let error: &'static Error;
                if __DEV__
                    && let Some(message) = &message
                    && !message.is_empty()
                {
                    // eslint-disable-next-line react-internal/prod-error-codes
                    error = js::error::new(message, None);
                } else {
                    error = js::error::new(
                        concat!(
                            "The server could not finish this Suspense boundary, likely ",
                            "due to an error during server rendering. ",
                            "Switched to client rendering."
                        ),
                        None,
                    );
                }
                // Replace the stack with the server stack
                js::set(
                    error,
                    "stack",
                    if __DEV__ && js::truthy(stack.as_deref()) {
                        stack
                    } else {
                        Some(String::new())
                    },
                );
                js::set(error, "digest", digest);
                let captured_value =
                    create_captured_value_from_error(error, component_stack.or(js::null()));
                queue_hydration_error(captured_value);
            }
            return Some(retry_suspense_component_without_hydrating(
                current,
                work_in_progress,
                render_lanes,
            ));
        }

        if
        // TODO: Factoring is a little weird, since we check this right below, too.
        !DID_RECEIVE_UPDATE.get() {
            // We need to check if any children have context before we decide to bail
            // out, so propagate the changes now.
            lazily_propagate_parent_context_changes(current, work_in_progress, render_lanes);
        }

        // We use lanes to indicate that a child might depend on context, so if
        // any context has changed, we need to treat is as if the input might have changed.
        let has_context_changed = includes_some_lane(render_lanes, current.child_lanes.get());
        if DID_RECEIVE_UPDATE.get() || has_context_changed {
            // This boundary has changed since the first render. This means that we are now unable to
            // hydrate it. We might still be able to hydrate it using a higher priority lane.
            if is_current_tree_hidden() {
                // This boundary is inside a hidden subtree, where all work is
                // deferred until the tree is revealed. Selective hydration works by
                // rendering the boundary at a higher priority before the update
                // applies, so it can't make progress here; delaying the commit to
                // wait for it would deadlock. Replacing hidden content isn't
                // visible, so give up and client render.
                return Some(retry_suspense_component_without_hydrating(
                    current,
                    work_in_progress,
                    render_lanes,
                ));
            }
            let root = get_work_in_progress_root();
            if let Some(root) = root {
                let attempt_hydration_at_lane = get_bumped_lane_for_hydration(&root, render_lanes);
                if attempt_hydration_at_lane != NO_LANE
                    && attempt_hydration_at_lane != suspense_state.retry_lane.get()
                {
                    // Intentionally mutating since this render will get interrupted. This
                    // is one of the very rare times where we mutate the current tree
                    // during the render phase.
                    suspense_state.retry_lane.set(attempt_hydration_at_lane);
                    enqueue_concurrent_render_for_lane(current, attempt_hydration_at_lane);
                    schedule_update_on_fiber(&root, current, attempt_hydration_at_lane);

                    // Throw a special object that signals to the work loop that it should
                    // interrupt the current render.
                    //
                    // Because we're inside a React-only execution stack, we don't
                    // strictly need to throw here — we could instead modify some internal
                    // work loop state. But using an exception means we don't need to
                    // check for this case on every iteration of the work loop. So doing
                    // it this way moves the check out of the fast path.
                    SELECTIVE_HYDRATION_EXCEPTION
                        .with(|exception| js::throw(exception.as_unknown()));
                } else {
                    // We have already tried to ping at a higher priority than we're rendering with
                    // so if we got here, we must have failed to hydrate at those levels. We must
                    // now give up. Instead, we're going to delete the whole subtree and instead inject
                    // a new real Suspense boundary to take its place, which may render content
                    // or fallback. This might suspend for a while and if it does we might still have
                    // an opportunity to hydrate before this pass commits.
                }
            }

            // If we did not selectively hydrate, we'll continue rendering without
            // hydrating. Mark this tree as suspended to prevent it from committing
            // outside a transition.
            //
            // This path should only happen if the hydration lane already suspended.
            if is_suspense_instance_pending(suspense_instance) {
                // This is a dehydrated suspense instance. We don't need to suspend
                // because we're already showing a fallback.
                // TODO: The Fizz runtime might still stream in completed HTML, out-of-
                // band. Should we fix this? There's a version of this bug that happens
                // during client rendering, too. Needs more consideration.
            } else {
                render_did_suspend_delay_if_possible();
            }
            Some(retry_suspense_component_without_hydrating(
                current,
                work_in_progress,
                render_lanes,
            ))
        } else if is_suspense_instance_pending(suspense_instance) {
            // This component is still pending more data from the server, so we can't hydrate its
            // content. We treat it as if this component suspended itself. It might seem as if
            // we could just try to render it client-side instead. However, this will perform a
            // lot of unnecessary work and is unlikely to complete since it often will suspend
            // on missing data anyway. Additionally, the server might be able to render more
            // than we can on the client yet. In that case we'd end up with more fallback states
            // on the client than if we just leave it alone. If the server times out or errors
            // these should update this boundary to the permanent Fallback state instead.
            // Mark it as having captured (i.e. suspended).
            // Also Mark it as requiring retry.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | DID_CAPTURE | CALLBACK);
            // Leave the child in place. I.e. the dehydrated fragment.
            let child = current.child.borrow().clone();
            work_in_progress.child.replace(child);
            None
        } else {
            // This is the first attempt.
            reenter_hydration_state_from_dehydrated_suspense_instance(
                work_in_progress,
                suspense_instance,
                suspense_state.tree_context.as_ref(),
            );
            let primary_children = js::get(next_props, "children");
            let primary_child_fragment =
                mount_suspense_primary_children(work_in_progress, primary_children, render_lanes);
            // Mark the children as hydrating. This is a fast path to know whether this
            // tree is part of a hydrating tree. This is used to determine if a child
            // node has fully mounted yet, and for scheduling event replaying.
            // Conceptually this is similar to Placement in that a new subtree is
            // inserted into the React tree here. It just happens to not need DOM
            // mutations because it already exists.
            // We should still treat it as a newly inserted Fiber to double invoke Strict Effects.
            primary_child_fragment
                .flags
                .set(primary_child_fragment.flags.get() | HYDRATING | PLACEMENT_DEV);
            Some(primary_child_fragment)
        }
    } else {
        // This is the second render pass. We already attempted to hydrated, but
        // something either suspended or errored.

        if (work_in_progress.flags.get() & FORCE_CLIENT_RENDER) != 0 {
            // Something errored during hydration. Try again without hydrating.
            // The error should've already been logged in throwException.
            push_primary_tree_suspense_handler(work_in_progress);
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() & !FORCE_CLIENT_RENDER);
            Some(retry_suspense_component_without_hydrating(
                current,
                work_in_progress,
                render_lanes,
            ))
        } else if work_in_progress.memoized_state.get().is_some() {
            // Something suspended and we should still be in dehydrated mode.
            // Leave the existing child in place.

            // Push to avoid a mismatch
            push_fallback_tree_suspense_handler(work_in_progress);

            let child = current.child.borrow().clone();
            work_in_progress.child.replace(child);
            // The dehydrated completion pass expects this flag to be there
            // but the normal suspense pass doesn't.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | DID_CAPTURE);
            None
        } else {
            // Suspended but we should no longer be in dehydrated mode.
            // Therefore we now have to render the fallback.
            push_fallback_tree_suspense_handler(work_in_progress);

            let next_primary_children = js::get(next_props, "children");
            let next_fallback_children = js::get(next_props, "fallback");
            mount_suspense_fallback_after_retry_without_hydrating(
                current,
                work_in_progress,
                next_primary_children,
                next_fallback_children,
                render_lanes,
            );
            let primary_child_fragment = work_in_progress.child.borrow().clone().unwrap();
            primary_child_fragment
                .memoized_state
                .set(Some(js::unknown_of(mount_suspense_offscreen_state(
                    render_lanes,
                ))));
            primary_child_fragment
                .child_lanes
                .set(get_remaining_work_in_primary_tree(
                    Some(current),
                    did_primary_children_defer,
                    render_lanes,
                ));
            work_in_progress
                .memoized_state
                .set(Some(SUSPENDED_MARKER.with(|marker| *marker)));
            bailout_offscreen_component(js::null(), &primary_child_fragment)
        }
    }
}

fn schedule_suspense_work_on_fiber(fiber: &Fiber, render_lanes: Lanes, propagation_root: &Fiber) {
    fiber
        .lanes
        .set(merge_lanes(fiber.lanes.get(), render_lanes));
    let alternate = fiber.alternate.borrow().clone();
    if let Some(alternate) = alternate {
        alternate
            .lanes
            .set(merge_lanes(alternate.lanes.get(), render_lanes));
    }
    schedule_context_work_on_parent_path(
        fiber.r#return.borrow().clone(),
        render_lanes,
        propagation_root,
    );
}

fn propagate_suspense_context_change(
    work_in_progress: &Rc<Fiber>,
    first_child: Option<Rc<Fiber>>,
    render_lanes: Lanes,
) {
    // Mark any Suspense boundaries with fallbacks as having work to do.
    // If they were previously forced into fallbacks, they may now be able
    // to unblock.
    let mut node = first_child;
    while let Some(mut current) = node {
        let child = current.child.borrow().clone();
        if current.tag.get() == SUSPENSE_COMPONENT {
            let state: Option<&SuspenseState> = unsafe { js::cast(current.memoized_state.get()) };
            if state.is_some() {
                schedule_suspense_work_on_fiber(&current, render_lanes, work_in_progress);
            }
        } else if current.tag.get() == SUSPENSE_LIST_COMPONENT {
            // If the tail is hidden there might not be an Suspense boundaries
            // to schedule work on. In this case we have to schedule it on the
            // list itself.
            // We don't have to traverse to the children of the list since
            // the list will propagate the change when it rerenders.
            schedule_suspense_work_on_fiber(&current, render_lanes, work_in_progress);
        } else if let Some(child) = child {
            child.r#return.replace(Some(Rc::clone(&current)));
            node = Some(child);
            continue;
        }
        if current == *work_in_progress {
            return;
        }
        while current.sibling.borrow().is_none() {
            let return_fiber = current.r#return.borrow().clone();
            match return_fiber {
                Some(return_fiber) if return_fiber != *work_in_progress => {
                    current = return_fiber;
                }
                _ => return,
            }
        }
        let sibling = current.sibling.borrow().clone().unwrap();
        let return_fiber = current.r#return.borrow().clone();
        sibling.r#return.replace(return_fiber);
        node = Some(sibling);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn find_last_content_row(first_child: Option<Rc<Fiber>>) -> Option<Rc<Fiber>> {
    // This is going to find the last row among these children that is already
    // showing content on the screen, as opposed to being in fallback state or
    // new. If a row has multiple Suspense boundaries, any of them being in the
    // fallback state, counts as the whole row being in a fallback state.
    // Note that the "rows" will be workInProgress, but any nested children
    // will still be current since we haven't rendered them yet. The mounted
    // order may not be the same as the new order. We use the new order.
    let mut row = first_child;
    let mut last_content_row = js::null();
    while let Some(this_row) = row {
        let current_row = this_row.alternate.borrow().clone();
        // New rows can't be content rows.
        if let Some(current_row) = current_row
            && find_first_suspended(&current_row).is_none()
        {
            last_content_row = Some(Rc::clone(&this_row));
        }
        row = this_row.sibling.borrow().clone();
    }
    last_content_row
}

// shared/ReactTypes' `SuspenseListRevealOrder`.
fn validate_reveal_order(reveal_order: Option<&'static Unknown>) {
    if __DEV__ {
        let cache_key = match reveal_order {
            None => "null".to_string(),
            Some(reveal_order) => js::string(Some(reveal_order)),
        };
        if reveal_order.is_some()
            && !js::strict_equals(reveal_order, "forwards")
            && !js::strict_equals(reveal_order, "backwards")
            && !js::strict_equals(reveal_order, "unstable_legacy-backwards")
            && !js::strict_equals(reveal_order, "together")
            && !js::strict_equals(reveal_order, "independent")
            && !DID_WARN_ABOUT_REVEAL_ORDER
                .with(|did_warn| js::dict::get(did_warn, &cache_key).is_some_and(|&warned| warned))
        {
            DID_WARN_ABOUT_REVEAL_ORDER.with(|did_warn| js::dict::set(did_warn, &cache_key, true));
            if let Kind::String(reveal_order) = js::classify(reveal_order) {
                match reveal_order.to_lowercase().as_str() {
                    "together" | "forwards" | "backwards" | "independent" => {
                        console::error((
                            concat!(
                                "\"%s\" is not a valid value for revealOrder on <SuspenseList />. ",
                                "Use lowercase \"%s\" instead."
                            ),
                            reveal_order,
                            reveal_order.to_lowercase(),
                        ));
                    }
                    "forward" | "backward" => {
                        console::error((
                            concat!(
                                "\"%s\" is not a valid value for revealOrder on <SuspenseList />. ",
                                "React uses the -s suffix in the spelling. Use \"%ss\" instead."
                            ),
                            reveal_order,
                            reveal_order.to_lowercase(),
                        ));
                    }
                    _ => {
                        console::error((
                            concat!(
                                "\"%s\" is not a supported revealOrder on <SuspenseList />. ",
                                "Did you mean \"independent\", \"together\", \"forwards\" or \"backwards\"?"
                            ),
                            reveal_order,
                        ));
                    }
                }
            } else {
                console::error((
                    concat!(
                        "%s is not a supported value for revealOrder on <SuspenseList />. ",
                        "Did you mean \"independent\", \"together\", \"forwards\" or \"backwards\"?"
                    ),
                    reveal_order,
                ));
            }
        }
    }
}

// shared/ReactTypes' `SuspenseListTailMode` and `SuspenseListRevealOrder`.
fn validate_tail_options(
    tail_mode: Option<&'static Unknown>,
    reveal_order: Option<&'static Unknown>,
) {
    if __DEV__ {
        let cache_key = match tail_mode {
            None => "null".to_string(),
            Some(tail_mode) => js::string(Some(tail_mode)),
        };
        if !DID_WARN_ABOUT_TAIL_OPTIONS
            .with(|did_warn| js::dict::get(did_warn, &cache_key).is_some_and(|&warned| warned))
        {
            if tail_mode.is_none() {
                // The default tail is now "hidden".
            } else if !js::strict_equals(tail_mode, "visible")
                && !js::strict_equals(tail_mode, "collapsed")
                && !js::strict_equals(tail_mode, "hidden")
            {
                DID_WARN_ABOUT_TAIL_OPTIONS
                    .with(|did_warn| js::dict::set(did_warn, &cache_key, true));
                console::error((
                    concat!(
                        "\"%s\" is not a supported value for tail on <SuspenseList />. ",
                        "Did you mean \"visible\", \"collapsed\" or \"hidden\"?"
                    ),
                    tail_mode,
                ));
            } else if reveal_order.is_some()
                && !js::strict_equals(reveal_order, "forwards")
                && !js::strict_equals(reveal_order, "backwards")
                && !js::strict_equals(reveal_order, "unstable_legacy-backwards")
            {
                DID_WARN_ABOUT_TAIL_OPTIONS
                    .with(|did_warn| js::dict::set(did_warn, &cache_key, true));
                console::error((
                    concat!(
                        "<SuspenseList tail=\"%s\" /> is only valid if revealOrder is ",
                        "\"forwards\" (default) or \"backwards\". ",
                        "Did you mean to specify revealOrder=\"forwards\"?"
                    ),
                    tail_mode,
                ));
            }
        }
    }
}

fn init_suspense_list_render_state(
    work_in_progress: &Fiber,
    is_backwards: bool,
    tail: Option<Rc<Fiber>>,
    last_content_row: Option<Rc<Fiber>>,
    tail_mode: Option<SuspenseListTailMode>,
    tree_fork_count: i32,
) {
    let render_state: Option<&SuspenseListRenderState> =
        unsafe { js::cast(work_in_progress.memoized_state.get()) };
    match render_state {
        None => {
            work_in_progress
                .memoized_state
                .set(Some(js::unknown_of(SuspenseListRenderState {
                    is_backwards: Cell::new(is_backwards),
                    rendering: RefCell::new(js::null()),
                    rendering_start_time: Cell::new(0.0),
                    last: RefCell::new(last_content_row),
                    tail: RefCell::new(tail),
                    tail_mode: Cell::new(tail_mode),
                    tree_fork_count: Cell::new(tree_fork_count),
                })));
        }
        Some(render_state) => {
            // We can reuse the existing object from previous renders.
            render_state.is_backwards.set(is_backwards);
            render_state.rendering.replace(js::null());
            render_state.rendering_start_time.set(0.0);
            render_state.last.replace(last_content_row);
            render_state.tail.replace(tail);
            render_state.tail_mode.set(tail_mode);
            render_state.tree_fork_count.set(tree_fork_count);
        }
    }
}

fn reverse_children(fiber: &Fiber) {
    let mut row = fiber.child.borrow().clone();
    fiber.child.replace(js::null());
    while let Some(this_row) = row {
        let next_row = this_row.sibling.borrow().clone();
        let child = fiber.child.borrow().clone();
        this_row.sibling.replace(child);
        fiber.child.replace(Some(this_row));
        row = next_row;
    }
}

// This can end up rendering this component multiple passes.
// The first pass splits the children fibers into two sets. A head and tail.
// We first render the head. If anything is in fallback state, we do another
// pass through beginWork to rerender all children (including the tail) with
// the force suspend context. If the first render didn't have anything in
// in fallback state. Then we render each row in the tail one-by-one.
// That happens in the completeWork phase without going back to beginWork.
#[cfg_attr(rust_js, rust_js::nullable)]
fn update_suspense_list_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // shared/ReactTypes' `SuspenseListProps`.
    let next_props = work_in_progress.pending_props.get();
    let reveal_order = js::get(next_props, "revealOrder");
    let tail_mode = js::get(next_props, "tail");
    let new_children = js::get(next_props, "children");

    let mut suspense_context: SuspenseContext =
        SUSPENSE_STACK_CURSOR.with(|cursor| *cursor.current.borrow());

    if (work_in_progress.flags.get() & DID_CAPTURE) != 0 {
        // This is the second pass after having suspended in a row. Proceed directly
        // to the complete phase.
        push_suspense_list_context(work_in_progress, suspense_context);
        return None;
    }

    let should_force_fallback =
        has_suspense_list_context(suspense_context, FORCE_SUSPENSE_FALLBACK);
    if should_force_fallback {
        suspense_context =
            set_shallow_suspense_list_context(suspense_context, FORCE_SUSPENSE_FALLBACK);
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | DID_CAPTURE);
    } else {
        suspense_context = set_default_shallow_suspense_list_context(suspense_context);
    }
    push_suspense_list_context(work_in_progress, suspense_context);

    validate_reveal_order(reveal_order);
    validate_tail_options(tail_mode, reveal_order);
    validate_suspense_list_children(new_children, unsafe { js::cast(reveal_order) });

    if js::strict_equals(reveal_order, "backwards")
        && let Some(current) = current
    {
        // For backwards the current mounted set will be backwards. Reconciling against it
        // will lead to mismatches and reorders. We need to swap the original set first
        // and then restore it afterwards.
        reverse_children(current);
        reconcile_children(Some(current), work_in_progress, new_children, render_lanes);
        reverse_children(current);
    } else {
        reconcile_children(current, work_in_progress, new_children, render_lanes);
    }
    // Read how many children forks this set pushed so we can push it every time we retry.
    let tree_fork_count = if get_is_hydrating() {
        get_forks_at_level(work_in_progress)
    } else {
        0
    };

    if !should_force_fallback {
        let did_suspend_before =
            current.is_some_and(|current| (current.flags.get() & DID_CAPTURE) != NO_FLAGS);
        if did_suspend_before {
            // If we previously forced a fallback, we need to schedule work
            // on any nested boundaries to let them know to try to render
            // again. This is the same as context updating.
            let child = work_in_progress.child.borrow().clone();
            propagate_suspense_context_change(work_in_progress, child, render_lanes);
        }
    }

    if !DISABLE_LEGACY_MODE && (work_in_progress.mode.get() & CONCURRENT_MODE) == NO_MODE {
        // In legacy mode, SuspenseList doesn't work so we just
        // use make it a noop by treating it as the default revealOrder.
        work_in_progress.memoized_state.set(js::null());
    } else {
        match unsafe { js::cast::<Option<&str>>(reveal_order) } {
            Some("backwards") => {
                // We're going to find the first row that has existing content.
                // We are also going to reverse the order of anything in the existing content
                // since we want to actually render them backwards from the reconciled set.
                // The tail is left in order, because it'll be added to the front as we
                // complete each item.
                let child = work_in_progress.child.borrow().clone();
                let last_content_row = find_last_content_row(child);
                let tail;
                match last_content_row {
                    None => {
                        // The whole list is part of the tail.
                        tail = work_in_progress.child.borrow().clone();
                        work_in_progress.child.replace(js::null());
                    }
                    Some(last_content_row) => {
                        // Disconnect the tail rows after the content row.
                        // We're going to render them separately later in reverse order.
                        tail = last_content_row.sibling.borrow().clone();
                        last_content_row.sibling.replace(js::null());
                        // We have to now reverse the main content so it renders backwards too.
                        reverse_children(work_in_progress);
                    }
                }
                // TODO: If workInProgress.child is null, we can continue on the tail immediately.
                init_suspense_list_render_state(
                    work_in_progress,
                    true, // isBackwards
                    tail,
                    js::null(), // last
                    unsafe { js::cast(tail_mode) },
                    tree_fork_count,
                );
            }
            Some("unstable_legacy-backwards") => {
                // We're going to find the first row that has existing content.
                // At the same time we're going to reverse the list of everything
                // we pass in the meantime. That's going to be our tail in reverse
                // order.
                let mut tail = js::null();
                let mut row = work_in_progress.child.borrow().clone();
                work_in_progress.child.replace(js::null());
                while let Some(this_row) = row {
                    let current_row = this_row.alternate.borrow().clone();
                    // New rows can't be content rows.
                    if let Some(current_row) = current_row
                        && find_first_suspended(&current_row).is_none()
                    {
                        // This is the beginning of the main content.
                        work_in_progress.child.replace(Some(this_row));
                        break;
                    }
                    let next_row = this_row.sibling.borrow().clone();
                    this_row.sibling.replace(tail);
                    tail = Some(this_row);
                    row = next_row;
                }
                // TODO: If workInProgress.child is null, we can continue on the tail immediately.
                init_suspense_list_render_state(
                    work_in_progress,
                    true, // isBackwards
                    tail,
                    js::null(), // last
                    unsafe { js::cast(tail_mode) },
                    tree_fork_count,
                );
            }
            Some("together") => {
                init_suspense_list_render_state(
                    work_in_progress,
                    false,      // isBackwards
                    js::null(), // tail
                    js::null(), // last
                    None,
                    tree_fork_count,
                );
            }
            Some("independent") => {
                // The "independent" reveal order is the same as not having
                // a boundary.
                work_in_progress.memoized_state.set(js::null());
            }
            // The default is now forwards.
            _ => {
                let child = work_in_progress.child.borrow().clone();
                let last_content_row = find_last_content_row(child);
                let tail;
                match &last_content_row {
                    None => {
                        // The whole list is part of the tail.
                        // TODO: We could fast path by just rendering the tail now.
                        tail = work_in_progress.child.borrow().clone();
                        work_in_progress.child.replace(js::null());
                    }
                    Some(last_content_row) => {
                        // Disconnect the tail rows after the content row.
                        // We're going to render them separately later.
                        tail = last_content_row.sibling.borrow().clone();
                        last_content_row.sibling.replace(js::null());
                    }
                }
                init_suspense_list_render_state(
                    work_in_progress,
                    false, // isBackwards
                    tail,
                    last_content_row,
                    unsafe { js::cast(tail_mode) },
                    tree_fork_count,
                );
            }
        }
    }
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_view_transition(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if work_in_progress.state_node.get().is_none() {
        // We previously reset the work-in-progress.
        // We need to create a new ViewTransitionState instance.
        let instance = ViewTransitionState {
            auto_name: RefCell::new(js::null()),
            paired: RefCell::new(js::null()),
            clones: RefCell::new(js::null()),
            r#ref: Cell::new(js::null()),
        };
        work_in_progress
            .state_node
            .set(Some(js::unknown_of(Rc::new(instance))));
    }

    let pending_props: &ViewTransitionProps =
        unsafe { js::cast(work_in_progress.pending_props.get()) };
    if let Some(name) = &pending_props.name
        && name != "auto"
    {
        // Explicitly named boundary. We track it so that we can pair it up with another explicit
        // boundary if we get deleted.
        work_in_progress.flags.set(
            work_in_progress.flags.get()
                | if current.is_none() {
                    VIEW_TRANSITION_NAMED_MOUNT | VIEW_TRANSITION_NAMED_STATIC
                } else {
                    VIEW_TRANSITION_NAMED_STATIC
                },
        );
    } else {
        // The server may have used useId to auto-assign a generated name for this boundary.
        // We push a materialization to ensure child ids line up with the server.
        if get_is_hydrating() {
            push_materialized_tree_id(work_in_progress);
        }
    }
    if __DEV__ {
        let class_name = js::get(js::unknown(pending_props), "className");
        if !matches!(js::classify(class_name), Kind::Undefined) {
            let example = if let Kind::String(class_name) = js::classify(class_name) {
                js::json::stringify(class_name)
            } else {
                "{...}".to_string()
            };
            if !DID_WARN_ABOUT_CLASS_NAME_ON_VIEW_TRANSITION
                .with(|did_warn| js::dict::get(did_warn, &example).is_some_and(|&warned| warned))
            {
                DID_WARN_ABOUT_CLASS_NAME_ON_VIEW_TRANSITION
                    .with(|did_warn| js::dict::set(did_warn, &example, true));
                console::error((
                    concat!(
                        "<ViewTransition> doesn't accept a \"className\" prop. It has been renamed to \"default\".\n",
                        "-   <ViewTransition className=%s>\n",
                        "+   <ViewTransition default=%s>"
                    ),
                    &example,
                    &example,
                ));
            }
        }
    }
    if let Some(current) = current
        && unsafe { js::cast::<&ViewTransitionProps>(current.memoized_props.get()) }.name
            != pending_props.name
    {
        // If the name changes, we schedule a ref effect to create a new ref instance.
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | REF | REF_STATIC);
    } else {
        mark_ref(current.map(Rc::as_ref), work_in_progress);
    }
    let next_children = pending_props.children;
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_portal_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let state_node: &PortalStateNode = unsafe { js::cast(work_in_progress.state_node.get()) };
    push_host_container(work_in_progress, unsafe {
        js::cast(state_node.container_info)
    });
    let next_children = work_in_progress.pending_props.get();
    match current {
        None => {
            // Portals are special because we don't append the children during mount
            // but at commit. Therefore we need to track insertions which the normal
            // flow doesn't do during mount. This doesn't happen at the root because
            // the root always starts with a "current" with a null child.
            // TODO: Consider unifying this with how the root works.
            work_in_progress
                .child
                .replace(RECONCILE_CHILD_FIBERS.with(|reconcile_child_fibers| {
                    reconcile_child_fibers(
                        work_in_progress,
                        js::null(),
                        next_children,
                        render_lanes,
                    )
                }));
        }
        Some(current) => {
            reconcile_children(Some(current), work_in_progress, next_children, render_lanes);
        }
    }
    work_in_progress.child.borrow().clone()
}

thread_local! {
    static HAS_WARNED_ABOUT_USING_NO_VALUE_PROP_ON_CONTEXT_PROVIDER: Cell<bool> =
        const { Cell::new(false) };
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_context_provider(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let context: &ReactContext<Option<&'static Unknown>> =
        unsafe { js::cast(work_in_progress.r#type.get()) };
    let new_props = work_in_progress.pending_props.get();
    let new_value = js::get(new_props, "value");

    if __DEV__
        && !js::has(new_props, "value")
        && !HAS_WARNED_ABOUT_USING_NO_VALUE_PROP_ON_CONTEXT_PROVIDER.get()
    {
        HAS_WARNED_ABOUT_USING_NO_VALUE_PROP_ON_CONTEXT_PROVIDER.set(true);
        console::error((
            "The `value` prop is required for the `<Context.Provider>`. Did you misspell it or forget to pass it?",
        ));
    }

    push_provider(work_in_progress, context, new_value);

    let new_children = js::get(new_props, "children");
    reconcile_children(current, work_in_progress, new_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_context_consumer(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // shared/ReactTypes' `ReactConsumerType`.
    let consumer_type = work_in_progress.r#type.get();
    let context: Rc<ReactContext<Option<&'static Unknown>>> =
        unsafe { js::cast(js::get(consumer_type, "_context")) };
    let new_props = work_in_progress.pending_props.get();
    let render = js::get(new_props, "children");

    if __DEV__ && js::type_of(render) != "function" {
        console::error((concat!(
            "A context consumer was rendered with multiple children, or a child ",
            "that isn't a function. A context consumer expects a single child ",
            "that is a function. If you did pass a function, make sure there ",
            "is no trailing or leading whitespace around it."
        ),));
    }

    prepare_to_read_context(work_in_progress, render_lanes);
    let new_value = read_context(&context);
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_started(work_in_progress);
    }
    let new_children = if __DEV__ {
        CALL_COMPONENT_IN_DEV.with(|call_component_in_dev| {
            call_component_in_dev.unwrap()(unsafe { js::cast(render) }, new_value, None)
        })
    } else {
        let render: &dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown> =
            unsafe { js::cast(render) };
        render(new_value)
    };
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_stopped();
    }

    // React DevTools reads this flag.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | PERFORMED_WORK);
    reconcile_children(current, work_in_progress, new_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn update_scope_component(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let next_props = work_in_progress.pending_props.get();
    let next_children = js::get(next_props, "children");
    mark_ref(current.map(Rc::as_ref), work_in_progress);
    reconcile_children(current, work_in_progress, next_children, render_lanes);
    work_in_progress.child.borrow().clone()
}

pub fn mark_work_in_progress_received_update() {
    DID_RECEIVE_UPDATE.set(true);
}

pub fn check_if_work_in_progress_received_update() -> bool {
    DID_RECEIVE_UPDATE.get()
}

fn reset_suspended_current_on_mount_in_legacy_mode(
    current: Option<&Fiber>,
    work_in_progress: &Fiber,
) {
    if !DISABLE_LEGACY_MODE
        && (work_in_progress.mode.get() & CONCURRENT_MODE) == NO_MODE
        && let Some(current) = current
    {
        // A lazy component only mounts if it suspended inside a non-
        // concurrent tree, in an inconsistent state. We want to treat it like
        // a new mount, even though an empty version of it already committed.
        // Disconnect the alternate pointers.
        current.alternate.replace(js::null());
        work_in_progress.alternate.replace(js::null());
        // Since this is conceptually a new fiber, schedule a Placement effect
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | PLACEMENT);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn bailout_on_already_finished_work(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if let Some(current) = current {
        // Reuse previous dependencies
        let dependencies = current.dependencies.borrow().clone();
        work_in_progress.dependencies.replace(dependencies);
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        // Don't update "base" render times for bailouts.
        stop_profiler_timer_if_running(work_in_progress);
    }

    mark_skipped_update_lanes(work_in_progress.lanes.get());

    // Check if the children have any pending work.
    if !includes_some_lane(render_lanes, work_in_progress.child_lanes.get()) {
        // The children don't have any work either. We can skip them.
        // TODO: Once we add back resuming, we should check if the children are
        // a work-in-progress set. If so, we need to transfer their effects.

        let current = current?;
        // Before bailing out, check if there are any context changes in
        // the children.
        lazily_propagate_parent_context_changes(current, work_in_progress, render_lanes);
        if !includes_some_lane(render_lanes, work_in_progress.child_lanes.get()) {
            return None;
        }
    }

    // This fiber doesn't have work, but its subtree does. Clone the child
    // fibers and continue.
    clone_child_fibers(current.map(Rc::as_ref), work_in_progress);
    work_in_progress.child.borrow().clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn remount_fiber(
    current: &Rc<Fiber>,
    old_work_in_progress: &Rc<Fiber>,
    new_work_in_progress: Rc<Fiber>,
) -> Option<Rc<Fiber>> {
    if __DEV__ {
        let return_fiber = old_work_in_progress.r#return.borrow().clone();
        let Some(return_fiber) = return_fiber else {
            // eslint-disable-next-line react-internal/prod-error-codes
            panic!("Cannot swap the root fiber.");
        };

        // Disconnect from the old current.
        // It will get deleted.
        current.alternate.replace(js::null());
        old_work_in_progress.alternate.replace(js::null());

        // Connect to the new tree.
        new_work_in_progress
            .index
            .set(old_work_in_progress.index.get());
        let sibling = old_work_in_progress.sibling.borrow().clone();
        new_work_in_progress.sibling.replace(sibling);
        let old_return = old_work_in_progress.r#return.borrow().clone();
        new_work_in_progress.r#return.replace(old_return);
        new_work_in_progress
            .r#ref
            .set(old_work_in_progress.r#ref.get());

        if __DEV__ {
            new_work_in_progress
                .debug_info
                .set(old_work_in_progress.debug_info.get());
        }

        // Replace the child/sibling pointers above it.
        if return_fiber.child.borrow().as_ref() == Some(old_work_in_progress) {
            return_fiber
                .child
                .replace(Some(Rc::clone(&new_work_in_progress)));
        } else {
            let prev_sibling = return_fiber.child.borrow().clone();
            let Some(mut prev_sibling) = prev_sibling else {
                // eslint-disable-next-line react-internal/prod-error-codes
                panic!("Expected parent to have a child.");
            };
            while prev_sibling.sibling.borrow().as_ref() != Some(old_work_in_progress) {
                let next_sibling = prev_sibling.sibling.borrow().clone();
                let Some(next_sibling) = next_sibling else {
                    // eslint-disable-next-line react-internal/prod-error-codes
                    panic!("Expected to find the previous sibling.");
                };
                prev_sibling = next_sibling;
            }
            prev_sibling
                .sibling
                .replace(Some(Rc::clone(&new_work_in_progress)));
        }

        // Delete the old fiber and place the new one.
        // Since the old fiber is disconnected, we have to schedule it manually.
        let deletions = return_fiber.deletions.borrow().clone();
        match deletions {
            None => {
                return_fiber
                    .deletions
                    .replace(Some(Rc::new(RefCell::new(vec![Rc::clone(current)]))));
                return_fiber
                    .flags
                    .set(return_fiber.flags.get() | CHILD_DELETION);
            }
            Some(deletions) => deletions.borrow_mut().push(Rc::clone(current)),
        }

        new_work_in_progress
            .flags
            .set(new_work_in_progress.flags.get() | PLACEMENT | PLACEMENT_DEV);

        // Restart work from the new fiber.
        Some(new_work_in_progress)
    } else {
        panic!(concat!(
            "Did not expect this call in production. ",
            "This is a bug in React. Please file an issue."
        ));
    }
}

fn check_scheduled_update_or_context(current: &Fiber, render_lanes: Lanes) -> bool {
    // Before performing an early bailout, we must check if there are pending
    // updates or context.
    let update_lanes = current.lanes.get();
    if includes_some_lane(update_lanes, render_lanes) {
        return true;
    }
    // No pending update, but because context is propagated lazily, we need
    // to check for a context change before we bail out.
    let dependencies = current.dependencies.borrow().clone();
    if let Some(dependencies) = dependencies
        && check_if_context_changed(&dependencies)
    {
        return true;
    }
    false
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn attempt_early_bailout_if_no_scheduled_update(
    current: &Rc<Fiber>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // This fiber does not have any pending work. Bailout without entering
    // the begin phase. There's still some bookkeeping we that needs to be done
    // in this optimized path, mostly pushing stuff onto the stack.
    match work_in_progress.tag.get() {
        HOST_ROOT => {
            push_host_root_context(work_in_progress);
            let root: &FiberRoot = unsafe { js::cast(work_in_progress.state_node.get()) };
            push_root_transition(work_in_progress, root, render_lanes);

            if ENABLE_TRANSITION_TRACING {
                push_root_marker_instance(work_in_progress);
            }

            let cache: &Rc<Cache> =
                &unsafe { js::cast::<&RootState>(current.memoized_state.get()) }.cache;
            push_cache_provider(work_in_progress, Rc::clone(cache));
            reset_hydration_state();
        }
        HOST_SINGLETON | HOST_COMPONENT => {
            push_host_context(work_in_progress);
        }
        CLASS_COMPONENT => {
            let component = work_in_progress.r#type.get();
            if is_legacy_context_provider(component) {
                push_legacy_context_provider(work_in_progress);
            }
        }
        HOST_PORTAL => {
            let state_node: &PortalStateNode =
                unsafe { js::cast(work_in_progress.state_node.get()) };
            push_host_container(work_in_progress, unsafe {
                js::cast(state_node.container_info)
            });
        }
        CONTEXT_PROVIDER => {
            let new_value = js::get(work_in_progress.memoized_props.get(), "value");
            let context: &ReactContext<Option<&'static Unknown>> =
                unsafe { js::cast(work_in_progress.r#type.get()) };
            push_provider(work_in_progress, context, new_value);
        }
        PROFILER => {
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                // Profiler should only call onRender when one of its descendants actually rendered.
                let has_child_work =
                    includes_some_lane(render_lanes, work_in_progress.child_lanes.get());
                if has_child_work {
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | UPDATE);
                }

                if ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled) {
                    // Schedule a passive effect for this Profiler to call onPostCommit hooks.
                    // This effect should be scheduled even if there is no onPostCommit callback for this Profiler,
                    // because the effect is also where times bubble to parent Profilers.
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | PASSIVE);
                    // Reset effect durations for the next eventual effect phase.
                    // These are reset during render to allow the DevTools commit hook a chance to read them,
                    let state_node: &ProfilerStateNode =
                        unsafe { js::cast(work_in_progress.state_node.get()) };
                    state_node.effect_duration.set(-0.0);
                    state_node.passive_effect_duration.set(-0.0);
                }
            }
        }
        ACTIVITY_COMPONENT => {
            let state: Option<&ActivityState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };
            if state.is_some() {
                // We're dehydrated so we're not going to render the children. This is just
                // to maintain push/pop symmetry.
                // We know that this component will suspend again because if it has
                // been unsuspended it has committed as a hydrated Activity component.
                // If it needs to be retried, it should have work scheduled on it.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | DID_CAPTURE);
                push_dehydrated_activity_suspense_handler(work_in_progress);
                return None;
            }
        }
        SUSPENSE_COMPONENT => {
            let state: Option<&SuspenseState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };
            if let Some(state) = state {
                if state.dehydrated.is_some() {
                    // We're not going to render the children, so this is just to maintain
                    // push/pop symmetry
                    push_primary_tree_suspense_handler(work_in_progress);
                    // We know that this component will suspend again because if it has
                    // been unsuspended it has committed as a resolved Suspense component.
                    // If it needs to be retried, it should have work scheduled on it.
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | DID_CAPTURE);
                    // We should never render the children of a dehydrated boundary until we
                    // upgrade it. We return null instead of bailoutOnAlreadyFinishedWork.
                    return None;
                }

                // If this boundary is currently timed out, we need to decide
                // whether to retry the primary children, or to skip over it and
                // go straight to the fallback. Check the priority of the primary
                // child fragment.
                //
                // Propagate context changes first. If a parent context changed
                // and the primary children's consumer fibers were discarded
                // during initial mount suspension, normal propagation can't find
                // them. In that case we conservatively retry the boundary — the
                // re-mounted children will read the updated context value.
                let context_changed = lazily_propagate_parent_context_changes(
                    current,
                    work_in_progress,
                    render_lanes,
                );
                let primary_child_fragment = work_in_progress.child.borrow().clone().unwrap();
                let primary_child_lanes = primary_child_fragment.child_lanes.get();
                if context_changed || includes_some_lane(render_lanes, primary_child_lanes) {
                    // The primary children have pending work. Use the normal path
                    // to attempt to render the primary children again.
                    return update_suspense_component(
                        Some(current),
                        work_in_progress,
                        render_lanes,
                    );
                } else {
                    // The primary child fragment does not have pending work marked
                    // on it
                    push_primary_tree_suspense_handler(work_in_progress);
                    // The primary children do not have pending work with sufficient
                    // priority. Bailout.
                    let child = bailout_on_already_finished_work(
                        Some(current),
                        work_in_progress,
                        render_lanes,
                    )?;
                    // Note: We can return `null` here because we already checked
                    // whether there were nested context consumers, via the call to
                    // `bailoutOnAlreadyFinishedWork` above.
                    // The fallback children have pending work. Skip over the
                    // primary children and work on the fallback.
                    return child.sibling.borrow().clone();
                }
            }
            push_primary_tree_suspense_handler(work_in_progress);
        }
        SUSPENSE_LIST_COMPONENT => {
            if (work_in_progress.flags.get() & DID_CAPTURE) != 0 {
                // Second pass caught.
                return update_suspense_list_component(
                    Some(current),
                    work_in_progress,
                    render_lanes,
                );
            }
            let did_suspend_before = (current.flags.get() & DID_CAPTURE) != NO_FLAGS;

            let mut has_child_work =
                includes_some_lane(render_lanes, work_in_progress.child_lanes.get());

            if !has_child_work {
                // Context changes may not have been propagated yet. We need to do
                // that now, before we can decide whether to bail out.
                // TODO: We use `childLanes` as a heuristic for whether there is
                // remaining work in a few places, including
                // `bailoutOnAlreadyFinishedWork` and
                // `updateDehydratedSuspenseComponent`. We should maybe extract this
                // into a dedicated function.
                lazily_propagate_parent_context_changes(current, work_in_progress, render_lanes);
                has_child_work =
                    includes_some_lane(render_lanes, work_in_progress.child_lanes.get());
            }

            if did_suspend_before {
                if has_child_work {
                    // If something was in fallback state last time, and we have all the
                    // same children then we're still in progressive loading state.
                    // Something might get unblocked by state updates or retries in the
                    // tree which will affect the tail. So we need to use the normal
                    // path to compute the correct tail.
                    return update_suspense_list_component(
                        Some(current),
                        work_in_progress,
                        render_lanes,
                    );
                }
                // If none of the children had any work, that means that none of
                // them got retried so they'll still be blocked in the same way
                // as before. We can fast bail out.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | DID_CAPTURE);
            }

            // If nothing suspended before and we're rendering the same children,
            // then the tail doesn't matter. Anything new that suspends will work
            // in the "together" mode, so we can continue from the state we had.
            let render_state: Option<&SuspenseListRenderState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };
            if let Some(render_state) = render_state {
                // Reset to the "together" mode in case we've started a different
                // update in the past but didn't complete it.
                render_state.rendering.replace(js::null());
                render_state.tail.replace(js::null());
                js::set(
                    js::unknown(render_state),
                    "lastEffect",
                    js::null::<&Unknown>(),
                );
            }
            push_suspense_list_context(
                work_in_progress,
                SUSPENSE_STACK_CURSOR.with(|cursor| *cursor.current.borrow()),
            );

            if !has_child_work {
                // If none of the children had any work, that means that none of
                // them got retried so they'll still be blocked in the same way
                // as before. We can fast bail out.
                return None;
            }
        }
        OFFSCREEN_COMPONENT => {
            // Need to check if the tree still needs to be deferred. This is
            // almost identical to the logic used in the normal update path,
            // so we'll just enter that. The only difference is we'll bail out
            // at the next level instead of this one, because the child props
            // have not changed. Which is fine.
            // TODO: Probably should refactor `beginWork` to split the bailout
            // path from the normal path. I'm tempted to do a labeled break here
            // but I won't :)
            work_in_progress.lanes.set(NO_LANES);
            return update_offscreen_component(
                Some(current),
                work_in_progress,
                render_lanes,
                unsafe { js::cast(work_in_progress.pending_props.get()) },
            );
        }
        CACHE_COMPONENT => {
            let cache: &Rc<Cache> =
                &unsafe { js::cast::<&CacheComponentState>(current.memoized_state.get()) }.cache;
            push_cache_provider(work_in_progress, Rc::clone(cache));
        }
        TRACING_MARKER_COMPONENT if ENABLE_TRANSITION_TRACING => {
            let instance: Option<Rc<TracingMarkerInstance>> =
                unsafe { js::cast(work_in_progress.state_node.get()) };
            if let Some(instance) = instance {
                push_marker_instance(work_in_progress, instance);
            }
        }
        // Fallthrough
        TRACING_MARKER_COMPONENT | LEGACY_HIDDEN_COMPONENT if ENABLE_LEGACY_HIDDEN => {
            work_in_progress.lanes.set(NO_LANES);
            return update_legacy_hidden_component(Some(current), work_in_progress, render_lanes);
        }
        // Fallthrough
        _ => {}
    }
    bailout_on_already_finished_work(Some(current), work_in_progress, render_lanes)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn begin_work(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    if __DEV__
        && work_in_progress.debug_needs_remount.get() == Some(true)
        && let Some(current) = current
    {
        // This will restart the begin phase with a new fiber.
        let key = work_in_progress.key.borrow().clone();
        let copied_fiber = create_fiber_from_type_and_props(
            // Remount from the fiber's outermost identity; mounting resolves
            // any inner types to their latest implementations.
            resolve_type_for_hot_reloading(work_in_progress.element_type.get()),
            key,
            work_in_progress.pending_props.get(),
            work_in_progress.debug_owner.get().or(js::null()),
            work_in_progress.mode.get(),
            work_in_progress.lanes.get(),
        );
        copied_fiber
            .debug_stack
            .set(work_in_progress.debug_stack.get());
        copied_fiber
            .debug_task
            .set(work_in_progress.debug_task.get());
        return remount_fiber(current, work_in_progress, copied_fiber);
    }

    match current {
        Some(current) => {
            let old_props = current.memoized_props.get();
            let new_props = work_in_progress.pending_props.get();

            if !js::strict_equals(old_props, new_props)
                || has_legacy_context_changed()
                // Force a re-render if the implementation changed due to hot reload:
                || (if __DEV__ {
                    !js::strict_equals(work_in_progress.r#type.get(), current.r#type.get())
                } else {
                    false
                })
            {
                // If props or context changed, mark the fiber as having performed work.
                // This may be unset if the props are determined to be equal later (memo).
                DID_RECEIVE_UPDATE.set(true);
            } else {
                // Neither props nor legacy context changes. Check if there's a pending
                // update or context change.
                let has_scheduled_update_or_context =
                    check_scheduled_update_or_context(current, render_lanes);
                if !has_scheduled_update_or_context
                    // If this is the second pass of an error or suspense boundary, there
                    // may not be work scheduled on `current`, so we check for this flag.
                    && (work_in_progress.flags.get() & DID_CAPTURE) == NO_FLAGS
                {
                    // No pending updates or context. Bail out now.
                    DID_RECEIVE_UPDATE.set(false);
                    return attempt_early_bailout_if_no_scheduled_update(
                        current,
                        work_in_progress,
                        render_lanes,
                    );
                }
                if (current.flags.get() & FORCE_UPDATE_FOR_LEGACY_SUSPENSE) != NO_FLAGS {
                    // This is a special case that only exists for legacy mode.
                    // See https://github.com/react/react/pull/19216.
                    DID_RECEIVE_UPDATE.set(true);
                } else {
                    // An update was scheduled on this fiber, but there are no new props
                    // nor legacy context. Set this to false. If an update queue or context
                    // consumer produces a changed value, it will set this to true. Otherwise,
                    // the component will assume the children have not changed and bail out.
                    DID_RECEIVE_UPDATE.set(false);
                }
            }
        }
        None => {
            DID_RECEIVE_UPDATE.set(false);

            if get_is_hydrating() && is_forked_child(work_in_progress) {
                // Check if this child belongs to a list of muliple children in
                // its parent.
                //
                // In a true multi-threaded implementation, we would render children on
                // parallel threads. This would represent the beginning of a new render
                // thread for this subtree.
                //
                // We only use this for id generation during hydration, which is why the
                // logic is located in this special branch.
                let slot_index = work_in_progress.index.get();
                let number_of_forks = get_forks_at_level(work_in_progress);
                push_tree_id(work_in_progress, number_of_forks, slot_index as i32);
            }
        }
    }

    // Before entering the begin phase, clear pending update priority.
    // TODO: This assumes that we're about to evaluate the component and process
    // the update queue. However, there's an exception: SimpleMemoComponent
    // sometimes bails out later in the begin phase. This indicates that we should
    // move this assignment out of the common path and into each branch.
    work_in_progress.lanes.set(NO_LANES);

    match work_in_progress.tag.get() {
        LAZY_COMPONENT => {
            let element_type = work_in_progress.element_type.get();
            return mount_lazy_component(current, work_in_progress, element_type, render_lanes);
        }
        FUNCTION_COMPONENT => {
            let component = work_in_progress.r#type.get();
            return update_function_component(
                current,
                work_in_progress,
                component,
                work_in_progress.pending_props.get(),
                render_lanes,
            );
        }
        CLASS_COMPONENT => {
            let component = work_in_progress.r#type.get();
            let unresolved_props = work_in_progress.pending_props.get();
            let resolved_props =
                resolve_class_component_props(component, unsafe { js::cast(unresolved_props) });
            return update_class_component(
                current,
                work_in_progress,
                unsafe { js::cast(component) },
                Some(resolved_props),
                render_lanes,
            );
        }
        HOST_ROOT => return update_host_root(current, work_in_progress, render_lanes),
        HOST_HOISTABLE if SUPPORTS_RESOURCES => {
            return update_host_hoistable(current, work_in_progress, render_lanes);
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON if SUPPORTS_SINGLETONS => {
            return update_host_singleton(current, work_in_progress, render_lanes);
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            return update_host_component(current, work_in_progress, render_lanes);
        }
        HOST_TEXT => return update_host_text(current, work_in_progress),
        SUSPENSE_COMPONENT => {
            return update_suspense_component(current, work_in_progress, render_lanes);
        }
        HOST_PORTAL => return update_portal_component(current, work_in_progress, render_lanes),
        FORWARD_REF => {
            return update_forward_ref(
                current,
                work_in_progress,
                work_in_progress.r#type.get(),
                work_in_progress.pending_props.get(),
                render_lanes,
            );
        }
        FRAGMENT => return update_fragment(current, work_in_progress, render_lanes),
        MODE => return update_mode(current, work_in_progress, render_lanes),
        PROFILER => return update_profiler(current, work_in_progress, render_lanes),
        CONTEXT_PROVIDER => {
            return update_context_provider(current, work_in_progress, render_lanes);
        }
        CONTEXT_CONSUMER => {
            return update_context_consumer(current, work_in_progress, render_lanes);
        }
        MEMO_COMPONENT => {
            return update_memo_component(
                current,
                work_in_progress,
                work_in_progress.r#type.get(),
                work_in_progress.pending_props.get(),
                render_lanes,
            );
        }
        SIMPLE_MEMO_COMPONENT => {
            return update_simple_memo_component(
                current,
                work_in_progress,
                work_in_progress.r#type.get(),
                work_in_progress.pending_props.get(),
                render_lanes,
            );
        }
        INCOMPLETE_CLASS_COMPONENT if DISABLE_LEGACY_MODE => {}
        INCOMPLETE_CLASS_COMPONENT => {
            let component = work_in_progress.r#type.get();
            let unresolved_props = work_in_progress.pending_props.get();
            let resolved_props =
                resolve_class_component_props(component, unsafe { js::cast(unresolved_props) });
            return mount_incomplete_class_component(
                current,
                work_in_progress,
                unsafe { js::cast(component) },
                Some(resolved_props),
                render_lanes,
            );
        }
        INCOMPLETE_FUNCTION_COMPONENT if DISABLE_LEGACY_MODE => {}
        INCOMPLETE_FUNCTION_COMPONENT => {
            let component = work_in_progress.r#type.get();
            let unresolved_props = work_in_progress.pending_props.get();
            let resolved_props =
                resolve_class_component_props(component, unsafe { js::cast(unresolved_props) });
            return mount_incomplete_function_component(
                current,
                work_in_progress,
                component,
                Some(resolved_props),
                render_lanes,
            );
        }
        SUSPENSE_LIST_COMPONENT => {
            return update_suspense_list_component(current, work_in_progress, render_lanes);
        }
        SCOPE_COMPONENT if ENABLE_SCOPE_API => {
            return update_scope_component(current, work_in_progress, render_lanes);
        }
        ACTIVITY_COMPONENT => {
            return update_activity_component(current, work_in_progress, render_lanes);
        }
        OFFSCREEN_COMPONENT => {
            return update_offscreen_component(current, work_in_progress, render_lanes, unsafe {
                js::cast(work_in_progress.pending_props.get())
            });
        }
        LEGACY_HIDDEN_COMPONENT if ENABLE_LEGACY_HIDDEN => {
            return update_legacy_hidden_component(current, work_in_progress, render_lanes);
        }
        CACHE_COMPONENT => {
            return update_cache_component(current, work_in_progress, render_lanes);
        }
        TRACING_MARKER_COMPONENT if ENABLE_TRANSITION_TRACING => {
            return update_tracing_marker_component(current, work_in_progress, render_lanes);
        }
        VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
            return update_view_transition(current, work_in_progress, render_lanes);
        }
        THROW => {
            // This represents a Component that threw in the reconciliation phase.
            // So we'll rethrow here. This might be a Thenable.
            js::throw(work_in_progress.pending_props.get());
        }
        _ => {}
    }

    panic!(
        "Unknown unit of work tag ({}). This error is likely caused by a bug in React. Please file an issue.",
        work_in_progress.tag.get()
    );
}
