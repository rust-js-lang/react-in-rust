// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use js::{Dict, Kind, Unknown};
use shared::environment::__DEV__;
use shared::react_feature_flags::{DISABLE_LEGACY_MODE, ENABLE_SCHEDULING_PROFILER};
use shared::react_instance_map::get as get_instance;
use shared::react_shared_internals::{REACT_SHARED_INTERNALS, SharedStateClient};
#[cfg_attr(rust_js, rust_js::name = "ReactVersion")]
use shared::react_version::REACT_VERSION;
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
#[cfg_attr(rust_js, rust_js::name = "ReactCurrentFiberCurrent")]
use crate::react_current_fiber::CURRENT as REACT_CURRENT_FIBER_CURRENT;
#[cfg_attr(rust_js, rust_js::name = "ReactCurrentFiberIsRendering")]
use crate::react_current_fiber::IS_RENDERING as REACT_CURRENT_FIBER_IS_RENDERING;
use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_fiber_async_action::register_default_indicator;
use crate::react_fiber_class_update_queue::{create_update, enqueue_update, entangle_transitions};
use crate::react_fiber_concurrent_updates::enqueue_concurrent_render_for_lane;
use crate::react_fiber_dev_tools_hook::{
    inject_internals, inject_profiling_hooks, mark_render_scheduled, on_schedule_root,
};
use crate::react_fiber_hooks::Hook;
use crate::react_fiber_hot_reloading::{
    ScheduleRefresh, ScheduleRoot, SetRefreshHandler, schedule_refresh, schedule_root,
    set_refresh_handler,
};
use crate::react_fiber_lane::{
    Lane, SELECTIVE_HYDRATION_LANE, SYNC_LANE, TOTAL_LANES, claim_next_retry_lane,
    get_bumped_lane_for_hydration_by_lane, get_highest_priority_pending_lanes, get_label_for_lane,
    higher_priority_lane,
};
use crate::react_fiber_legacy_context::{
    Context, EMPTY_CONTEXT_OBJECT, find_current_unmasked_context,
    is_context_provider as is_legacy_context_provider, process_child_context,
};
use crate::react_fiber_root::create_fiber_root;
use crate::react_fiber_shell_hydration::is_root_dehydrated;
use crate::react_fiber_suspense_component::SuspenseState;
use crate::react_fiber_tree_reflection::{
    find_current_host_fiber, find_current_host_fiber_with_no_portals,
};
use crate::react_fiber_work_loop::{
    flush_pending_effects, flush_root, request_update_lane, schedule_initial_hydration_on_root,
    schedule_update_on_fiber,
};
use crate::react_internal_types::{
    Fiber, FiberRoot, OnCaughtError, OnDefaultTransitionIndicator, OnError,
    SuspenseHydrationCallbacks, TransitionTracingCallbacks,
};
use crate::react_profiler_timer::start_update_timer_by_lane;
use crate::react_root_tags::{LEGACY_ROOT, RootTag};
use crate::react_type_of_mode::STRICT_LEGACY_MODE;
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CLASS_COMPONENT, HOST_COMPONENT, HOST_ROOT, HOST_SINGLETON,
    SUSPENSE_COMPONENT,
};

pub use crate::react_portal::create_portal;

pub use crate::react_test_selectors::{
    create_component_selector, create_has_pseudo_class_selector, create_role_selector,
    create_test_name_selector, create_text_selector, find_all_nodes, find_bounding_rects,
    focus_within, get_find_all_nodes_failure_description, observe_visible_rects,
};

pub use crate::react_fiber_hooks::start_host_transition;

pub use crate::react_fiber_error_logger::{
    default_on_caught_error, default_on_recoverable_error, default_on_uncaught_error,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#rendererVersion"]
    safe static RENDERER_VERSION: &'static str;
    #[link_name = "./src/ReactFiberConfig.js#rendererPackageName"]
    safe static RENDERER_PACKAGE_NAME: &'static str;
    // ReactFiberConfig's `RendererInspectionConfig`.
    #[link_name = "./src/ReactFiberConfig.js#extraDevToolsConfig"]
    safe static EXTRA_DEV_TOOLS_CONFIG: Option<&'static Unknown>;
}

