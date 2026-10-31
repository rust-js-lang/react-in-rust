// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::catch_unwind;
use std::rc::Rc;

use js::{Kind, SymbolFor, Unknown};
use react_isomorphic::react_jsx_element::{ReactElement, ReactKey};
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::is_array::is_array;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, ENABLE_LEGACY_HIDDEN, ENABLE_OPTIMISTIC_KEY, ENABLE_PROFILER_TIMER,
    ENABLE_SCOPE_API, ENABLE_SUSPENSEY_IMAGES, ENABLE_TRANSITION_TRACING, ENABLE_VIEW_TRANSITION,
};
use shared::react_symbols::{
    REACT_ACTIVITY_TYPE, REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE, REACT_ELEMENT_TYPE,
    REACT_FORWARD_REF_TYPE, REACT_FRAGMENT_TYPE, REACT_LAZY_TYPE, REACT_LEGACY_HIDDEN_TYPE,
    REACT_MEMO_TYPE, REACT_PROFILER_TYPE, REACT_SCOPE_TYPE, REACT_STRICT_MODE_TYPE,
    REACT_SUSPENSE_LIST_TYPE, REACT_SUSPENSE_TYPE, REACT_TRACING_MARKER_TYPE,
    REACT_VIEW_TRANSITION_TYPE,
};
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_owner;
use crate::react_fiber_flags::{NO_FLAGS, PLACEMENT, STATIC_MASK};
use crate::react_fiber_host_context::get_host_context;
use crate::react_fiber_hot_reloading::resolve_type_for_hot_reloading;
use crate::react_fiber_lane::{Lanes, NO_LANES};
use crate::react_fiber_offscreen_component::{LegacyHiddenProps, OffscreenProps};
use crate::react_fiber_tracing_marker_component::{
    TRANSITION_TRACING_MARKER, TracingMarkerInstance,
};
use crate::react_internal_types::{Dependencies, Fiber};
use crate::react_portal::ReactPortal;
use crate::react_root_tags::{CONCURRENT_ROOT, RootTag};
use crate::react_type_of_mode::{
    CONCURRENT_MODE, NO_MODE, PROFILE_MODE, STRICT_EFFECTS_MODE, STRICT_LEGACY_MODE,
    SUSPENSEY_IMAGES_MODE, TypeOfMode,
};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CLASS_COMPONENT, CONTEXT_CONSUMER, CONTEXT_PROVIDER, DEHYDRATED_FRAGMENT,
    FORWARD_REF, FRAGMENT, FUNCTION_COMPONENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL,
    HOST_ROOT, HOST_SINGLETON, HOST_TEXT, LAZY_COMPONENT, LEGACY_HIDDEN_COMPONENT, MEMO_COMPONENT,
    MODE, OFFSCREEN_COMPONENT, PROFILER, SCOPE_COMPONENT, SIMPLE_MEMO_COMPONENT,
    SUSPENSE_COMPONENT, SUSPENSE_LIST_COMPONENT, THROW, TRACING_MARKER_COMPONENT,
    VIEW_TRANSITION_COMPONENT, WorkTag,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsResources"]
    safe static SUPPORTS_RESOURCES: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
    #[link_name = "./src/ReactFiberDevToolsHook.js#isDevToolsPresent"]
    safe static IS_DEV_TOOLS_PRESENT: bool;
}

// ReactFiberConfig's `HostContext`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isHostHoistableType"
)]
fn is_host_hoistable_type(
    r#type: &str,
    props: Option<&Unknown>,
    host_context: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isHostSingletonType"
)]
fn is_host_singleton_type(r#type: &str) -> bool {
    unreachable!()
}

/// A Profiler's `stateNode`: the time its effects took.
pub struct ProfilerStateNode {
    pub effect_duration: Cell<f64>,
    pub passive_effect_duration: Cell<f64>,
}

unsafe impl js::Defined for ProfilerStateNode {}

/// A HostPortal's `stateNode`.
pub struct PortalStateNode {
    pub container_info: Option<&'static Unknown>,
    // Used by persistent updates
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending_children: Cell<Option<&'static Unknown>>,
    pub implementation: Option<&'static Unknown>,
}

unsafe impl js::Defined for PortalStateNode {}