// ReactFiberConfig's `Instance`'s public instance.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getPublicInstance",
    rust_js::nullable
)]
fn get_public_instance(instance: Option<&'static Unknown>) -> Option<&'static Unknown> {
    unreachable!()
}

type OpaqueRoot = FiberRoot;

thread_local! {
    static DID_WARN_ABOUT_NESTED_UPDATES: Cell<Option<bool>> = const { Cell::new(None) };
    static DID_WARN_ABOUT_FIND_NODE_IN_STRICT_MODE: Cell<Option<&'static Dict<bool>>> =
        const { Cell::new(None) };
}

js::on_load! {
    if __DEV__ {
        DID_WARN_ABOUT_NESTED_UPDATES.set(Some(false));
        DID_WARN_ABOUT_FIND_NODE_IN_STRICT_MODE.set(Some(js::dict::new()));
    }
}

// A class component, if any.
#[cfg_attr(rust_js, rust_js::nullable(parent_component))]
fn get_context_for_subtree(parent_component: Option<&'static Unknown>) -> &'static Context {
    let Some(parent_component) = parent_component else {
        return EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object);
    };

    let fiber: Rc<Fiber> = unsafe { js::cast(get_instance(parent_component)) };
    let parent_context = find_current_unmasked_context(&fiber);

    if fiber.tag.get() == CLASS_COMPONENT {
        #[cfg_attr(rust_js, rust_js::name = "Component")]
        let component = fiber.r#type.get();
        if is_legacy_context_provider(component) {
            return process_child_context(&fiber, unsafe { js::cast(component) }, parent_context);
        }
    }

    parent_context
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_host_instance(component: &'static Unknown) -> Option<&'static Unknown> {
    let fiber: Option<Rc<Fiber>> = unsafe { js::cast(get_instance(component)) };
    let Some(fiber) = fiber else {
        if js::type_of(js::get(component, "render")) == "function" {
            panic!("Unable to find node on an unmounted component.");
        } else {
            let keys = js::object::keys(component).join(",");
            panic!("Argument appears to not be a ReactComponent. Keys: {keys}");
        }
    };
    let host_fiber = find_current_host_fiber(&fiber);
    let host_fiber = host_fiber?;
    get_public_instance(host_fiber.state_node.get())
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_host_instance_with_warning(
    component: &'static Unknown,
    method_name: &str,
) -> Option<&'static Unknown> {
    if __DEV__ {
        let fiber: Option<Rc<Fiber>> = unsafe { js::cast(get_instance(component)) };
        let Some(fiber) = fiber else {
            if js::type_of(js::get(component, "render")) == "function" {
                panic!("Unable to find node on an unmounted component.");
            } else {
                let keys = js::object::keys(component).join(",");
                panic!("Argument appears to not be a ReactComponent. Keys: {keys}");
            }
        };
        let host_fiber = find_current_host_fiber(&fiber);
        let host_fiber = host_fiber?;
        if (host_fiber.mode.get() & STRICT_LEGACY_MODE) != 0 {
            let component_name = get_component_name_from_fiber(&fiber)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Component".to_string());
            let did_warn_about_find_node_in_strict_mode =
                DID_WARN_ABOUT_FIND_NODE_IN_STRICT_MODE.get().unwrap();
            if !js::dict::get(did_warn_about_find_node_in_strict_mode, &component_name)
                .is_some_and(|&did_warn| did_warn)
            {
                js::dict::set(
                    did_warn_about_find_node_in_strict_mode,
                    &component_name,
                    true,
                );
                run_with_fiber_in_dev(
                    Some(Rc::clone(&host_fiber)),
                    |fiber: &Fiber, method_name: &str, component_name: &str, _: (), _: ()| {
                        if (fiber.mode.get() & STRICT_LEGACY_MODE) != 0 {
                            console::error((
                                concat!(
                                    "%s is deprecated in StrictMode. ",
                                    "%s was passed an instance of %s which is inside StrictMode. ",
                                    "Instead, add a ref directly to the element you want to reference. ",
                                    "Learn more about using refs safely here: ",
                                    "https://react.dev/link/strict-mode-find-node"
                                ),
                                method_name,
                                method_name,
                                component_name,
                            ));
                        } else {
                            console::error((
                                concat!(
                                    "%s is deprecated in StrictMode. ",
                                    "%s was passed an instance of %s which renders StrictMode children. ",
                                    "Instead, add a ref directly to the element you want to reference. ",
                                    "Learn more about using refs safely here: ",
                                    "https://react.dev/link/strict-mode-find-node"
                                ),
                                method_name,
                                method_name,
                                component_name,
                            ));
                        }
                    },
                    &*fiber,
                    method_name,
                    component_name.as_str(),
                    (),
                    (),
                );
            }
        }
        return get_public_instance(host_fiber.state_node.get());
    }
    find_host_instance(component)
}

#[cfg_attr(rust_js, rust_js::nullable(hydration_callbacks))]
#[cfg_attr(rust_js, rust_js::nullable(transition_callbacks))]
pub fn create_container(
    // ReactFiberConfig's `Container`.
    container_info: &'static Unknown,
    tag: RootTag,
    hydration_callbacks: Option<&'static SuspenseHydrationCallbacks>,
    is_strict_mode: bool,
    // TODO: Remove `concurrentUpdatesByDefaultOverride`. It is now ignored.
    _concurrent_updates_by_default_override: Option<bool>,
    identifier_prefix: String,
    on_uncaught_error: &'static OnError,
    on_caught_error: &'static OnCaughtError,
    on_recoverable_error: &'static OnError,
    on_default_transition_indicator: &'static OnDefaultTransitionIndicator,
    transition_callbacks: Option<&'static TransitionTracingCallbacks>,
) -> Rc<OpaqueRoot> {
    let hydrate = false;
    let initial_children = js::null();
    let root = create_fiber_root(
        container_info,
        tag,
        hydrate,
        initial_children,
        hydration_callbacks,
        is_strict_mode,
        identifier_prefix,
        js::null(),
        on_uncaught_error,
        on_caught_error,
        on_recoverable_error,
        on_default_transition_indicator,
        transition_callbacks,
    );
    register_default_indicator(on_default_transition_indicator);
    root
}

#[cfg_attr(rust_js, rust_js::nullable(hydration_callbacks))]
#[cfg_attr(rust_js, rust_js::nullable(transition_callbacks, form_state))]
pub fn create_hydration_container(
    // shared/ReactTypes' `ReactNodeList`.
    initial_children: Option<&'static Unknown>,
    // TODO: Remove `callback` when we delete legacy mode.
    callback: Option<&'static Unknown>,
    // ReactFiberConfig's `Container`.
    container_info: &'static Unknown,
    tag: RootTag,
    hydration_callbacks: Option<&'static SuspenseHydrationCallbacks>,
    is_strict_mode: bool,
    // TODO: Remove `concurrentUpdatesByDefaultOverride`. It is now ignored.
    _concurrent_updates_by_default_override: Option<bool>,
    identifier_prefix: String,
    on_uncaught_error: &'static OnError,
    on_caught_error: &'static OnCaughtError,
    on_recoverable_error: &'static OnError,
    on_default_transition_indicator: &'static OnDefaultTransitionIndicator,
    transition_callbacks: Option<&'static TransitionTracingCallbacks>,
    // shared/ReactTypes' `ReactFormState<any, any>`.
    form_state: Option<&'static Unknown>,
) -> Rc<OpaqueRoot> {
    let hydrate = true;
    let root = create_fiber_root(
        container_info,
        tag,
        hydrate,
        initial_children,
        hydration_callbacks,
        is_strict_mode,
        identifier_prefix,
        form_state,
        on_uncaught_error,
        on_caught_error,
        on_recoverable_error,
        on_default_transition_indicator,
        transition_callbacks,
    );

    register_default_indicator(on_default_transition_indicator);

    // TODO: Move this to FiberRoot constructor
    root.context.set(Some(unsafe {
        js::cast(Some(js::unknown(get_context_for_subtree(js::null()))))
    }));

    // Schedule the initial render. In a hydration root, this is different from
    // a regular update because the initial render must match was was rendered
    // on the server.
    // NOTE: This update intentionally doesn't have a payload. We're only using
    // the update to schedule work on the root fiber (and, for legacy roots, to
    // enqueue the callback if one is provided).
    let current = root.current.borrow().clone();
    let mut lane = request_update_lane(&current);
    lane = get_bumped_lane_for_hydration_by_lane(lane);
    let update = create_update(lane);
    update.callback.set(callback);
    enqueue_update(&current, &update, lane);
    start_update_timer_by_lane(lane, "hydrateRoot()", None);
    schedule_initial_hydration_on_root(&root, lane);

    root
}

#[cfg_attr(rust_js, rust_js::nullable(parent_component, callback))]
pub fn update_container(
    // shared/ReactTypes' `ReactNodeList`.
    element: Option<&'static Unknown>,
    container: &OpaqueRoot,
    // A class component, if any.
    parent_component: Option<&'static Unknown>,
    callback: Option<&'static Unknown>,
) -> Lane {
    let current = container.current.borrow().clone();
    let lane = request_update_lane(&current);
    update_container_impl(
        &current,
        lane,
        element,
        container,
        parent_component,
        callback,
    );
    lane
}

#[cfg_attr(rust_js, rust_js::nullable(parent_component, callback))]
pub fn update_container_sync(
    // shared/ReactTypes' `ReactNodeList`.
    element: Option<&'static Unknown>,
    container: &OpaqueRoot,
    // A class component, if any.
    parent_component: Option<&'static Unknown>,
    callback: Option<&'static Unknown>,
) -> Lane {
    if !DISABLE_LEGACY_MODE && container.tag == LEGACY_ROOT {
        flush_pending_effects();
    }
    let current = container.current.borrow().clone();
    update_container_impl(
        &current,
        SYNC_LANE,
        element,
        container,
        parent_component,
        callback,
    );
    SYNC_LANE
}

/// The payload of a root's update: the element it renders.
struct RootUpdatePayload {
    // Caution: React DevTools currently depends on this property
    // being called "element".
    element: Option<&'static Unknown>,
}

unsafe impl js::Defined for RootUpdatePayload {}

#[cfg_attr(rust_js, rust_js::nullable(parent_component, callback))]
fn update_container_impl(
    root_fiber: &Rc<Fiber>,
    lane: Lane,
    // shared/ReactTypes' `ReactNodeList`.
    element: Option<&'static Unknown>,
    container: &OpaqueRoot,
    // A class component, if any.
    parent_component: Option<&'static Unknown>,
    callback: Option<&'static Unknown>,
) {
    if __DEV__ {
        on_schedule_root(container, element);
    }

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_render_scheduled(lane);
    }

    let context: &'static Context = get_context_for_subtree(parent_component);
    if container.context.get().is_none() {
        container
            .context
            .set(Some(unsafe { js::cast(Some(js::unknown(context))) }));
    } else {
        container
            .pending_context
            .set(Some(unsafe { js::cast(Some(js::unknown(context))) }));
    }

    if __DEV__
        && REACT_CURRENT_FIBER_IS_RENDERING.get()
        && let Some(react_current_fiber_current) =
            REACT_CURRENT_FIBER_CURRENT.with_borrow(Clone::clone)
        && !DID_WARN_ABOUT_NESTED_UPDATES.get().unwrap_or_default()
    {
        DID_WARN_ABOUT_NESTED_UPDATES.set(Some(true));
        console::error((
            concat!(
                "Render methods should be a pure function of props and state; ",
                "triggering nested component updates from render is not allowed. ",
                "If necessary, trigger nested updates in componentDidUpdate.\n\n",
                "Check the render method of %s."
            ),
            get_component_name_from_fiber(&react_current_fiber_current)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Unknown".to_string()),
        ));
    }

    let update = create_update(lane);
    // Caution: React DevTools currently depends on this property
    // being called "element".
    update
        .payload
        .set(Some(js::unknown_of(RootUpdatePayload { element })));

    if let Some(callback) = callback {
        if __DEV__ && js::type_of(Some(callback)) != "function" {
            console::error((
                concat!(
                    "Expected the last optional `callback` argument to be a ",
                    "function. Instead received: %s."
                ),
                callback,
            ));
        }
        update.callback.set(Some(callback));
    }

    let root = enqueue_update(root_fiber, &update, lane);
    if let Some(root) = root {
        start_update_timer_by_lane(lane, "root.render()", None);
        schedule_update_on_fiber(&root, root_fiber, lane);
        entangle_transitions(&root, root_fiber, lane);
    }
}

pub use crate::react_fiber_work_loop::{
    batched_updates, deferred_updates, discrete_updates,
    flush_pending_effects as flush_passive_effects, flush_sync_from_reconciler, flush_sync_work,
    is_already_rendering,
};

// A class component's instance, or ReactFiberConfig's `PublicInstance`, if any.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_public_root_instance(container: &OpaqueRoot) -> Option<&'static Unknown> {
    let container_fiber = container.current.borrow().clone();
    let child = container_fiber.child.borrow().clone()?;
    match child.tag.get() {
        HOST_SINGLETON | HOST_COMPONENT => get_public_instance(child.state_node.get()),
        _ => child.state_node.get(),
    }
}

pub fn attempt_synchronous_hydration(fiber: &Rc<Fiber>) {
    match fiber.tag.get() {
        HOST_ROOT => {
            let root: Rc<FiberRoot> = unsafe { js::cast(fiber.state_node.get()) };
            if is_root_dehydrated(&root) {
                // Flush the first scheduled "update".
                let lanes = get_highest_priority_pending_lanes(&root);
                flush_root(&root, lanes);
            }
        }
        ACTIVITY_COMPONENT | SUSPENSE_COMPONENT => {
            let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
            if let Some(root) = root {
                schedule_update_on_fiber(&root, fiber, SYNC_LANE);
            }
            flush_sync_work();
            // If we're still blocked after this, we need to increase
            // the priority of any promises resolving within this
            // boundary so that they next attempt also has higher pri.
            let retry_lane = SYNC_LANE;
            mark_retry_lane_if_not_hydrated(fiber, retry_lane);
        }
        _ => {}
    }
}

fn mark_retry_lane_impl(fiber: &Fiber, retry_lane: Lane) {
    // A SuspenseState or an ActivityState, which share these fields.
    let suspense_state: Option<&SuspenseState> = unsafe { js::cast(fiber.memoized_state.get()) };
    if let Some(suspense_state) = suspense_state
        && suspense_state.dehydrated.is_some()
    {
        suspense_state.retry_lane.set(higher_priority_lane(
            suspense_state.retry_lane.get(),
            retry_lane,
        ));
    }
}

// Increases the priority of thenables when they resolve within this boundary.
fn mark_retry_lane_if_not_hydrated(fiber: &Fiber, retry_lane: Lane) {
    mark_retry_lane_impl(fiber, retry_lane);
    let alternate = fiber.alternate.borrow().clone();
    if let Some(alternate) = alternate {
        mark_retry_lane_impl(&alternate, retry_lane);
    }
}

pub fn attempt_continuous_hydration(fiber: &Rc<Fiber>) {
    if fiber.tag.get() != SUSPENSE_COMPONENT && fiber.tag.get() != ACTIVITY_COMPONENT {
        // We ignore HostRoots here because we can't increase
        // their priority and they should not suspend on I/O,
        // since you have to wrap anything that might suspend in
        // Suspense.
        return;
    }
    let lane = SELECTIVE_HYDRATION_LANE;
    let root = enqueue_concurrent_render_for_lane(fiber, lane);
    if let Some(root) = root {
        schedule_update_on_fiber(&root, fiber, lane);
    }
    mark_retry_lane_if_not_hydrated(fiber, lane);
}

pub fn attempt_hydration_at_current_priority(fiber: &Rc<Fiber>) {
    if fiber.tag.get() != SUSPENSE_COMPONENT && fiber.tag.get() != ACTIVITY_COMPONENT {
        // We ignore HostRoots here because we can't increase
        // their priority other than synchronously flush it.
        return;
    }
    let mut lane = request_update_lane(fiber);
    lane = get_bumped_lane_for_hydration_by_lane(lane);
    let root = enqueue_concurrent_render_for_lane(fiber, lane);
    if let Some(root) = root {
        schedule_update_on_fiber(&root, fiber, lane);
    }
    mark_retry_lane_if_not_hydrated(fiber, lane);
}

// ReactFiberConfig's `PublicInstance`, if any.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_host_instance_with_no_portals(fiber: &Rc<Fiber>) -> Option<&'static Unknown> {
    let host_fiber = find_current_host_fiber_with_no_portals(fiber);
    let host_fiber = host_fiber?;
    get_public_instance(host_fiber.state_node.get())
}