/// ReactFiberViewTransitionComponent's `ViewTransitionState`.
pub struct ViewTransitionState {
    // the view-transition-name to use when an explicit one is not specified
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub auto_name: RefCell<Option<String>>,
    // a temporary state during the commit phase if we have paired this with another instance
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub paired: RefCell<Option<Rc<ViewTransitionState>>>,
    // a temporary state during the apply gesture phase if we cloned this boundary
    // ReactFiberConfig's `Instance`s, an array the apply phase adds to and reads.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub clones: RefCell<Option<Rc<RefCell<Vec<&'static Unknown>>>>>,
    // the current ref instance. This can change through the lifetime of the instance.
    // ReactFiberConfig's `ViewTransitionInstance`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#ref: Cell<Option<&'static Unknown>>,
}

unsafe impl js::Defined for ViewTransitionState {}

thread_local! {
    static HAS_BAD_MAP_POLYFILL: Cell<Option<bool>> = const { Cell::new(None) };
}

js::on_load! {
    if __DEV__ {
        HAS_BAD_MAP_POLYFILL.set(Some(false));
        if let Err(_e) = catch_unwind(|| {
            let non_extensible_object = js::object::prevent_extensions(js::dict::new::<Kind>());
            // eslint-disable-next-line no-new
            js::map::new(Some(&[(non_extensible_object, Kind::Null)]));
            // eslint-disable-next-line no-new
            js::set::new(Some(&[non_extensible_object]));
        }) {
            // TODO: Consider warning about bad polyfills
            HAS_BAD_MAP_POLYFILL.set(Some(true));
        }
    }
}

fn create_fiber(
    tag: WorkTag,
    pending_props: Option<&'static Unknown>,
    key: Option<ReactKey>,
    mode: TypeOfMode,
) -> Rc<Fiber> {
    let fiber = Rc::new(Fiber {
        // Instance
        tag: Cell::new(tag),
        key: RefCell::new(key),
        element_type: Cell::new(js::null()),
        r#type: Cell::new(js::null()),
        state_node: Cell::new(js::null()),

        // Fiber
        r#return: RefCell::new(None),
        child: RefCell::new(None),
        sibling: RefCell::new(None),
        index: Cell::new(0),

        r#ref: Cell::new(None),
        ref_cleanup: Cell::new(js::null()),

        pending_props: Cell::new(pending_props),
        memoized_props: Cell::new(js::null()),
        update_queue: Cell::new(js::null()),
        memoized_state: Cell::new(js::null()),
        dependencies: RefCell::new(None),

        mode: Cell::new(mode),

        // Effects
        flags: Cell::new(NO_FLAGS),
        subtree_flags: Cell::new(NO_FLAGS),
        deletions: RefCell::new(None),

        lanes: Cell::new(NO_LANES),
        child_lanes: Cell::new(NO_LANES),

        alternate: RefCell::new(None),

        actual_duration: Cell::new(None),
        actual_start_time: Cell::new(None),
        self_base_duration: Cell::new(None),
        tree_base_duration: Cell::new(None),

        debug_info: Cell::new(None),
        debug_owner: Cell::new(None),
        debug_stack: Cell::new(None),
        debug_task: Cell::new(None),
        debug_needs_remount: Cell::new(None),
        debug_hook_types: RefCell::new(None),
    });

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        // Note: The following is done to avoid a v8 performance cliff.
        //
        // Initializing the fields below to smis and later updating them with
        // double values will cause Fibers to end up having separate shapes.
        // This behavior/bug has something to do with Object.preventExtension().
        // Fortunately this only impacts DEV builds.
        // Unfortunately it makes React unusably slow for some applications.
        // To work around this, initialize the fields below with doubles.
        //
        // Learn more about this here:
        // https://github.com/react/react/issues/14365
        // https://bugs.chromium.org/p/v8/issues/detail?id=8538
        fiber.actual_duration.set(Some(-0.0));
        fiber.actual_start_time.set(Some(-1.1));
        fiber.self_base_duration.set(Some(-0.0));
        fiber.tree_base_duration.set(Some(-0.0));
    }

    if __DEV__ {
        // This isn't directly used but is handy for debugging internals:
        fiber.debug_info.set(js::null());
        fiber.debug_owner.set(js::null());
        fiber.debug_stack.set(js::null());
        fiber.debug_task.set(js::null());
        fiber.debug_needs_remount.set(Some(false));
        fiber.debug_hook_types.replace(js::null());
        if !HAS_BAD_MAP_POLYFILL.get().unwrap_or(false)
            && js::is_function(js::object::prevent_extensions::<Fiber>)
        {
            js::object::prevent_extensions(&*fiber);
        }
    }
    fiber
}