/// What DevTools decides of a boundary: to force it to error (`true`), to
/// stop forcing it (`false`), or neither.
type ShouldErrorImpl = dyn Fn(&Fiber) -> Option<bool>;
/// What DevTools decides of a boundary: to force it to suspend.
type ShouldSuspendImpl = dyn Fn(&Fiber) -> bool;

thread_local! {
    static SHOULD_ERROR_IMPL: Cell<&'static ShouldErrorImpl> = Cell::new(&|_fiber| js::null());
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn should_error(fiber: &Fiber) -> Option<bool> {
    SHOULD_ERROR_IMPL.get()(fiber)
}

thread_local! {
    static SHOULD_SUSPEND_IMPL: Cell<&'static ShouldSuspendImpl> = Cell::new(&|_fiber| false);
}

pub fn should_suspend(fiber: &Fiber) -> bool {
    SHOULD_SUSPEND_IMPL.get()(fiber)
}

/// A path DevTools edits a value at: each key, a property's name or an
/// array's index.
type Path<'a> = &'a [&'static Unknown];

type OverrideHookState = fn(&Rc<Fiber>, f64, Path, Option<&'static Unknown>);
type OverrideHookStateDeletePath = fn(&Rc<Fiber>, f64, Path);
type OverrideHookStateRenamePath = fn(&Rc<Fiber>, f64, Path, Path);
type OverrideProps = fn(&Rc<Fiber>, Path, Option<&'static Unknown>);
type OverridePropsDeletePath = fn(&Rc<Fiber>, Path);
type OverridePropsRenamePath = fn(&Rc<Fiber>, Path, Path);
type ScheduleUpdate = fn(&Rc<Fiber>);
type SetErrorHandler = fn(&'static ShouldErrorImpl);
type SetSuspenseHandler = fn(&'static ShouldSuspendImpl);

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static OVERRIDE_HOOK_STATE: Cell<Option<OverrideHookState>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static OVERRIDE_HOOK_STATE_DELETE_PATH: Cell<Option<OverrideHookStateDeletePath>> =
        const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static OVERRIDE_HOOK_STATE_RENAME_PATH: Cell<Option<OverrideHookStateRenamePath>> =
        const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static OVERRIDE_PROPS: Cell<Option<OverrideProps>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static OVERRIDE_PROPS_DELETE_PATH: Cell<Option<OverridePropsDeletePath>> =
        const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static OVERRIDE_PROPS_RENAME_PATH: Cell<Option<OverridePropsRenamePath>> =
        const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SCHEDULE_UPDATE: Cell<Option<ScheduleUpdate>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SCHEDULE_RETRY: Cell<Option<ScheduleUpdate>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SET_ERROR_HANDLER: Cell<Option<SetErrorHandler>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SET_SUSPENSE_HANDLER: Cell<Option<SetSuspenseHandler>> = const { Cell::new(None) };
}

js::on_load! {
    if __DEV__ {
        // An object or an array, copied with the value at `path` gone.
        fn copy_with_delete_impl(
            obj: Option<&'static Unknown>,
            path: Path,
            index: usize,
        ) -> Option<&'static Unknown> {
            let key = path[index];
            let updated: &'static Unknown = match js::classify(obj) {
                Kind::Array(items) => js::unknown_of(items.to_vec()),
                _ => js::unknown(js::object::spread(obj)),
            };
            if index + 1 == path.len() {
                if js::array::is_array(Some(updated)) {
                    js::array::splice(updated, unsafe { js::cast(Some(key)) }, 1.0);
                } else {
                    js::delete(updated, key);
                }
                return Some(updated);
            }
            js::set(updated, key, copy_with_delete_impl(js::get(obj, key), path, index + 1));
            Some(updated)
        }

        fn copy_with_delete(obj: Option<&'static Unknown>, path: Path) -> Option<&'static Unknown> {
            copy_with_delete_impl(obj, path, 0)
        }

        fn copy_with_rename_impl(
            obj: Option<&'static Unknown>,
            old_path: Path,
            new_path: Path,
            index: usize,
        ) -> Option<&'static Unknown> {
            let old_key = old_path[index];
            let updated: &'static Unknown = match js::classify(obj) {
                Kind::Array(items) => js::unknown_of(items.to_vec()),
                _ => js::unknown(js::object::spread(obj)),
            };
            if index + 1 == old_path.len() {
                let new_key = new_path[index];
                js::set(updated, new_key, js::get(updated, old_key));
                if js::array::is_array(Some(updated)) {
                    js::array::splice(updated, unsafe { js::cast(Some(old_key)) }, 1.0);
                } else {
                    js::delete(updated, old_key);
                }
            } else {
                js::set(
                    updated,
                    old_key,
                    copy_with_rename_impl(js::get(obj, old_key), old_path, new_path, index + 1),
                );
            }
            Some(updated)
        }

        fn copy_with_rename(
            obj: Option<&'static Unknown>,
            old_path: Path,
            new_path: Path,
        ) -> Option<&'static Unknown> {
            if old_path.len() != new_path.len() {
                console::warn(("copyWithRename() expects paths of the same length",));
                return None;
            } else {
                for i in 0..new_path.len() - 1 {
                    if !js::strict_equals(old_path[i], new_path[i]) {
                        console::warn((
                            "copyWithRename() expects paths to be the same except for the deepest key",
                        ));
                        return None;
                    }
                }
            }
            copy_with_rename_impl(obj, old_path, new_path, 0)
        }

        fn copy_with_set_impl(
            obj: Option<&'static Unknown>,
            path: Path,
            index: usize,
            value: Option<&'static Unknown>,
        ) -> Option<&'static Unknown> {
            if index >= path.len() {
                return value;
            }
            let key = path[index];
            let updated: &'static Unknown = match js::classify(obj) {
                Kind::Array(items) => js::unknown_of(items.to_vec()),
                _ => js::unknown(js::object::spread(obj)),
            };
            js::set(updated, key, copy_with_set_impl(js::get(obj, key), path, index + 1, value));
            Some(updated)
        }

        fn copy_with_set(
            obj: Option<&'static Unknown>,
            path: Path,
            value: Option<&'static Unknown>,
        ) -> Option<&'static Unknown> {
            copy_with_set_impl(obj, path, 0, value)
        }

        fn find_hook(fiber: &Fiber, mut id: f64) -> Option<Rc<Hook>> {
            // For now, the "id" of stateful hooks is just the stateful hook index.
            // This may change in the future with e.g. nested hooks.
            let mut current_hook: Option<Rc<Hook>> = unsafe { js::cast(fiber.memoized_state.get()) };
            while let Some(hook) = &current_hook
                && id > 0.0
            {
                let next = hook.next.borrow().clone();
                current_hook = next;
                id -= 1.0;
            }
            current_hook
        }

        // Support DevTools editable values for useState and useReducer.
        OVERRIDE_HOOK_STATE.set(Some(|fiber, id, path, value| {
            let hook = find_hook(fiber, id);
            if let Some(hook) = hook {
                let new_state = copy_with_set(hook.memoized_state.get(), path, value);
                hook.memoized_state.set(new_state);
                hook.base_state.set(new_state);

                // We aren't actually adding an update to the queue,
                // because there is no update we can add for useReducer hooks that won't trigger an error.
                // (There's no appropriate action type for DevTools overrides.)
                // As a result though, React will see the scheduled update as a noop and bailout.
                // Shallow cloning props works as a workaround for now to bypass the bailout check.
                fiber
                    .memoized_props
                    .set(Some(js::unknown(js::object::spread(fiber.memoized_props.get()))));

                let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
                if let Some(root) = root {
                    schedule_update_on_fiber(&root, fiber, SYNC_LANE);
                }
            }
        }));
        OVERRIDE_HOOK_STATE_DELETE_PATH.set(Some(|fiber, id, path| {
            let hook = find_hook(fiber, id);
            if let Some(hook) = hook {
                let new_state = copy_with_delete(hook.memoized_state.get(), path);
                hook.memoized_state.set(new_state);
                hook.base_state.set(new_state);

                // We aren't actually adding an update to the queue,
                // because there is no update we can add for useReducer hooks that won't trigger an error.
                // (There's no appropriate action type for DevTools overrides.)
                // As a result though, React will see the scheduled update as a noop and bailout.
                // Shallow cloning props works as a workaround for now to bypass the bailout check.
                fiber
                    .memoized_props
                    .set(Some(js::unknown(js::object::spread(fiber.memoized_props.get()))));

                let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
                if let Some(root) = root {
                    schedule_update_on_fiber(&root, fiber, SYNC_LANE);
                }
            }
        }));
        OVERRIDE_HOOK_STATE_RENAME_PATH.set(Some(|fiber, id, old_path, new_path| {
            let hook = find_hook(fiber, id);
            if let Some(hook) = hook {
                let new_state = copy_with_rename(hook.memoized_state.get(), old_path, new_path);
                hook.memoized_state.set(new_state);
                hook.base_state.set(new_state);

                // We aren't actually adding an update to the queue,
                // because there is no update we can add for useReducer hooks that won't trigger an error.
                // (There's no appropriate action type for DevTools overrides.)
                // As a result though, React will see the scheduled update as a noop and bailout.
                // Shallow cloning props works as a workaround for now to bypass the bailout check.
                fiber
                    .memoized_props
                    .set(Some(js::unknown(js::object::spread(fiber.memoized_props.get()))));

                let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
                if let Some(root) = root {
                    schedule_update_on_fiber(&root, fiber, SYNC_LANE);
                }
            }
        }));

        // Support DevTools props for function components, forwardRef, memo, host components, etc.
        OVERRIDE_PROPS.set(Some(|fiber, path, value| {
            fiber
                .pending_props
                .set(copy_with_set(fiber.memoized_props.get(), path, value));
            let alternate = fiber.alternate.borrow().clone();
            if let Some(alternate) = alternate {
                alternate.pending_props.set(fiber.pending_props.get());
            }
            let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
            if let Some(root) = root {
                schedule_update_on_fiber(&root, fiber, SYNC_LANE);
            }
        }));
        OVERRIDE_PROPS_DELETE_PATH.set(Some(|fiber, path| {
            fiber
                .pending_props
                .set(copy_with_delete(fiber.memoized_props.get(), path));
            let alternate = fiber.alternate.borrow().clone();
            if let Some(alternate) = alternate {
                alternate.pending_props.set(fiber.pending_props.get());
            }
            let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
            if let Some(root) = root {
                schedule_update_on_fiber(&root, fiber, SYNC_LANE);
            }
        }));
        OVERRIDE_PROPS_RENAME_PATH.set(Some(|fiber, old_path, new_path| {
            fiber
                .pending_props
                .set(copy_with_rename(fiber.memoized_props.get(), old_path, new_path));
            let alternate = fiber.alternate.borrow().clone();
            if let Some(alternate) = alternate {
                alternate.pending_props.set(fiber.pending_props.get());
            }
            let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
            if let Some(root) = root {
                schedule_update_on_fiber(&root, fiber, SYNC_LANE);
            }
        }));

        SCHEDULE_UPDATE.set(Some(|fiber| {
            let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
            if let Some(root) = root {
                schedule_update_on_fiber(&root, fiber, SYNC_LANE);
            }
        }));

        SCHEDULE_RETRY.set(Some(|fiber| {
            let lane = claim_next_retry_lane();
            let root = enqueue_concurrent_render_for_lane(fiber, lane);
            if let Some(root) = root {
                schedule_update_on_fiber(&root, fiber, lane);
            }
        }));

        SET_ERROR_HANDLER.set(Some(|new_should_error_impl| {
            SHOULD_ERROR_IMPL.set(new_should_error_impl);
        }));

        SET_SUSPENSE_HANDLER.set(Some(|new_should_suspend_impl| {
            SHOULD_SUSPEND_IMPL.set(new_should_suspend_impl);
        }));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_current_fiber_for_dev_tools() -> Option<Rc<Fiber>> {
    REACT_CURRENT_FIBER_CURRENT.with_borrow(Clone::clone)
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_lane_label_map() -> Option<HashMap<Lane, Option<&'static str>>> {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        let mut map: HashMap<Lane, Option<&'static str>> = HashMap::new();

        let mut lane: Lane = 1;
        for _index in 0..TOTAL_LANES {
            let label = get_label_for_lane(lane);
            map.insert(lane, label);
            lane <<= 1;
        }

        Some(map)
    } else {
        None
    }
}