fn should_construct(component: Option<&Unknown>) -> bool {
    let prototype = js::get(component, "prototype");
    js::truthy(prototype) && js::truthy(js::get(prototype, "isReactComponent"))
}

pub fn is_simple_function_component(r#type: Option<&Unknown>) -> bool {
    js::type_of(r#type) == "function"
        && !should_construct(r#type)
        && matches!(
            js::classify(js::get(r#type, "defaultProps")),
            Kind::Undefined
        )
}

pub fn is_function_class_component(r#type: &Unknown) -> bool {
    should_construct(Some(r#type))
}

// This is used to create an alternate fiber to do work on.
pub fn create_work_in_progress(
    current: &Rc<Fiber>,
    pending_props: Option<&'static Unknown>,
) -> Rc<Fiber> {
    let work_in_progress = current.alternate.borrow().clone();
    let work_in_progress = match work_in_progress {
        None => {
            // We use a double buffering pooling technique because we know that we'll
            // only ever need at most two versions of a tree. We pool the "other" unused
            // node that we're free to reuse. This is lazily created to avoid allocating
            // extra objects for things that are never updated. It also allow us to
            // reclaim the extra memory if needed.
            let key = current.key.borrow().clone();
            let work_in_progress =
                create_fiber(current.tag.get(), pending_props, key, current.mode.get());
            work_in_progress
                .element_type
                .set(current.element_type.get());
            work_in_progress.r#type.set(current.r#type.get());
            work_in_progress.state_node.set(current.state_node.get());

            if __DEV__ {
                // DEV-only fields

                work_in_progress.debug_owner.set(current.debug_owner.get());
                work_in_progress.debug_stack.set(current.debug_stack.get());
                work_in_progress.debug_task.set(current.debug_task.get());
                let debug_hook_types = current.debug_hook_types.borrow().clone();
                work_in_progress.debug_hook_types.replace(debug_hook_types);
            }

            work_in_progress.alternate.replace(Some(Rc::clone(current)));
            current
                .alternate
                .replace(Some(Rc::clone(&work_in_progress)));
            work_in_progress
        }
        Some(work_in_progress) => {
            work_in_progress.pending_props.set(pending_props);
            // Needed because Blocks store data on type.
            work_in_progress.r#type.set(current.r#type.get());

            // We already have an alternate.
            // Reset the effect tag.
            work_in_progress.flags.set(NO_FLAGS);

            // The effects are no longer valid.
            work_in_progress.subtree_flags.set(NO_FLAGS);
            work_in_progress.deletions.replace(None);

            if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
                // For optimistic keys, the Fibers can have different keys if one is optimistic
                // and the other one is filled in.
                let key = current.key.borrow().clone();
                work_in_progress.key.replace(key);
            }

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                // We intentionally reset, rather than copy, actualDuration & actualStartTime.
                // This prevents time from endlessly accumulating in new commits.
                // This has the downside of resetting values for different priority renders,
                // But works for yielding (the common case) and should support resuming.
                work_in_progress.actual_duration.set(Some(-0.0));
                work_in_progress.actual_start_time.set(Some(-1.1));
            }
            work_in_progress
        }
    };

    // Reset all effects except static ones.
    // Static effects are not specific to a render.
    work_in_progress
        .flags
        .set(current.flags.get() & STATIC_MASK);
    work_in_progress.child_lanes.set(current.child_lanes.get());
    work_in_progress.lanes.set(current.lanes.get());

    let child = current.child.borrow().clone();
    work_in_progress.child.replace(child);
    work_in_progress
        .memoized_props
        .set(current.memoized_props.get());
    work_in_progress
        .memoized_state
        .set(current.memoized_state.get());
    work_in_progress
        .update_queue
        .set(current.update_queue.get());

    // Clone the dependencies object. This is mutated during the render phase, so
    // it cannot be shared with the current fiber.
    let current_dependencies = current.dependencies.borrow().clone();
    work_in_progress
        .dependencies
        .replace(current_dependencies.map(|current_dependencies| {
            Rc::new(if __DEV__ {
                Dependencies {
                    lanes: Cell::new(current_dependencies.lanes.get()),
                    first_context: RefCell::new(
                        current_dependencies.first_context.borrow().clone(),
                    ),
                    debug_thenable_state: Cell::new(
                        current_dependencies.debug_thenable_state.get(),
                    ),
                }
            } else {
                Dependencies {
                    lanes: Cell::new(current_dependencies.lanes.get()),
                    first_context: RefCell::new(
                        current_dependencies.first_context.borrow().clone(),
                    ),
                    debug_thenable_state: Cell::new(None),
                }
            })
        }));

    // These will be overridden during the parent's reconciliation
    let sibling = current.sibling.borrow().clone();
    work_in_progress.sibling.replace(sibling);
    work_in_progress.index.set(current.index.get());
    work_in_progress.r#ref.set(current.r#ref.get());
    work_in_progress.ref_cleanup.set(current.ref_cleanup.get());

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        work_in_progress
            .self_base_duration
            .set(current.self_base_duration.get());
        work_in_progress
            .tree_base_duration
            .set(current.tree_base_duration.get());
    }

    if __DEV__ {
        work_in_progress.debug_info.set(current.debug_info.get());
        work_in_progress
            .debug_needs_remount
            .set(current.debug_needs_remount.get());
        match work_in_progress.tag.get() {
            FUNCTION_COMPONENT
            | SIMPLE_MEMO_COMPONENT
            | MEMO_COMPONENT
            | CLASS_COMPONENT
            | FORWARD_REF => {
                work_in_progress
                    .r#type
                    .set(resolve_type_for_hot_reloading(current.r#type.get()));
            }
            _ => {}
        }
    }

    work_in_progress
}

// Used to reuse a Fiber for a second pass.
pub fn reset_work_in_progress(work_in_progress: &Rc<Fiber>, render_lanes: Lanes) -> Rc<Fiber> {
    // This resets the Fiber to what createFiber or createWorkInProgress would
    // have set the values to before during the first pass. Ideally this wouldn't
    // be necessary but unfortunately many code paths reads from the workInProgress
    // when they should be reading from current and writing to workInProgress.

    // We assume pendingProps, index, key, ref, return are still untouched to
    // avoid doing another reconciliation.

    // Reset the effect flags but keep any Placement tags, since that's something
    // that child fiber is setting, not the reconciliation.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() & (STATIC_MASK | PLACEMENT));

    // The effects are no longer valid.

    let current = work_in_progress.alternate.borrow().clone();
    match current {
        None => {
            // Reset to createFiber's initial values.
            work_in_progress.child_lanes.set(NO_LANES);
            work_in_progress.lanes.set(render_lanes);

            work_in_progress.child.replace(None);
            work_in_progress.subtree_flags.set(NO_FLAGS);
            work_in_progress.memoized_props.set(js::null());
            work_in_progress.memoized_state.set(js::null());
            work_in_progress.update_queue.set(js::null());

            work_in_progress.dependencies.replace(None);

            work_in_progress.state_node.set(js::null());

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                // Note: We don't reset the actualTime counts. It's useful to accumulate
                // actual time across multiple render passes.
                work_in_progress.self_base_duration.set(Some(0.0));
                work_in_progress.tree_base_duration.set(Some(0.0));
            }
        }
        Some(current) => {
            // Reset to the cloned values that createWorkInProgress would've.
            work_in_progress.child_lanes.set(current.child_lanes.get());
            work_in_progress.lanes.set(current.lanes.get());

            let child = current.child.borrow().clone();
            work_in_progress.child.replace(child);
            work_in_progress.subtree_flags.set(NO_FLAGS);
            work_in_progress.deletions.replace(None);
            work_in_progress
                .memoized_props
                .set(current.memoized_props.get());
            work_in_progress
                .memoized_state
                .set(current.memoized_state.get());
            work_in_progress
                .update_queue
                .set(current.update_queue.get());
            // Needed because Blocks store data on type.
            // TODO: Blocks don't exist anymore. Do we still need this?
            work_in_progress.r#type.set(current.r#type.get());

            if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
                // For optimistic keys, the Fibers can have different keys if one is optimistic
                // and the other one is filled in.
                let key = current.key.borrow().clone();
                work_in_progress.key.replace(key);
            }

            // Clone the dependencies object. This is mutated during the render phase, so
            // it cannot be shared with the current fiber.
            let current_dependencies = current.dependencies.borrow().clone();
            work_in_progress
                .dependencies
                .replace(current_dependencies.map(|current_dependencies| {
                    Rc::new(if __DEV__ {
                        Dependencies {
                            lanes: Cell::new(current_dependencies.lanes.get()),
                            first_context: RefCell::new(
                                current_dependencies.first_context.borrow().clone(),
                            ),
                            debug_thenable_state: Cell::new(
                                current_dependencies.debug_thenable_state.get(),
                            ),
                        }
                    } else {
                        Dependencies {
                            lanes: Cell::new(current_dependencies.lanes.get()),
                            first_context: RefCell::new(
                                current_dependencies.first_context.borrow().clone(),
                            ),
                            debug_thenable_state: Cell::new(None),
                        }
                    })
                }));

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                // Note: We don't reset the actualTime counts. It's useful to accumulate
                // actual time across multiple render passes.
                work_in_progress
                    .self_base_duration
                    .set(current.self_base_duration.get());
                work_in_progress
                    .tree_base_duration
                    .set(current.tree_base_duration.get());
            }
        }
    }

    Rc::clone(work_in_progress)
}

pub fn create_host_root_fiber(tag: RootTag, is_strict_mode: bool) -> Rc<Fiber> {
    let mut mode;
    if DISABLE_LEGACY_MODE || tag == CONCURRENT_ROOT {
        mode = CONCURRENT_MODE;
        if is_strict_mode {
            mode |= STRICT_LEGACY_MODE | STRICT_EFFECTS_MODE;
        }
    } else {
        mode = NO_MODE;
    }

    if __DEV__ || (ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && IS_DEV_TOOLS_PRESENT) {
        // dev: Enable profiling instrumentation by default.
        // profile: enabled if DevTools is present or subtree is wrapped in <Profiler>.
        // production: disabled.
        mode |= PROFILE_MODE;
    }

    create_fiber(HOST_ROOT, js::null(), js::null(), mode)
}

// TODO: Get rid of this helper. Only createFiberFromElement should exist.
pub fn create_fiber_from_type_and_props(
    r#type: Option<&'static Unknown>, // React$ElementType
    key: Option<ReactKey>,
    pending_props: Option<&'static Unknown>,
    // shared/ReactTypes' `ReactComponentInfo`, or a Fiber.
    owner: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
) -> Rc<Fiber> {
    let mut mode = mode;
    let mut pending_props = pending_props;
    let mut fiber_tag = FUNCTION_COMPONENT;
    // The resolved type is set if we know what the final type will be. I.e. it's not lazy.
    let mut resolved_type = r#type;
    if __DEV__ {
        resolved_type = resolve_type_for_hot_reloading(r#type);
    }
    if js::type_of(resolved_type) == "function" {
        if should_construct(resolved_type) {
            fiber_tag = CLASS_COMPONENT;
        }
    } else if js::type_of(resolved_type) == "string" {
        let r#type: &str = unsafe { js::cast(r#type) };
        if SUPPORTS_RESOURCES && SUPPORTS_SINGLETONS {
            let host_context = get_host_context();
            fiber_tag = if is_host_hoistable_type(r#type, pending_props, host_context) {
                HOST_HOISTABLE
            } else if is_host_singleton_type(r#type) {
                HOST_SINGLETON
            } else {
                HOST_COMPONENT
            };
        } else if SUPPORTS_RESOURCES {
            let host_context = get_host_context();
            fiber_tag = if is_host_hoistable_type(r#type, pending_props, host_context) {
                HOST_HOISTABLE
            } else {
                HOST_COMPONENT
            };
        } else if SUPPORTS_SINGLETONS {
            fiber_tag = if is_host_singleton_type(r#type) {
                HOST_SINGLETON
            } else {
                HOST_COMPONENT
            };
        } else {
            fiber_tag = HOST_COMPONENT;
        }
    } else {
        'get_tag: {
            match SymbolFor::from_value(resolved_type) {
                Some(REACT_ACTIVITY_TYPE) => {
                    return create_fiber_from_activity(pending_props, mode, lanes, key);
                }
                Some(REACT_FRAGMENT_TYPE) => {
                    return create_fiber_from_fragment(
                        js::get(pending_props, "children"),
                        mode,
                        lanes,
                        key,
                    );
                }
                Some(REACT_STRICT_MODE_TYPE) => {
                    fiber_tag = MODE;
                    mode |= STRICT_LEGACY_MODE;
                    if DISABLE_LEGACY_MODE || (mode & CONCURRENT_MODE) != NO_MODE {
                        // Strict effects should never run on legacy roots
                        mode |= STRICT_EFFECTS_MODE;
                    }
                }
                Some(REACT_PROFILER_TYPE) => {
                    return create_fiber_from_profiler(pending_props, mode, lanes, key);
                }
                Some(REACT_SUSPENSE_TYPE) => {
                    return create_fiber_from_suspense(pending_props, mode, lanes, key);
                }
                Some(REACT_SUSPENSE_LIST_TYPE) => {
                    return create_fiber_from_suspense_list(pending_props, mode, lanes, key);
                }
                Some(REACT_LEGACY_HIDDEN_TYPE) if ENABLE_LEGACY_HIDDEN => {
                    return create_fiber_from_legacy_hidden(
                        unsafe { js::cast(pending_props) },
                        mode,
                        lanes,
                        key,
                    );
                }
                // falls through
                Some(REACT_LEGACY_HIDDEN_TYPE | REACT_VIEW_TRANSITION_TYPE)
                    if ENABLE_VIEW_TRANSITION =>
                {
                    return create_fiber_from_view_transition(pending_props, mode, lanes, key);
                }
                // falls through
                Some(REACT_LEGACY_HIDDEN_TYPE | REACT_VIEW_TRANSITION_TYPE | REACT_SCOPE_TYPE)
                    if ENABLE_SCOPE_API =>
                {
                    return create_fiber_from_scope(
                        unsafe { js::cast(r#type) },
                        pending_props,
                        mode,
                        lanes,
                        key,
                    );
                }
                // falls through
                Some(
                    REACT_LEGACY_HIDDEN_TYPE
                    | REACT_VIEW_TRANSITION_TYPE
                    | REACT_SCOPE_TYPE
                    | REACT_TRACING_MARKER_TYPE,
                ) if ENABLE_TRANSITION_TRACING => {
                    return create_fiber_from_tracing_marker(pending_props, mode, lanes, key);
                }
                // Fall through
                _ => {
                    if js::type_of(resolved_type) == "object" && resolved_type.is_some() {
                        match SymbolFor::from_value(js::get(resolved_type, "$$typeof")) {
                            Some(REACT_CONTEXT_TYPE) => {
                                fiber_tag = CONTEXT_PROVIDER;
                                break 'get_tag;
                            }
                            Some(REACT_CONSUMER_TYPE) => {
                                fiber_tag = CONTEXT_CONSUMER;
                                break 'get_tag;
                            }
                            // Fall through
                            Some(REACT_FORWARD_REF_TYPE) => {
                                fiber_tag = FORWARD_REF;
                                break 'get_tag;
                            }
                            Some(REACT_MEMO_TYPE) => {
                                fiber_tag = MEMO_COMPONENT;
                                break 'get_tag;
                            }
                            Some(REACT_LAZY_TYPE) => {
                                fiber_tag = LAZY_COMPONENT;
                                resolved_type = js::null();
                                break 'get_tag;
                            }
                            _ => {}
                        }
                    }
                    let mut info = String::new();
                    let type_string;
                    if __DEV__ {
                        if matches!(js::classify(r#type), Kind::Undefined)
                            || (js::type_of(r#type) == "object"
                                && r#type.is_some_and(|r#type| js::object::keys(r#type).is_empty()))
                        {
                            info += concat!(
                                " You likely forgot to export your component from the file ",
                                "it's defined in, or you might have mixed up default and named imports."
                            );
                        }

                        if matches!(js::classify(r#type), Kind::Null) {
                            type_string = "null".to_string();
                        } else if is_array(r#type) {
                            type_string = "array".to_string();
                        } else if r#type.is_some()
                            && SymbolFor::from_value(js::get(r#type, "$$typeof"))
                                == Some(REACT_ELEMENT_TYPE)
                        {
                            type_string = format!(
                                "<{} />",
                                get_component_name_from_type(js::get(r#type, "type"))
                                    .filter(|name| js::truthy(Some(name.as_str())))
                                    .unwrap_or_else(|| "Unknown".to_string())
                            );
                            info = " Did you accidentally export a JSX literal instead of a component?"
                                    .to_string();
                        } else {
                            type_string = js::type_of(r#type);
                        }

                        let owner_name = owner
                            .filter(|owner| js::truthy(Some(*owner)))
                            .and_then(get_component_name_from_owner);
                        if let Some(owner_name) =
                            owner_name.filter(|name| js::truthy(Some(name.as_str())))
                        {
                            info += &("\n\nCheck the render method of `".to_string()
                                + &owner_name
                                + "`.");
                        }
                    } else {
                        type_string = if matches!(js::classify(r#type), Kind::Null) {
                            "null".to_string()
                        } else {
                            js::type_of(r#type)
                        };
                    }

                    // The type is invalid but it's conceptually a child that errored and not the
                    // current component itself so we create a virtual child that throws in its
                    // begin phase. This is the same thing we do in ReactChildFiber if we throw
                    // but we do it here so that we can assign the debug owner and stack from the
                    // element itself. That way the error stack will point to the JSX callsite.
                    fiber_tag = THROW;
                    pending_props = js::error::new(
                        &format!(
                            concat!(
                                "Element type is invalid: expected a string (for built-in ",
                                "components) or a class/function (for composite components) ",
                                "but got: {}.{}"
                            ),
                            type_string, info
                        ),
                        None,
                    )
                    .as_unknown();
                    resolved_type = js::null();
                }
            }
        }
    }

    let fiber = create_fiber(fiber_tag, pending_props, key, mode);
    fiber.element_type.set(r#type);
    fiber.r#type.set(resolved_type);
    fiber.lanes.set(lanes);

    if __DEV__ {
        fiber.debug_owner.set(owner);
    }

    fiber
}

pub fn create_fiber_from_element(
    element: &ReactElement,
    mode: TypeOfMode,
    lanes: Lanes,
) -> Rc<Fiber> {
    let mut owner = js::null();
    if __DEV__ {
        owner = element.owner;
    }
    let r#type = element.r#type;
    let key = element.key.clone();
    let pending_props = element.props;
    let fiber =
        create_fiber_from_type_and_props(r#type, key, Some(pending_props), owner, mode, lanes);
    if __DEV__ {
        fiber.debug_owner.set(element.owner);
        fiber
            .debug_stack
            .set(unsafe { js::cast(js::get(js::unknown(element), "_debugStack")) });
        fiber
            .debug_task
            .set(unsafe { js::cast(js::get(js::unknown(element), "_debugTask")) });
    }
    fiber
}

// shared/ReactTypes' `ReactFragment`.
pub fn create_fiber_from_fragment(
    elements: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(FRAGMENT, elements, key, mode);
    fiber.lanes.set(lanes);
    fiber
}

// shared/ReactTypes' `ReactScope`.
fn create_fiber_from_scope(
    scope: &'static Unknown,
    pending_props: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(SCOPE_COMPONENT, pending_props, key, mode);
    fiber.r#type.set(Some(scope));
    fiber.element_type.set(Some(scope));
    fiber.lanes.set(lanes);
    fiber
}

fn create_fiber_from_profiler(
    pending_props: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    if __DEV__ && js::type_of(js::get(pending_props, "id")) != "string" {
        console::error((
            "Profiler must specify an \"id\" of type `string` as a prop. Received the type `%s` instead.",
            js::type_of(js::get(pending_props, "id")),
        ));
    }

    let fiber = create_fiber(PROFILER, pending_props, key, mode | PROFILE_MODE);
    fiber
        .element_type
        .set(Some(js::unknown(&REACT_PROFILER_TYPE)));
    fiber.lanes.set(lanes);

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        fiber.state_node.set(Some(js::unknown_of(ProfilerStateNode {
            effect_duration: Cell::new(0.0),
            passive_effect_duration: Cell::new(0.0),
        })));
    }

    fiber
}

pub fn create_fiber_from_suspense(
    pending_props: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(SUSPENSE_COMPONENT, pending_props, key, mode);
    fiber
        .element_type
        .set(Some(js::unknown(&REACT_SUSPENSE_TYPE)));
    fiber.lanes.set(lanes);
    fiber
}

pub fn create_fiber_from_suspense_list(
    pending_props: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(SUSPENSE_LIST_COMPONENT, pending_props, key, mode);
    fiber
        .element_type
        .set(Some(js::unknown(&REACT_SUSPENSE_LIST_TYPE)));
    fiber.lanes.set(lanes);
    fiber
}

pub fn create_fiber_from_offscreen(
    pending_props: &'static OffscreenProps,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(
        OFFSCREEN_COMPONENT,
        Some(js::unknown(pending_props)),
        key,
        mode,
    );
    fiber.lanes.set(lanes);
    fiber
}

// shared/ReactTypes' `ActivityProps`.
pub fn create_fiber_from_activity(
    pending_props: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(ACTIVITY_COMPONENT, pending_props, key, mode);
    fiber
        .element_type
        .set(Some(js::unknown(&REACT_ACTIVITY_TYPE)));
    fiber.lanes.set(lanes);
    fiber
}

// shared/ReactTypes' `ViewTransitionProps`.
pub fn create_fiber_from_view_transition(
    pending_props: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let mut mode = mode;
    if !ENABLE_SUSPENSEY_IMAGES {
        // Render a ViewTransition component opts into SuspenseyImages mode even
        // when the flag is off.
        mode |= SUSPENSEY_IMAGES_MODE;
    }
    let fiber = create_fiber(VIEW_TRANSITION_COMPONENT, pending_props, key, mode);
    fiber
        .element_type
        .set(Some(js::unknown(&REACT_VIEW_TRANSITION_TYPE)));
    fiber.lanes.set(lanes);
    let instance = ViewTransitionState {
        auto_name: RefCell::new(None),
        paired: RefCell::new(None),
        clones: RefCell::new(None),
        r#ref: Cell::new(None),
    };
    fiber.state_node.set(Some(js::unknown_of(instance)));
    fiber
}

pub fn create_fiber_from_legacy_hidden(
    pending_props: &'static LegacyHiddenProps,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(
        LEGACY_HIDDEN_COMPONENT,
        Some(js::unknown(pending_props)),
        key,
        mode,
    );
    fiber
        .element_type
        .set(Some(js::unknown(&REACT_LEGACY_HIDDEN_TYPE)));
    fiber.lanes.set(lanes);
    fiber
}

pub fn create_fiber_from_tracing_marker(
    pending_props: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
    key: Option<ReactKey>,
) -> Rc<Fiber> {
    let fiber = create_fiber(TRACING_MARKER_COMPONENT, pending_props, key, mode);
    fiber
        .element_type
        .set(Some(js::unknown(&REACT_TRACING_MARKER_TYPE)));
    fiber.lanes.set(lanes);
    let tracing_marker_instance = TracingMarkerInstance {
        tag: Some(TRANSITION_TRACING_MARKER),
        transitions: Cell::new(None),
        pending_boundaries: Cell::new(None),
        aborts: RefCell::new(None),
        name: RefCell::new(unsafe { js::cast(js::get(pending_props, "name")) }),
    };
    fiber
        .state_node
        .set(Some(js::unknown_of(tracing_marker_instance)));
    fiber
}

pub fn create_fiber_from_text(content: String, mode: TypeOfMode, lanes: Lanes) -> Rc<Fiber> {
    let fiber = create_fiber(HOST_TEXT, Some(js::unknown_of(content)), js::null(), mode);
    fiber.lanes.set(lanes);
    fiber
}

// ReactFiberConfig's `SuspenseInstance | ActivityInstance`.
pub fn create_fiber_from_dehydrated_fragment(dehydrated_node: &'static Unknown) -> Rc<Fiber> {
    let fiber = create_fiber(DEHYDRATED_FRAGMENT, js::null(), js::null(), NO_MODE);
    fiber.state_node.set(Some(dehydrated_node));
    fiber
}

pub fn create_fiber_from_portal(portal: &ReactPortal, mode: TypeOfMode, lanes: Lanes) -> Rc<Fiber> {
    let pending_props = if !matches!(js::classify(portal.children), Kind::Null) {
        portal.children
    } else {
        Some(js::unknown_of(Vec::<&Unknown>::new()))
    };
    let fiber = create_fiber(HOST_PORTAL, pending_props, portal.key.clone(), mode);
    fiber.lanes.set(lanes);
    fiber.state_node.set(Some(js::unknown_of(PortalStateNode {
        container_info: portal.container_info,
        pending_children: Cell::new(None), // Used by persistent updates
        implementation: portal.implementation,
    })));
    fiber
}

pub fn create_fiber_from_throw(
    error: Option<&'static Unknown>,
    mode: TypeOfMode,
    lanes: Lanes,
) -> Rc<Fiber> {
    let fiber = create_fiber(THROW, error, js::null(), mode);
    fiber.lanes.set(lanes);
    fiber
}