/// What a renderer tells DevTools of itself, more of it set as the build
/// has it.
struct DevToolsInternals {
    bundle_type: u8, // Might add PROFILE later.
    version: &'static str,
    renderer_package_name: &'static str,
    current_dispatcher_ref: &'static SharedStateClient,
    // Enables DevTools to detect reconciler version rather than renderer version
    // which may not match for third party renderers.
    reconciler_version: &'static str,
}

unsafe impl js::Defined for DevToolsInternals {}

pub fn inject_into_dev_tools() -> bool {
    let internals = js::unknown_of(DevToolsInternals {
        bundle_type: if __DEV__ { 1 } else { 0 }, // Might add PROFILE later.
        version: RENDERER_VERSION,
        renderer_package_name: RENDERER_PACKAGE_NAME,
        current_dispatcher_ref: REACT_SHARED_INTERNALS,
        // Enables DevTools to detect reconciler version rather than renderer version
        // which may not match for third party renderers.
        reconciler_version: REACT_VERSION,
    });
    if EXTRA_DEV_TOOLS_CONFIG.is_some() {
        js::set(internals, "rendererConfig", EXTRA_DEV_TOOLS_CONFIG);
    }
    if __DEV__ {
        js::set(internals, "overrideHookState", OVERRIDE_HOOK_STATE.get());
        js::set(
            internals,
            "overrideHookStateDeletePath",
            OVERRIDE_HOOK_STATE_DELETE_PATH.get(),
        );
        js::set(
            internals,
            "overrideHookStateRenamePath",
            OVERRIDE_HOOK_STATE_RENAME_PATH.get(),
        );
        js::set(internals, "overrideProps", OVERRIDE_PROPS.get());
        js::set(
            internals,
            "overridePropsDeletePath",
            OVERRIDE_PROPS_DELETE_PATH.get(),
        );
        js::set(
            internals,
            "overridePropsRenamePath",
            OVERRIDE_PROPS_RENAME_PATH.get(),
        );
        js::set(internals, "scheduleUpdate", SCHEDULE_UPDATE.get());
        js::set(internals, "scheduleRetry", SCHEDULE_RETRY.get());
        js::set(internals, "setErrorHandler", SET_ERROR_HANDLER.get());
        js::set(internals, "setSuspenseHandler", SET_SUSPENSE_HANDLER.get());
        // React Refresh
        js::set(
            internals,
            "scheduleRefresh",
            schedule_refresh as ScheduleRefresh,
        );
        js::set(internals, "scheduleRoot", schedule_root as ScheduleRoot);
        js::set(
            internals,
            "setRefreshHandler",
            set_refresh_handler as SetRefreshHandler,
        );
        // Enables DevTools to append owner stacks to error messages in DEV mode.
        js::set(
            internals,
            "getCurrentFiber",
            get_current_fiber_for_dev_tools as fn() -> Option<Rc<Fiber>>,
        );
    }
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        // Conditionally inject these hooks only if Timeline profiler is supported by this build.
        // This gives DevTools a way to feature detect that isn't tied to version number
        // (since profiling and timeline are controlled by different feature flags).
        js::set(
            internals,
            "getLaneLabelMap",
            get_lane_label_map as fn() -> Option<HashMap<Lane, Option<&'static str>>>,
        );
        js::set(
            internals,
            "injectProfilingHooks",
            inject_profiling_hooks as fn(&'static _),
        );
    }
    inject_internals(internals)
}
