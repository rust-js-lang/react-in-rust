// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{IntoUnknown, JsError, Kind, Rest, Set, SymbolFor, Unknown, function};
use react_isomorphic::react_context::ReactContext;
use react_isomorphic::react_create_ref::RefObject;
use react_isomorphic::react_hooks::{BasicStateAction, StartTransitionOptions};
use react_isomorphic::react_lazy::Thenable;
use react_isomorphic::react_start_transition::{Transition, s, set_t, t};
use shared::environment::__DEV__;
use shared::is_array::is_array;
#[cfg_attr(rust_js, rust_js::name = "is")]
use shared::object_is::OBJECT_IS;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, ENABLE_GESTURE_TRANSITION, ENABLE_LEGACY_CACHE,
    ENABLE_NO_CLONING_MEMO_CACHE, ENABLE_SCHEDULING_PROFILER, ENABLE_TRANSITION_TRACING,
    ENABLE_VIEW_TRANSITION,
};
use shared::react_shared_internals::{Dispatcher as SharedDispatcher, REACT_SHARED_INTERNALS};
use shared::react_symbols::{
    REACT_CONTEXT_TYPE, REACT_MEMO_CACHE_SENTINEL, REACT_RECOVERABLE_TYPE,
};
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_event_priorities::{
    CONTINUOUS_EVENT_PRIORITY, EventPriority, higher_event_priority,
};
use crate::react_fiber_async_action::{
    chain_thenable_value, peek_entangled_action_lane, peek_entangled_action_thenable,
};
use crate::react_fiber_begin_work::{
    check_if_work_in_progress_received_update, mark_work_in_progress_received_update,
};
use crate::react_fiber_cache_component::{Cache, create_cache};
use crate::react_fiber_call_user_space::{CALL_COMPONENT_IN_DEV, Component, Destroy};
use crate::react_fiber_class_update_queue::{
    create_update as create_legacy_queue_update, enqueue_update as enqueue_legacy_queue_update,
    entangle_transitions as entangle_legacy_queue_transitions,
};
use crate::react_fiber_concurrent_updates::{
    enqueue_concurrent_hook_update, enqueue_concurrent_hook_update_and_eagerly_bailout,
    enqueue_concurrent_render_for_lane,
};
use crate::react_fiber_dev_tools_hook::{
    mark_state_update_scheduled, set_is_strict_mode_for_devtools,
};
#[cfg_attr(rust_js, rust_js::name = "LayoutStaticEffect")]
use crate::react_fiber_flags::LAYOUT_STATIC as LAYOUT_STATIC_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "MountLayoutDevEffect")]
use crate::react_fiber_flags::MOUNT_LAYOUT_DEV as MOUNT_LAYOUT_DEV_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "MountPassiveDevEffect")]
use crate::react_fiber_flags::MOUNT_PASSIVE_DEV as MOUNT_PASSIVE_DEV_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "PassiveEffect")]
use crate::react_fiber_flags::PASSIVE as PASSIVE_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "PassiveStaticEffect")]
use crate::react_fiber_flags::PASSIVE_STATIC as PASSIVE_STATIC_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "StaticMaskEffect")]
use crate::react_fiber_flags::STATIC_MASK as STATIC_MASK_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "UpdateEffect")]
use crate::react_fiber_flags::UPDATE as UPDATE_EFFECT;
use crate::react_fiber_flags::{FORM_RESET, Flags, STORE_CONSISTENCY};
use crate::react_fiber_gesture_scheduler::{ScheduledGesture, schedule_gesture};
use crate::react_fiber_hidden_context::is_current_tree_hidden;
use crate::react_fiber_hydration_context::{
    get_is_hydrating, try_to_claim_next_hydratable_form_marker_instance,
};
use crate::react_fiber_lane::{
    DEFERRED_LANE, GESTURE_LANE, Lane, Lanes, NO_LANE, NO_LANES, OFFSCREEN_LANE, SYNC_LANE,
    UPDATE_LANES, includes_blocking_lane, includes_only_non_urgent_lanes, includes_some_lane,
    intersect_lanes, is_gesture_render, is_subset_of_lanes, is_transition_lane,
    mark_root_entangled, merge_lanes, remove_lanes,
};
use crate::react_fiber_new_context::{check_if_context_changed, read_context};
use crate::react_fiber_root_scheduler::request_transition_lane;
use crate::react_fiber_thenable::{
    SUSPENSE_ACTION_EXCEPTION, SUSPENSE_EXCEPTION, ThenableState, check_if_use_was_used_before,
    check_if_use_wrapped_in_try_catch, create_thenable_state, track_used_thenable,
};
use crate::react_fiber_transition::request_current_transition;
use crate::react_fiber_tree_context::get_tree_id;
use crate::react_fiber_work_loop::{
    get_work_in_progress_root, get_work_in_progress_root_render_lanes,
    is_invalid_execution_context_for_event_function, mark_skipped_update_lanes,
    request_deferred_lane, request_update_lane, schedule_update_on_fiber,
};
#[cfg_attr(rust_js, rust_js::name = "HookHasEffect")]
use crate::react_hook_effect_tags::HAS_EFFECT as HOOK_HAS_EFFECT;
use crate::react_hook_effect_tags::HookFlags;
#[cfg_attr(rust_js, rust_js::name = "HookInsertion")]
use crate::react_hook_effect_tags::INSERTION as HOOK_INSERTION;
#[cfg_attr(rust_js, rust_js::name = "HookLayout")]
use crate::react_hook_effect_tags::LAYOUT as HOOK_LAYOUT;
#[cfg_attr(rust_js, rust_js::name = "HookNoFlags")]
use crate::react_hook_effect_tags::NO_FLAGS as HOOK_NO_FLAGS;
#[cfg_attr(rust_js, rust_js::name = "HookPassive")]
use crate::react_hook_effect_tags::PASSIVE as HOOK_PASSIVE;
use crate::react_internal_types::{
    Dependencies, Dispatcher, Fiber, FiberRoot, HookType, MemoCache, MemoCacheArray,
};
use crate::react_profiler_timer::{start_host_action_timer, start_update_timer_by_lane};
use crate::react_type_of_mode::{
    CONCURRENT_MODE, NO_MODE, STRICT_EFFECTS_MODE, STRICT_LEGACY_MODE,
};
use crate::react_work_tags::{CACHE_COMPONENT, HOST_COMPONENT, HOST_ROOT};
use crate::scheduler::NOW as now;

unsafe extern "Rust" {
    // ReactFiberConfig's `ReactContext<TransitionStatus>`.
    #[link_name = "./src/ReactFiberConfig.js#HostTransitionContext"]
    safe static HOST_TRANSITION_CONTEXT: Rc<ReactContext<Option<&'static Unknown>>>;
    // ReactFiberConfig's `TransitionStatus`.
    #[cfg_attr(rust_js, rust_js::name = "NoPendingHostTransition")]
    #[link_name = "./src/ReactFiberConfig.js#NotPendingTransition"]
    safe static NO_PENDING_HOST_TRANSITION: Option<&'static Unknown>;
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#setCurrentUpdatePriority"
)]
fn set_current_update_priority(new_priority: EventPriority) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getCurrentUpdatePriority"
)]
fn get_current_update_priority() -> EventPriority {
    unreachable!()
}

pub struct Update {
    pub lane: Cell<Lane>,
    pub revert_lane: Cell<Lane>,
    pub action: Option<&'static Unknown>,
    pub has_eager_state: Cell<bool>,
    pub eager_state: Cell<Option<&'static Unknown>>,
    pub next: RefCell<Option<Rc<Update>>>,
    pub gesture: RefCell<Option<Rc<ScheduledGesture>>>, // enableGestureTransition
}

// An update is itself: two are equal only where they're one, as JS's `===`
// tells.
impl PartialEq for Update {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Update {}

/// What a hook's deps are: the values it was made of, compared with
/// `Object.is`.
pub type Deps = Vec<Option<&'static Unknown>>;

/// What a state hook's update is applied with: the next state, of the
/// state and the action.
pub type Reducer =
    dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>) -> Option<&'static Unknown>;

/// A hook's dispatch: what schedules an update of its action. The second
/// argument is what a caller may wrongly pass as a callback (DEV warns).
pub type Dispatch = dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>);

/// What `useActionState` dispatches its action with: of the payload.
pub type ActionDispatch = dyn Fn(Option<&'static Unknown>);

/// What a state hook gives: its state, and what updates it.
pub type StateAndDispatch = (Option<&'static Unknown>, Option<Rc<Dispatch>>);

/// What `useActionState` gives: its state, what dispatches the action, and
/// whether it's pending.
pub type ActionStateAndDispatch = (Option<&'static Unknown>, Rc<ActionDispatch>, bool);

pub struct UpdateQueue {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending: RefCell<Option<Rc<Update>>>,
    pub lanes: Cell<Lanes>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub dispatch: RefCell<Option<Rc<Dispatch>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub last_rendered_reducer: Cell<Option<&'static Reducer>>,
    pub last_rendered_state: Cell<Option<&'static Unknown>>,
}

thread_local! {
    static DID_WARN_ABOUT_MISMATCHED_HOOKS_FOR_COMPONENT: &'static Set<Option<String>> =
        js::set::new(None);
    static DID_WARN_UNCACHED_GET_SNAPSHOT: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_ABOUT_USE_WRAPPED_IN_TRY_CATCH: &'static Set<Option<String>> =
        js::set::new(None);
    static DID_WARN_ABOUT_ASYNC_CLIENT_COMPONENT: &'static Set<Option<String>> =
        js::set::new(None);
    static DID_WARN_ABOUT_USE_FORM_STATE: &'static Set<Option<String>> = js::set::new(None);
}

pub struct Hook {
    pub memoized_state: Cell<Option<&'static Unknown>>,
    pub base_state: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub base_queue: RefCell<Option<Rc<Update>>>,
    pub queue: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub next: RefCell<Option<Rc<Hook>>>,
}

// The effect "instance" is a shared object that remains the same for the entire
// lifetime of an effect. In Rust terms, a RefCell. We use it to store the
// "destroy" function that is returned from an effect, because that is stateful.
// The field is `undefined` if the effect is unmounted, or if the effect ran
// but is not stateful. We don't explicitly track whether the effect is mounted
// or unmounted because that can be inferred by the hiddenness of the fiber in
// the tree, i.e. whether there is a hidden Offscreen fiber above it.
//
// It's unfortunate that this is stored on a separate object, because it adds
// more memory per effect instance, but it's conceptually sound. I think there's
// likely a better data structure we could use for effects; perhaps just one
// array of effect instances per fiber. But I think this is OK for now despite
// the additional memory and we can follow up with performance
// optimizations later.
struct EffectInstance {
    destroy: Cell<Destroy>,
}

/// What an effect runs: what it gives is its destroy function, or nothing.
pub type Create = dyn Fn() -> Destroy;

pub struct Effect {
    pub tag: HookFlags,
    inst: Rc<EffectInstance>,
    pub create: Rc<Create>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub deps: Option<Deps>,
    pub next: RefCell<Option<Rc<Effect>>>,
}

struct StoreInstance {
    value: Cell<Option<&'static Unknown>>,
    get_snapshot: Cell<&'static GetSnapshot>,
}

#[derive(Clone, Copy)]
pub struct StoreConsistencyCheck {
    pub value: Option<&'static Unknown>,
    pub get_snapshot: &'static GetSnapshot,
}

/// What `useSyncExternalStore` reads the store with.
pub type GetSnapshot = dyn Fn() -> Option<&'static Unknown>;

/// What `useSyncExternalStore` subscribes with: of what's called when the
/// store changes, it gives what unsubscribes.
pub type Subscribe = dyn Fn(Rc<dyn Fn()>) -> Destroy;

/// What a `useEffectEvent` function calls: what the component last gave.
pub struct EventFunctionRef {
    pub r#impl: Cell<&'static EventFunctionImpl>,
}

pub struct EventFunctionPayload {
    pub r#ref: Rc<EventFunctionRef>,
    pub next_impl: &'static EventFunctionImpl,
}

pub struct FunctionComponentUpdateQueue {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub last_effect: RefCell<Option<Rc<Effect>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub events: RefCell<Option<Vec<EventFunctionPayload>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub stores: RefCell<Option<Rc<RefCell<Vec<StoreConsistencyCheck>>>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub memo_cache: RefCell<Option<Rc<MemoCache>>>,
}

// Safety: each is a JS object: a hook, its queue and its updates, an
// effect, and a function component's `updateQueue`.
unsafe impl js::Defined for Update {}
unsafe impl js::Defined for UpdateQueue {}
unsafe impl js::Defined for Hook {}
unsafe impl js::Defined for Effect {}
unsafe impl js::Defined for StoreInstance {}
unsafe impl js::Defined for EventFunctionRef {}
unsafe impl js::Defined for FunctionComponentUpdateQueue {}

thread_local! {
    // These are set right before calling the component.
    static RENDER_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    // The work-in-progress fiber. I've named it differently to distinguish it from
    // the work-in-progress hook.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_RENDERING_FIBER: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };

    // Hooks are stored as a linked list on the fiber's memoizedState field. The
    // current hook list is the list that belongs to the current fiber. The
    // work-in-progress hook list is a new list that will be added to the
    // work-in-progress fiber.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_HOOK: RefCell<Option<Rc<Hook>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS_HOOK: RefCell<Option<Rc<Hook>>> = const { RefCell::new(None) };

    // Whether an update was scheduled at any point during the render phase. This
    // does not get reset if we do another render pass; only when we're completely
    // finished evaluating this component. This is an optimization so we know
    // whether we need to clear render phase updates after a throw.
    static DID_SCHEDULE_RENDER_PHASE_UPDATE: Cell<bool> = const { Cell::new(false) };
    // Where an update was scheduled only during the current render pass. This
    // gets reset after each attempt.
    // TODO: Maybe there's some way to consolidate this with
    // `didScheduleRenderPhaseUpdate`. Or with `numberOfReRenders`.
    static DID_SCHEDULE_RENDER_PHASE_UPDATE_DURING_THIS_PASS: Cell<bool> = const { Cell::new(false) };
    #[cfg_attr(rust_js, rust_js::name = "shouldDoubleInvokeUserFnsInHooksDEV")]
    static SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV: Cell<bool> = const { Cell::new(false) };
    // Counts the number of useId hooks in this component.
    static LOCAL_ID_COUNTER: Cell<u32> = const { Cell::new(0) };
    // Counts number of `use`-d thenables
    static THENABLE_INDEX_COUNTER: Cell<usize> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static THENABLE_STATE: RefCell<Option<Rc<ThenableState>>> = const { RefCell::new(None) };

    // Used for ids that are generated completely client-side (i.e. not during
    // hydration). This counter is global, so client ids are not stable across
    // render attempts.
    static GLOBAL_CLIENT_ID_COUNTER: Cell<u32> = const { Cell::new(0) };
}

const RE_RENDER_LIMIT: u32 = 25;

thread_local! {
    // In DEV, this is the name of the currently executing primitive hook
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_HOOK_NAME_IN_DEV: Cell<Option<HookType>> = const { Cell::new(None) };

    // In DEV, this list ensures that hooks are called in the same order between renders.
    // The list stores the order of hooks used during the initial render (mount).
    // Subsequent renders (updates) reference this list.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static HOOK_TYPES_DEV: RefCell<Option<Rc<RefCell<Vec<HookType>>>>> = const { RefCell::new(None) };
    static HOOK_TYPES_UPDATE_INDEX_DEV: Cell<i32> = const { Cell::new(-1) };

    // In DEV, this tracks whether currently rendering component needs to ignore
    // the dependencies for Hooks that need them (e.g. useEffect or useMemo).
    // When true, such Hooks will always be "remounted". Only used during hot reload.
    static IGNORE_PREVIOUS_DEPENDENCIES: Cell<bool> = const { Cell::new(false) };
}

fn mount_hook_types_dev() {
    if __DEV__ {
        let hook_name = CURRENT_HOOK_NAME_IN_DEV.get().unwrap();

        match HOOK_TYPES_DEV.with_borrow(Clone::clone) {
            None => HOOK_TYPES_DEV.set(Some(Rc::new(RefCell::new(vec![hook_name])))),
            Some(hook_types_dev) => hook_types_dev.borrow_mut().push(hook_name),
        }
    }
}

fn update_hook_types_dev() {
    if __DEV__ {
        let hook_name = CURRENT_HOOK_NAME_IN_DEV.get().unwrap();

        let hook_types_dev = HOOK_TYPES_DEV.with_borrow(Clone::clone);
        if let Some(hook_types_dev) = hook_types_dev {
            HOOK_TYPES_UPDATE_INDEX_DEV.set(HOOK_TYPES_UPDATE_INDEX_DEV.get() + 1);
            let index = HOOK_TYPES_UPDATE_INDEX_DEV.get() as usize;
            if hook_types_dev.borrow().get(index) != Some(&hook_name) {
                warn_on_hook_mismatch_in_dev(hook_name);
            }
        }
    }
}

fn check_deps_are_array_dev(deps: Option<&Vec<Option<&'static Unknown>>>) {
    if __DEV__
        && let Some(deps) = deps
        && !is_array(Some(js::unknown(deps)))
    {
        // Verify deps, but only on mount to avoid extra checks.
        // It's unlikely their type would change as usually you define them inline.
        console::error((
            concat!(
                "%s received a final argument that is not an array (instead, received `%s`). When ",
                "specified, the final argument must be an array."
            ),
            CURRENT_HOOK_NAME_IN_DEV.get(),
            &js::type_of(Some(js::unknown(deps))),
        ));
    }
}

fn warn_on_hook_mismatch_in_dev(current_hook_name: HookType) {
    if __DEV__ {
        let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
        let component_name = get_component_name_from_fiber(&rendering_fiber);
        if !DID_WARN_ABOUT_MISMATCHED_HOOKS_FOR_COMPONENT
            .with(|did_warn| did_warn.has(&component_name))
        {
            DID_WARN_ABOUT_MISMATCHED_HOOKS_FOR_COMPONENT
                .with(|did_warn| did_warn.add(&component_name));

            let hook_types_dev = HOOK_TYPES_DEV.with_borrow(Clone::clone);
            if let Some(hook_types_dev) = hook_types_dev {
                let mut table = String::new();

                let second_column_start = 30;

                let update_index = HOOK_TYPES_UPDATE_INDEX_DEV.get();
                for i in 0..=update_index {
                    let old_hook_name = hook_types_dev.borrow().get(i as usize).copied();
                    let new_hook_name = if i == update_index {
                        Some(current_hook_name)
                    } else {
                        old_hook_name
                    };

                    let mut row = format!(
                        "{}. {}",
                        i + 1,
                        old_hook_name.map_or("undefined", |h| h.name())
                    );

                    // Extra space so second column lines up
                    // lol @ IE not supporting String#repeat
                    while js::string::length(&row) < second_column_start {
                        row += " ";
                    }

                    row += &(new_hook_name.map_or("undefined", |h| h.name()).to_string() + "\n");

                    table += &row;
                }

                console::error((
                    concat!(
                        "React has detected a change in the order of Hooks called by %s. ",
                        "This will lead to bugs and errors if not fixed. ",
                        "For more information, read the Rules of Hooks: https://react.dev/link/rules-of-hooks\n\n",
                        "   Previous render            Next render\n",
                        "   ------------------------------------------------------\n",
                        "%s",
                        "   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n"
                    ),
                    component_name.as_deref(),
                    &table,
                ));
            }
        }
    }
}

fn warn_on_use_form_state_in_dev() {
    if __DEV__ {
        let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
        let component_name = get_component_name_from_fiber(&rendering_fiber);
        if !DID_WARN_ABOUT_USE_FORM_STATE.with(|did_warn| did_warn.has(&component_name)) {
            DID_WARN_ABOUT_USE_FORM_STATE.with(|did_warn| did_warn.add(&component_name));

            console::error((
                concat!(
                    "ReactDOM.useFormState has been renamed to React.useActionState. ",
                    "Please update %s to use React.useActionState."
                ),
                component_name.as_deref(),
            ));
        }
    }
}

fn warn_if_async_client_component(component: &Component) {
    if __DEV__ {
        // This dev-only check only works for detecting native async functions,
        // not transpiled ones. There's also a prod check that we use to prevent
        // async client components from crashing the app; the prod one works even
        // for transpiled async functions. Neither mechanism is completely
        // bulletproof but together they cover the most common cases.
        let is_async_function =
            // $FlowFixMe[method-unbinding]
            js::object::to_string(Some(js::unknown(component))).unwrap() == "[object AsyncFunction]" ||
            // $FlowFixMe[method-unbinding]
            js::object::to_string(Some(js::unknown(component))).unwrap()
                == "[object AsyncGeneratorFunction]";
        if is_async_function {
            // Encountered an async Client Component. This is not yet supported.
            let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
            let component_name = get_component_name_from_fiber(&rendering_fiber);
            if !DID_WARN_ABOUT_ASYNC_CLIENT_COMPONENT.with(|did_warn| did_warn.has(&component_name))
            {
                DID_WARN_ABOUT_ASYNC_CLIENT_COMPONENT
                    .with(|did_warn| did_warn.add(&component_name));
                console::error((
                    concat!(
                        "%s is an async Client Component. ",
                        "Only Server Components can be async at the moment. This error is often caused by accidentally ",
                        "adding `'use client'` to a module that was originally written ",
                        "for the server."
                    ),
                    &match &component_name {
                        None => "An unknown Component".to_string(),
                        Some(component_name) => format!("<{component_name}>"),
                    },
                ));
            }
        }
    }
}

fn throw_invalid_hook_error() -> ! {
    panic!(concat!(
        "Invalid hook call. Hooks can only be called inside of the body of a function component. This could happen for",
        " one of the following reasons:\n",
        "1. You might have mismatching versions of React and the renderer (such as React DOM)\n",
        "2. You might be breaking the Rules of Hooks\n",
        "3. You might have more than one copy of React in the same app\n",
        "See https://react.dev/link/invalid-hook-call for tips about how to debug and fix this problem."
    ));
}

fn are_hook_inputs_equal(
    next_deps: &[Option<&'static Unknown>],
    // `Array<mixed> | null`, or what a hook of another kind keeps where the
    // order of hooks changed.
    prev_deps: Option<&'static Unknown>,
) -> bool {
    if __DEV__ && IGNORE_PREVIOUS_DEPENDENCIES.get() {
        // Only true when this component is being hot reloaded.
        return false;
    }

    if matches!(js::classify(prev_deps), Kind::Null) {
        if __DEV__ {
            console::error((
                concat!(
                    "%s received a final argument during this render, but not during ",
                    "the previous render. Even though the final argument is optional, ",
                    "its type cannot change between renders."
                ),
                CURRENT_HOOK_NAME_IN_DEV.get(),
            ));
        }
        return false;
    }
    let prev_deps: &[Option<&'static Unknown>] = unsafe { js::cast(prev_deps) };

    // Don't bother comparing lengths in prod because these arrays should be
    // passed inline.
    if __DEV__ && next_deps.len() != prev_deps.len() {
        console::error((
            concat!(
                "The final argument passed to %s changed size between renders. The ",
                "order and size of this array must remain constant.\n\n",
                "Previous: %s\n",
                "Incoming: %s"
            ),
            CURRENT_HOOK_NAME_IN_DEV.get(),
            &format!("[{}]", js::array::join(prev_deps, ", ")),
            &format!("[{}]", js::array::join(next_deps, ", ")),
        ));
    }
    // $FlowFixMe[incompatible-use] found when upgrading Flow
    let mut i = 0;
    while i < prev_deps.len() && i < next_deps.len() {
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        if OBJECT_IS.with(|is| is(&next_deps[i], &prev_deps[i])) {
            i += 1;
            continue;
        }
        return false;
    }
    true
}

struct ExitStrictModeForDevtools;

impl Drop for ExitStrictModeForDevtools {
    fn drop(&mut self) {
        set_is_strict_mode_for_devtools(false);
    }
}

pub fn render_with_hooks(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: &Component,
    props: Option<&'static Unknown>,
    second_arg: Option<&'static Unknown>,
    next_render_lanes: Lanes,
) -> Option<&'static Unknown> {
    RENDER_LANES.set(next_render_lanes);
    CURRENTLY_RENDERING_FIBER.set(Some(Rc::clone(work_in_progress)));

    if __DEV__ {
        HOOK_TYPES_DEV.set(match current {
            Some(current) => current.debug_hook_types.borrow().clone(),
            None => None,
        });
        HOOK_TYPES_UPDATE_INDEX_DEV.set(-1);
        // Used for hot reloading:
        IGNORE_PREVIOUS_DEPENDENCIES.set(current.is_some_and(|current| {
            !js::strict_equals(current.r#type.get(), work_in_progress.r#type.get())
        }));

        warn_if_async_client_component(component);
    }

    work_in_progress.memoized_state.set(js::null());
    work_in_progress.update_queue.set(js::null());
    work_in_progress.lanes.set(NO_LANES);

    // The following should have already been reset
    // currentHook = null;
    // workInProgressHook = null;

    // didScheduleRenderPhaseUpdate = false;
    // localIdCounter = 0;
    // thenableIndexCounter = 0;
    // thenableState = null;

    // TODO Warn if no hooks are used at all during mount, then some are used during update.
    // Currently we will identify the update render as a mount because memoizedState === null.
    // This is tricky because it's valid for certain types of components (e.g. React.lazy)

    // Using memoizedState to differentiate between mount/update only works if at least one stateful hook is used.
    // Non-stateful hooks (e.g. context) don't get added to memoizedState,
    // so memoizedState would be null during updates and mounts.
    if __DEV__ {
        if current.is_some_and(|current| current.memoized_state.get().is_some()) {
            HOOKS_DISPATCHER_ON_UPDATE_IN_DEV
                .with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
        } else if HOOK_TYPES_DEV.with_borrow(Option::is_some) {
            // This dispatcher handles an edge case where a component is updating,
            // but no stateful hooks have been used.
            // We want to match the production code behavior (which will use HooksDispatcherOnMount),
            // but with the extra DEV validation to ensure hooks ordering hasn't changed.
            // This dispatcher does that.
            HOOKS_DISPATCHER_ON_MOUNT_WITH_HOOK_TYPES_IN_DEV
                .with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
        } else {
            HOOKS_DISPATCHER_ON_MOUNT_IN_DEV
                .with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
        }
    } else {
        REACT_SHARED_INTERNALS.set_h(Some(
            if current.is_none_or(|current| current.memoized_state.get().is_none()) {
                &HOOKS_DISPATCHER_ON_MOUNT
            } else {
                &HOOKS_DISPATCHER_ON_UPDATE
            },
        ));
    }

    // In Strict Mode, during development, user functions are double invoked to
    // help detect side effects. The logic for how this is implemented for in
    // hook components is a bit complex so let's break it down.
    //
    // We will invoke the entire component function twice. However, during the
    // second invocation of the component, the hook state from the first
    // invocation will be reused. That means things like `useMemo` functions won't
    // run again, because the deps will match and the memoized result will
    // be reused.
    //
    // We want memoized functions to run twice, too, so account for this, user
    // functions are double invoked during the *first* invocation of the component
    // function, and are *not* double invoked during the second incovation:
    //
    // - First execution of component function: user functions are double invoked
    // - Second execution of component function (in Strict Mode, during
    //   development): user functions are not double invoked.
    //
    // This is intentional for a few reasons; most importantly, it's because of
    // how `use` works when something suspends: it reuses the promise that was
    // passed during the first attempt. This is itself a form of memoization.
    // We need to be able to memoize the reactive inputs to the `use` call using
    // a hook (i.e. `useMemo`), which means, the reactive inputs to `use` must
    // come from the same component invocation as the output.
    //
    // There are plenty of tests to ensure this behavior is correct.
    let should_double_render_dev =
        __DEV__ && (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != NO_MODE;

    SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV.set(should_double_render_dev);
    let mut children = if __DEV__ {
        CALL_COMPONENT_IN_DEV.with(|call_component_in_dev| {
            call_component_in_dev.unwrap()(component, props, second_arg)
        })
    } else {
        component(props, second_arg)
    };
    SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV.set(false);

    // Check if there was a render phase update
    if DID_SCHEDULE_RENDER_PHASE_UPDATE_DURING_THIS_PASS.get() {
        // Keep rendering until the component stabilizes (there are no more render
        // phase updates).
        children = render_with_hooks_again(work_in_progress, component, props, second_arg);
    }

    if should_double_render_dev {
        // In development, components are invoked twice to help detect side effects.
        set_is_strict_mode_for_devtools(true);
        let _exit = ExitStrictModeForDevtools;
        children = render_with_hooks_again(work_in_progress, component, props, second_arg);
    }

    finish_rendering_hooks(current, work_in_progress, component);

    children
}

fn finish_rendering_hooks(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    _component: &Component,
) {
    if __DEV__ {
        work_in_progress
            .debug_hook_types
            .replace(HOOK_TYPES_DEV.with_borrow(Clone::clone));
        // Stash the thenable state for use by DevTools.
        let thenable_state = THENABLE_STATE.with_borrow(Clone::clone);
        let dependencies = work_in_progress.dependencies.borrow().clone();
        if let Some(dependencies) = dependencies {
            dependencies
                .debug_thenable_state
                .set(thenable_state.clone().map(js::unknown_of));
        } else if let Some(thenable_state) = &thenable_state {
            work_in_progress
                .dependencies
                .replace(Some(Rc::new(Dependencies {
                    lanes: Cell::new(NO_LANES),
                    first_context: RefCell::new(None),
                    debug_thenable_state: Cell::new(Some(js::unknown_of(Rc::clone(
                        thenable_state,
                    )))),
                })));
        }
        check_if_use_was_used_before(work_in_progress, thenable_state.as_deref());
    }

    // We can assume the previous dispatcher is always this one, since we set it
    // at the beginning of the render phase and there's no re-entrance.
    REACT_SHARED_INTERNALS.set_h(Some(&CONTEXT_ONLY_DISPATCHER));

    // This check uses currentHook so that it works the same in DEV and prod bundles.
    // hookTypesDev could catch more cases (e.g. context) but only in DEV bundles.
    let did_render_too_few_hooks = CURRENT_HOOK
        .with_borrow(Clone::clone)
        .is_some_and(|current_hook| current_hook.next.borrow().is_some());

    RENDER_LANES.set(NO_LANES);
    CURRENTLY_RENDERING_FIBER.set(None);

    CURRENT_HOOK.set(None);
    WORK_IN_PROGRESS_HOOK.set(None);

    if __DEV__ {
        CURRENT_HOOK_NAME_IN_DEV.set(None);
        HOOK_TYPES_DEV.set(None);
        HOOK_TYPES_UPDATE_INDEX_DEV.set(-1);

        // Confirm that a static flag was not added or removed since the last
        // render. If this fires, it suggests that we incorrectly reset the static
        // flags in some other part of the codebase. This has happened before, for
        // example, in the SuspenseList implementation.
        if let Some(current) = current
            && (current.flags.get() & STATIC_MASK_EFFECT)
                != (work_in_progress.flags.get() & STATIC_MASK_EFFECT)
            // Disable this warning in legacy mode, because legacy Suspense is weird
            // and creates false positives. To make this work in legacy mode, we'd
            // need to mark fibers that commit in an incomplete state, somehow. For
            // now I'll disable the warning that most of the bugs that would trigger
            // it are either exclusive to concurrent mode or exist in both.
            && (DISABLE_LEGACY_MODE || (current.mode.get() & CONCURRENT_MODE) != NO_MODE)
        {
            console::error((concat!(
                "Internal React error: Expected static flag was missing. Please ",
                "notify the React team."
            ),));
        }
    }

    DID_SCHEDULE_RENDER_PHASE_UPDATE.set(false);
    // This is reset by checkDidRenderIdHook
    // localIdCounter = 0;

    THENABLE_INDEX_COUNTER.set(0);
    THENABLE_STATE.set(None);

    if did_render_too_few_hooks {
        panic!(concat!(
            "Rendered fewer hooks than expected. This may be caused by an accidental ",
            "early return statement."
        ));
    }

    if let Some(current) = current
        && !check_if_work_in_progress_received_update()
    {
        // If there were no changes to props or state, we need to check if there
        // was a context change. We didn't already do this because there's no
        // 1:1 correspondence between dependencies and hooks. Although, because
        // there almost always is in the common case (`readContext` is an
        // internal API), we could compare in there. OTOH, we only hit this case
        // if everything else bails out, so on the whole it might be better to
        // keep the comparison out of the common path.
        let current_dependencies = current.dependencies.borrow().clone();
        if let Some(current_dependencies) = current_dependencies
            && check_if_context_changed(&current_dependencies)
        {
            mark_work_in_progress_received_update();
        }
    }

    if __DEV__ && check_if_use_wrapped_in_try_catch() {
        let component_name = get_component_name_from_fiber(work_in_progress)
            .filter(|name| js::truthy(Some(name.as_str())))
            .unwrap_or_else(|| "Unknown".to_string());
        if !DID_WARN_ABOUT_USE_WRAPPED_IN_TRY_CATCH
            .with(|did_warn| did_warn.has(&Some(component_name.clone())))
            // This warning also fires if you suspend with `use` inside an
            // async component. Since we warn for that above, we'll silence this
            // second warning by checking here.
            && !DID_WARN_ABOUT_ASYNC_CLIENT_COMPONENT
                .with(|did_warn| did_warn.has(&Some(component_name.clone())))
        {
            DID_WARN_ABOUT_USE_WRAPPED_IN_TRY_CATCH
                .with(|did_warn| did_warn.add(&Some(component_name)));
            console::error((concat!(
                "`use` was called from inside a try/catch block. This is not allowed ",
                "and can lead to unexpected behavior. To handle errors triggered ",
                "by `use`, wrap your component in a error boundary."
            ),));
        }
    }
}

pub fn replay_suspended_component_with_hooks(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    component: &Component,
    props: Option<&'static Unknown>,
    second_arg: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    // This function is used to replay a component that previously suspended,
    // after its data resolves.
    //
    // It's a simplified version of renderWithHooks, but it doesn't need to do
    // most of the set up work because they weren't reset when we suspended; they
    // only get reset when the component either completes (finishRenderingHooks)
    // or unwinds (resetHooksOnUnwind).
    if __DEV__ {
        HOOK_TYPES_UPDATE_INDEX_DEV.set(-1);
        // Used for hot reloading:
        IGNORE_PREVIOUS_DEPENDENCIES.set(current.is_some_and(|current| {
            !js::strict_equals(current.r#type.get(), work_in_progress.r#type.get())
        }));
    }
    // renderWithHooks only resets the updateQueue but does not clear it, since
    // it needs to work for both this case (suspense replay) as well as for double
    // renders in dev and setState-in-render. However, for the suspense replay case
    // we need to reset the updateQueue to correctly handle unmount effects, so we
    // clear the queue here
    work_in_progress.update_queue.set(js::null());
    let children = render_with_hooks_again(work_in_progress, component, props, second_arg);
    finish_rendering_hooks(current, work_in_progress, component);
    children
}

fn render_with_hooks_again(
    work_in_progress: &Rc<Fiber>,
    component: &Component,
    props: Option<&'static Unknown>,
    second_arg: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    // This is used to perform another render pass. It's used when setState is
    // called during render, and for double invoking components in Strict Mode
    // during development.
    //
    // The state from the previous pass is reused whenever possible. So, state
    // updates that were already processed are not processed again, and memoized
    // functions (`useMemo`) are not invoked again.
    //
    // Keep rendering in a loop for as long as render phase updates continue to
    // be scheduled. Use a counter to prevent infinite loops.

    CURRENTLY_RENDERING_FIBER.set(Some(Rc::clone(work_in_progress)));

    let mut number_of_re_renders: u32 = 0;
    let mut children;
    loop {
        if DID_SCHEDULE_RENDER_PHASE_UPDATE_DURING_THIS_PASS.get() {
            // It's possible that a use() value depended on a state that was updated in
            // this rerender, so we need to watch for different thenables this time.
            THENABLE_STATE.set(None);
        }
        THENABLE_INDEX_COUNTER.set(0);
        DID_SCHEDULE_RENDER_PHASE_UPDATE_DURING_THIS_PASS.set(false);

        if number_of_re_renders >= RE_RENDER_LIMIT {
            panic!(concat!(
                "Too many re-renders. React limits the number of renders to prevent ",
                "an infinite loop."
            ));
        }

        number_of_re_renders += 1;
        if __DEV__ {
            // Even when hot reloading, allow dependencies to stabilize
            // after first render to prevent infinite render phase updates.
            IGNORE_PREVIOUS_DEPENDENCIES.set(false);
        }

        // Start over from the beginning of the list
        CURRENT_HOOK.set(None);
        WORK_IN_PROGRESS_HOOK.set(None);

        let update_queue: Option<&FunctionComponentUpdateQueue> =
            unsafe { js::cast(work_in_progress.update_queue.get()) };
        if let Some(update_queue) = update_queue {
            reset_function_component_update_queue(update_queue);
        }

        if __DEV__ {
            // Also validate hook order for cascading updates.
            HOOK_TYPES_UPDATE_INDEX_DEV.set(-1);
        }

        if __DEV__ {
            HOOKS_DISPATCHER_ON_RERENDER_IN_DEV
                .with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
        } else {
            REACT_SHARED_INTERNALS.set_h(Some(&HOOKS_DISPATCHER_ON_RERENDER));
        }

        children = if __DEV__ {
            CALL_COMPONENT_IN_DEV.with(|call_component_in_dev| {
                call_component_in_dev.unwrap()(component, props, second_arg)
            })
        } else {
            component(props, second_arg)
        };
        if !DID_SCHEDULE_RENDER_PHASE_UPDATE_DURING_THIS_PASS.get() {
            break;
        }
    }
    children
}

// ReactFiberConfig's `TransitionStatus`.
pub fn render_transition_aware_host_component_with_hooks(
    current: Option<&Rc<Fiber>>,
    work_in_progress: &Rc<Fiber>,
    lanes: Lanes,
) -> Option<&'static Unknown> {
    render_with_hooks(
        current,
        work_in_progress,
        &|_, _| transition_aware_host_component(),
        js::null(),
        js::null(),
        lanes,
    )
}

// ReactFiberConfig's `TransitionStatus`.
#[cfg_attr(rust_js, rust_js::name = "TransitionAwareHostComponent")]
pub fn transition_aware_host_component() -> Option<&'static Unknown> {
    let dispatcher: &Dispatcher = unsafe { js::cast(REACT_SHARED_INTERNALS.h().map(js::unknown)) };
    let (maybe_thenable, _) = (dispatcher.use_state)(None);
    let next_state = if js::type_of(js::get(maybe_thenable, "then")) == "function" {
        let thenable: &'static Thenable = unsafe { js::cast(maybe_thenable) };
        use_thenable(thenable)
    } else {
        maybe_thenable
    };

    // The "reset state" is an object. If it changes, that means something
    // requested that we reset the form.
    let (next_reset_state, _) = (dispatcher.use_state)(None);
    let prev_reset_state = match CURRENT_HOOK.with_borrow(Clone::clone) {
        Some(current_hook) => current_hook.memoized_state.get(),
        None => js::null(),
    };
    if !js::strict_equals(prev_reset_state, next_reset_state) {
        // Schedule a form reset
        let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
        rendering_fiber
            .flags
            .set(rendering_fiber.flags.get() | FORM_RESET);
    }

    next_state
}

pub fn check_did_render_id_hook() -> bool {
    // This should be called immediately after every renderWithHooks call.
    // Conceptually, it's part of the return value of renderWithHooks; it's only a
    // separate function to avoid using an array tuple.
    let did_render_id_hook = LOCAL_ID_COUNTER.get() != 0;
    LOCAL_ID_COUNTER.set(0);
    did_render_id_hook
}

pub fn bailout_hooks(current: &Fiber, work_in_progress: &Fiber, lanes: Lanes) {
    work_in_progress
        .update_queue
        .set(current.update_queue.get());
    // TODO: Don't need to reset the flags here, because they're reset in the
    // complete phase (bubbleProperties).
    if __DEV__ && (work_in_progress.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
        work_in_progress.flags.set(
            work_in_progress.flags.get()
                & !(MOUNT_PASSIVE_DEV_EFFECT
                    | MOUNT_LAYOUT_DEV_EFFECT
                    | PASSIVE_EFFECT
                    | UPDATE_EFFECT),
        );
    } else {
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() & !(PASSIVE_EFFECT | UPDATE_EFFECT));
    }
    current.lanes.set(remove_lanes(current.lanes.get(), lanes));
}

pub fn reset_hooks_after_throw() {
    // This is called immediaetly after a throw. It shouldn't reset the entire
    // module state, because the work loop might decide to replay the component
    // again without rewinding.
    //
    // It should only reset things like the current dispatcher, to prevent hooks
    // from being called outside of a component.
    CURRENTLY_RENDERING_FIBER.set(None);

    // We can assume the previous dispatcher is always this one, since we set it
    // at the beginning of the render phase and there's no re-entrance.
    REACT_SHARED_INTERNALS.set_h(Some(&CONTEXT_ONLY_DISPATCHER));
}

pub fn reset_hooks_on_unwind(work_in_progress: &Fiber) {
    if DID_SCHEDULE_RENDER_PHASE_UPDATE.get() {
        // There were render phase updates. These are only valid for this render
        // phase, which we are now aborting. Remove the updates from the queues so
        // they do not persist to the next render. Do not remove updates from hooks
        // that weren't processed.
        //
        // Only reset the updates from the queue if it has a clone. If it does
        // not have a clone, that means it wasn't processed, and the updates were
        // scheduled before we entered the render phase.
        let mut hook: Option<Rc<Hook>> = unsafe { js::cast(work_in_progress.memoized_state.get()) };
        while let Some(current) = hook {
            let queue: Option<&UpdateQueue> = unsafe { js::cast(current.queue.get()) };
            if let Some(queue) = queue {
                queue.pending.replace(None);
            }
            hook = current.next.borrow().clone();
        }
        DID_SCHEDULE_RENDER_PHASE_UPDATE.set(false);
    }

    RENDER_LANES.set(NO_LANES);
    CURRENTLY_RENDERING_FIBER.set(None);

    CURRENT_HOOK.set(None);
    WORK_IN_PROGRESS_HOOK.set(None);

    if __DEV__ {
        HOOK_TYPES_DEV.set(None);
        HOOK_TYPES_UPDATE_INDEX_DEV.set(-1);

        CURRENT_HOOK_NAME_IN_DEV.set(None);
    }

    DID_SCHEDULE_RENDER_PHASE_UPDATE_DURING_THIS_PASS.set(false);
    LOCAL_ID_COUNTER.set(0);
    THENABLE_INDEX_COUNTER.set(0);
    THENABLE_STATE.set(None);
}

fn mount_work_in_progress_hook() -> Rc<Hook> {
    let hook = Rc::new(Hook {
        memoized_state: Cell::new(js::null()),

        base_state: Cell::new(js::null()),
        base_queue: RefCell::new(None),
        queue: Cell::new(js::null()),

        next: RefCell::new(None),
    });

    match WORK_IN_PROGRESS_HOOK.with_borrow(Clone::clone) {
        None => {
            // This is the first hook in the list
            let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
            rendering_fiber
                .memoized_state
                .set(Some(js::unknown_of(Rc::clone(&hook))));
        }
        Some(work_in_progress_hook) => {
            // Append to the end of the list
            work_in_progress_hook.next.replace(Some(Rc::clone(&hook)));
        }
    }
    WORK_IN_PROGRESS_HOOK.set(Some(Rc::clone(&hook)));
    hook
}

fn update_work_in_progress_hook() -> Rc<Hook> {
    // This function is used both for updates and for re-renders triggered by a
    // render phase update. It assumes there is either a current hook we can
    // clone, or a work-in-progress hook from a previous render pass that we can
    // use as a base.
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    let next_current_hook: Option<Rc<Hook>> = match CURRENT_HOOK.with_borrow(Clone::clone) {
        None => {
            let current = rendering_fiber.alternate.borrow().clone();
            match current {
                Some(current) => unsafe { js::cast(current.memoized_state.get()) },
                None => None,
            }
        }
        Some(current_hook) => current_hook.next.borrow().clone(),
    };

    let next_work_in_progress_hook: Option<Rc<Hook>> =
        match WORK_IN_PROGRESS_HOOK.with_borrow(Clone::clone) {
            None => unsafe { js::cast(rendering_fiber.memoized_state.get()) },
            Some(work_in_progress_hook) => work_in_progress_hook.next.borrow().clone(),
        };

    if let Some(next_work_in_progress_hook) = next_work_in_progress_hook {
        // There's already a work-in-progress. Reuse it.
        WORK_IN_PROGRESS_HOOK.set(Some(Rc::clone(&next_work_in_progress_hook)));

        CURRENT_HOOK.set(next_current_hook);
        next_work_in_progress_hook
    } else {
        // Clone from the current hook.

        let Some(next_current_hook) = next_current_hook else {
            let current_fiber = rendering_fiber.alternate.borrow().clone();
            if current_fiber.is_none() {
                // This is the initial render. This branch is reached when the component
                // suspends, resumes, then renders an additional hook.
                // Should never be reached because we should switch to the mount dispatcher first.
                panic!(
                    "Update hook called on initial render. This is likely a bug in React. Please file an issue."
                );
            } else {
                // This is an update. We should always have a current hook.
                panic!("Rendered more hooks than during the previous render.");
            }
        };

        CURRENT_HOOK.set(Some(Rc::clone(&next_current_hook)));

        let new_hook = Rc::new(Hook {
            memoized_state: Cell::new(next_current_hook.memoized_state.get()),

            base_state: Cell::new(next_current_hook.base_state.get()),
            base_queue: RefCell::new(next_current_hook.base_queue.borrow().clone()),
            queue: Cell::new(next_current_hook.queue.get()),

            next: RefCell::new(None),
        });

        match WORK_IN_PROGRESS_HOOK.with_borrow(Clone::clone) {
            None => {
                // This is the first hook in the list.
                rendering_fiber
                    .memoized_state
                    .set(Some(js::unknown_of(Rc::clone(&new_hook))));
            }
            Some(work_in_progress_hook) => {
                // Append to the end of the list.
                work_in_progress_hook
                    .next
                    .replace(Some(Rc::clone(&new_hook)));
            }
        }
        WORK_IN_PROGRESS_HOOK.set(Some(Rc::clone(&new_hook)));
        new_hook
    }
}

fn create_function_component_update_queue() -> Rc<FunctionComponentUpdateQueue> {
    Rc::new(FunctionComponentUpdateQueue {
        last_effect: RefCell::new(None),
        events: RefCell::new(None),
        stores: RefCell::new(None),
        memo_cache: RefCell::new(None),
    })
}

fn reset_function_component_update_queue(update_queue: &FunctionComponentUpdateQueue) {
    update_queue.last_effect.replace(None);
    update_queue.events.replace(None);
    update_queue.stores.replace(None);
    let memo_cache = update_queue.memo_cache.borrow().clone();
    if let Some(memo_cache) = memo_cache {
        // NOTE: this function intentionally does not reset memoCache data. We reuse updateQueue for the memo
        // cache to avoid increasing the size of fibers that don't need a cache, but we don't want to reset
        // the cache when other properties are reset.
        memo_cache.index.set(0);
    }
}

fn use_thenable(thenable: &'static Thenable) -> Option<&'static Unknown> {
    // Track the position of the thenable within this fiber.
    let index = THENABLE_INDEX_COUNTER.get();
    THENABLE_INDEX_COUNTER.set(THENABLE_INDEX_COUNTER.get() + 1);
    if THENABLE_STATE.with_borrow(Option::is_none) {
        THENABLE_STATE.set(Some(Rc::new(create_thenable_state())));
    }
    let thenable_state = THENABLE_STATE.with_borrow(Clone::clone).unwrap();
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone);
    let result = track_used_thenable(
        &thenable_state,
        thenable,
        index,
        if __DEV__ {
            rendering_fiber.as_ref()
        } else {
            None
        },
    );

    // When something suspends with `use`, we replay the component with the
    // "re-render" dispatcher instead of the "mount" or "update" dispatcher.
    //
    // But if there are additional hooks that occur after the `use` invocation
    // that suspended, they wouldn't have been processed during the previous
    // attempt. So after we invoke `use` again, we may need to switch from the
    // "re-render" dispatcher back to the "mount" or "update" dispatcher. That's
    // what the following logic accounts for.
    //
    // TODO: Theoretically this logic only needs to go into the rerender
    // dispatcher. Could optimize, but probably not be worth it.

    // This is the same logic as in updateWorkInProgressHook.
    let work_in_progress_fiber = rendering_fiber.unwrap();
    let next_work_in_progress_hook: Option<Rc<Hook>> =
        match WORK_IN_PROGRESS_HOOK.with_borrow(Clone::clone) {
            // We're at the beginning of the list, so read from the first hook from
            // the fiber.
            None => unsafe { js::cast(work_in_progress_fiber.memoized_state.get()) },
            Some(work_in_progress_hook) => work_in_progress_hook.next.borrow().clone(),
        };

    if next_work_in_progress_hook.is_some() {
        // There are still hooks remaining from the previous attempt.
    } else {
        // There are no remaining hooks from the previous attempt. We're no longer
        // in "re-render" mode. Switch to the normal mount or update dispatcher.
        //
        // This is the same as the logic in renderWithHooks, except we don't bother
        // to track the hook types debug information in this case (sufficient to
        // only do that when nothing suspends).
        let current_fiber = work_in_progress_fiber.alternate.borrow().clone();
        if __DEV__ {
            if current_fiber
                .is_some_and(|current_fiber| current_fiber.memoized_state.get().is_some())
            {
                HOOKS_DISPATCHER_ON_UPDATE_IN_DEV
                    .with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
            } else {
                HOOKS_DISPATCHER_ON_MOUNT_IN_DEV
                    .with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
            }
        } else {
            REACT_SHARED_INTERNALS.set_h(Some(
                if current_fiber
                    .is_none_or(|current_fiber| current_fiber.memoized_state.get().is_none())
                {
                    &HOOKS_DISPATCHER_ON_MOUNT
                } else {
                    &HOOKS_DISPATCHER_ON_UPDATE
                },
            ));
        }
    }
    result
}

// shared/ReactTypes' `Usable<T>`.
fn r#use(usable: Option<&'static Unknown>) -> Option<&'static Unknown> {
    // $FlowFixMe[invalid-compare]
    if let Some(usable) = usable
        && js::type_of(Some(usable)) == "object"
    {
        // $FlowFixMe[method-unbinding]
        if js::type_of(js::get(usable, "then")) == "function" {
            // This is a thenable.
            let thenable: &'static Thenable = unsafe { js::cast(Some(usable)) };
            return use_thenable(thenable);
        } else if SymbolFor::from_value(js::get(usable, "$$typeof")) == Some(REACT_RECOVERABLE_TYPE)
        {
            // Fiber is the final renderer, so there is no downstream host that
            // needs to recover this subtree. Continue rendering through it.
            return None;
        } else if SymbolFor::from_value(js::get(usable, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
            let context: Rc<ReactContext<Option<&'static Unknown>>> =
                unsafe { js::cast(Some(usable)) };
            return read_context(&context);
        }
    }

    // eslint-disable-next-line react-internal/safe-string-coercion
    panic!(
        "An unsupported type was passed to use(): {}",
        js::string(usable)
    );
}

fn use_memo_cache(size: usize) -> MemoCacheArray {
    let mut memo_cache: Option<Rc<MemoCache>> = None;
    // Fast-path, load memo cache from wip fiber if already prepared
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    let update_queue: Option<Rc<FunctionComponentUpdateQueue>> =
        unsafe { js::cast(rendering_fiber.update_queue.get()) };
    if let Some(update_queue) = &update_queue {
        memo_cache = update_queue.memo_cache.borrow().clone();
    }
    // Otherwise clone from the current fiber
    if memo_cache.is_none() {
        let current = rendering_fiber.alternate.borrow().clone();
        if let Some(current) = current {
            let current_update_queue: Option<Rc<FunctionComponentUpdateQueue>> =
                unsafe { js::cast(current.update_queue.get()) };
            if let Some(current_update_queue) = current_update_queue {
                let current_memo_cache = current_update_queue.memo_cache.borrow().clone();
                if let Some(current_memo_cache) = current_memo_cache {
                    memo_cache = Some(Rc::new(MemoCache {
                        // When enableNoCloningMemoCache is enabled, instead of treating the
                        // cache as copy-on-write, like we do with fibers, we share the same
                        // cache instance across all render attempts, even if the component
                        // is interrupted before it commits.
                        //
                        // If an update is interrupted, either because it suspended or
                        // because of another update, we can reuse the memoized computations
                        // from the previous attempt. We can do this because the React
                        // Compiler performs atomic writes to the memo cache, i.e. it will
                        // not record the inputs to a memoization without also recording its
                        // output.
                        //
                        // This gives us a form of "resuming" within components and hooks.
                        //
                        // This only works when updating a component that already mounted.
                        // It has no impact during initial render, because the memo cache is
                        // stored on the fiber, and since we have not implemented resuming
                        // for fibers, it's always a fresh memo cache, anyway.
                        //
                        // However, this alone is pretty useful — it happens whenever you
                        // update the UI with fresh data after a mutation/action, which is
                        // extremely common in a Suspense-driven (e.g. RSC or Relay) app.
                        data: if ENABLE_NO_CLONING_MEMO_CACHE {
                            Rc::clone(&current_memo_cache.data)
                        } else {
                            // Clone the memo cache before each render (copy-on-write)
                            Rc::new(RefCell::new(
                                current_memo_cache
                                    .data
                                    .borrow()
                                    .iter()
                                    .map(|array| Rc::new(RefCell::new(array.borrow().clone())))
                                    .collect(),
                            ))
                        },
                        index: Cell::new(0),
                    }));
                }
            }
        }
    }
    // Finally fall back to allocating a fresh instance of the cache
    let memo_cache = memo_cache.unwrap_or_else(|| {
        Rc::new(MemoCache {
            data: Rc::new(RefCell::new(Vec::new())),
            index: Cell::new(0),
        })
    });
    let update_queue = match update_queue {
        Some(update_queue) => update_queue,
        None => {
            let update_queue = create_function_component_update_queue();
            rendering_fiber
                .update_queue
                .set(Some(js::unknown_of(Rc::clone(&update_queue))));
            update_queue
        }
    };
    update_queue
        .memo_cache
        .replace(Some(Rc::clone(&memo_cache)));

    let index = memo_cache.index.get();
    let mut data = memo_cache.data.borrow().get(index).cloned();
    match &data {
        Some(data) if !(__DEV__ && IGNORE_PREVIOUS_DEPENDENCIES.get()) => {
            let length = data.borrow().len();
            if length != size {
                // TODO: consider warning or throwing here
                if __DEV__ {
                    console::error((
                        concat!(
                            "Expected a constant size argument for each invocation of useMemoCache. ",
                            "The previous cache was allocated with size %s but size %s was requested."
                        ),
                        length,
                        size,
                    ));
                }
            }
        }
        _ => {
            let new_data = Rc::new(RefCell::new(vec![
                Some(js::unknown(
                    &REACT_MEMO_CACHE_SENTINEL
                ));
                size
            ]));
            let mut memo_cache_data = memo_cache.data.borrow_mut();
            if index < memo_cache_data.len() {
                memo_cache_data[index] = Rc::clone(&new_data);
            } else {
                memo_cache_data.push(Rc::clone(&new_data));
            }
            data = Some(new_data);
        }
    }
    memo_cache.index.set(memo_cache.index.get() + 1);
    data.unwrap()
}

fn basic_state_reducer(
    state: Option<&'static Unknown>,
    action: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let action: BasicStateAction<Option<&'static Unknown>> = unsafe { js::cast(action) };
    // $FlowFixMe[incompatible-use]: Flow doesn't like mixed types
    match action {
        BasicStateAction::Update(action) => action(state),
        BasicStateAction::State(action) => action,
    }
}

fn mount_reducer(
    reducer: &'static Reducer,
    initial_arg: Option<&'static Unknown>,
    init: Option<&'static Unknown>,
) -> StateAndDispatch {
    let hook = mount_work_in_progress_hook();
    let initial_state;
    if !matches!(js::classify(init), Kind::Undefined) {
        let init: &dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown> =
            unsafe { js::cast(init) };
        initial_state = init(initial_arg);
        if SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV.get() {
            set_is_strict_mode_for_devtools(true);
            let _exit = ExitStrictModeForDevtools;
            init(initial_arg);
        }
    } else {
        initial_state = initial_arg;
    }
    hook.memoized_state.set(initial_state);
    hook.base_state.set(initial_state);
    let queue = Rc::new(UpdateQueue {
        pending: RefCell::new(None),
        lanes: Cell::new(NO_LANES),
        dispatch: RefCell::new(None),
        last_rendered_reducer: Cell::new(Some(reducer)),
        last_rendered_state: Cell::new(initial_state),
    });
    hook.queue.set(Some(js::unknown_of(Rc::clone(&queue))));
    let dispatch: Rc<Dispatch> = Rc::from(function::bind(
        dispatch_reducer_action,
        Some(&Kind::Null),
        (
            CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap(),
            Rc::clone(&queue),
        ),
    ));
    queue.dispatch.replace(Some(Rc::clone(&dispatch)));
    (hook.memoized_state.get(), Some(dispatch))
}

fn update_reducer(
    reducer: &'static Reducer,
    _initial_arg: Option<&'static Unknown>,
    _init: Option<&'static Unknown>,
) -> StateAndDispatch {
    let hook = update_work_in_progress_hook();
    let current_hook = CURRENT_HOOK.with_borrow(Clone::clone);
    update_reducer_impl(&hook, current_hook.as_deref(), reducer)
}

fn update_reducer_impl(
    hook: &Hook,
    current: Option<&Hook>,
    reducer: &'static Reducer,
) -> StateAndDispatch {
    let queue: Option<&UpdateQueue> = unsafe { js::cast(hook.queue.get()) };

    let Some(queue) = queue else {
        panic!(concat!(
            "Should have a queue. You are likely calling Hooks conditionally, ",
            "which is not allowed. (https://react.dev/link/invalid-hook-call)"
        ));
    };

    queue.last_rendered_reducer.set(Some(reducer));

    // The last rebase update that is NOT part of the base state.
    let mut base_queue = hook.base_queue.borrow().clone();

    // The last pending update that hasn't been processed yet.
    let pending_queue = queue.pending.borrow().clone();
    if let Some(pending_queue) = pending_queue {
        // We have new updates that haven't been processed yet.
        // We'll add them to the base queue.
        if let Some(base_queue) = &base_queue {
            // Merge the pending queue and the base queue.
            let base_first = base_queue.next.borrow().clone();
            let pending_first = pending_queue.next.borrow().clone();
            base_queue.next.replace(pending_first);
            pending_queue.next.replace(base_first);
        }
        let current = current.unwrap();
        if __DEV__ && *current.base_queue.borrow() != base_queue {
            // Internal invariant that should never happen, but feasibly could in
            // the future if we implement resuming, or some form of that.
            console::error((concat!(
                "Internal error: Expected work-in-progress queue to be a clone. ",
                "This is a bug in React."
            ),));
        }
        current.base_queue.replace(Some(Rc::clone(&pending_queue)));
        base_queue = Some(pending_queue);
        queue.pending.replace(None);
    }

    let base_state = hook.base_state.get();
    match &base_queue {
        None => {
            // If there are no pending updates, then the memoized state should be the
            // same as the base state. Currently these only diverge in the case of
            // useOptimistic, because useOptimistic accepts a new baseState on
            // every render.
            hook.memoized_state.set(base_state);
            // We don't need to call markWorkInProgressReceivedUpdate because
            // baseState is derived from other reactive values.
        }
        Some(base_queue) => {
            // We have a queue to process.
            let first = base_queue.next.borrow().clone().unwrap();
            let mut new_state = base_state;

            let mut new_base_state = js::null();
            let mut new_base_queue_first: Option<Rc<Update>> = None;
            let mut new_base_queue_last: Option<Rc<Update>> = None;
            let mut update = Rc::clone(&first);
            let mut did_read_from_entangled_async_action = false;
            let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
            loop {
                // An extra OffscreenLane bit is added to updates that were made to
                // a hidden tree, so that we can distinguish them from updates that were
                // already there when the tree was hidden.
                let update_lane = remove_lanes(update.lane.get(), OFFSCREEN_LANE);
                let is_hidden_update = update_lane != update.lane.get();

                // Check if this update was made while the tree was hidden. If so, then
                // it's not a "base" update and we should disregard the extra base lanes
                // that were added to renderLanes when we entered the Offscreen tree.
                let mut should_skip_update = if is_hidden_update {
                    !is_subset_of_lanes(get_work_in_progress_root_render_lanes(), update_lane)
                } else {
                    !is_subset_of_lanes(RENDER_LANES.get(), update_lane)
                };

                if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) && update_lane == GESTURE_LANE
                {
                    // This is a gesture optimistic update. It should only be considered as part of the
                    // rendered state while rendering the gesture lane and if the rendering the associated
                    // ScheduledGesture.
                    let scheduled_gesture = update.gesture.borrow().clone();
                    if let Some(scheduled_gesture) = scheduled_gesture {
                        if scheduled_gesture.count.get() == 0 && !scheduled_gesture.committing.get()
                        {
                            // This gesture has already been cancelled. We can clean up this update.
                            let next = update.next.borrow().clone();
                            match next {
                                Some(next) if !Rc::ptr_eq(&next, &first) => update = next,
                                _ => break,
                            }
                            continue;
                        } else if !is_gesture_render(RENDER_LANES.get()) {
                            should_skip_update = true;
                        } else {
                            let root = get_work_in_progress_root();
                            let Some(root) = root else {
                                panic!(
                                    "Expected a work-in-progress root. This is a bug in React. Please file an issue."
                                );
                            };
                            // We assume that the currently rendering gesture is the one first in the queue.
                            should_skip_update =
                                !root.pending_gestures.borrow().as_ref().is_some_and(
                                    |pending_gestures| {
                                        Rc::ptr_eq(pending_gestures, &scheduled_gesture)
                                    },
                                );
                        }
                    }
                }

                if should_skip_update {
                    // Priority is insufficient. Skip this update. If this is the first
                    // skipped update, the previous update/state is the new base
                    // update/state.
                    let clone = Rc::new(Update {
                        lane: Cell::new(update_lane),
                        revert_lane: Cell::new(update.revert_lane.get()),
                        gesture: RefCell::new(update.gesture.borrow().clone()),
                        action: update.action,
                        has_eager_state: Cell::new(update.has_eager_state.get()),
                        eager_state: Cell::new(update.eager_state.get()),
                        next: RefCell::new(js::null()),
                    });
                    match &new_base_queue_last {
                        None => {
                            new_base_queue_first = Some(Rc::clone(&clone));
                            new_base_queue_last = Some(clone);
                            new_base_state = new_state;
                        }
                        Some(last) => {
                            last.next.replace(Some(Rc::clone(&clone)));
                            new_base_queue_last = Some(clone);
                        }
                    }
                    // Update the remaining priority in the queue.
                    // TODO: Don't need to accumulate this. Instead, we can remove
                    // renderLanes from the original lanes.
                    rendering_fiber
                        .lanes
                        .set(merge_lanes(rendering_fiber.lanes.get(), update_lane));
                    mark_skipped_update_lanes(update_lane);
                } else {
                    // This update does have sufficient priority.

                    // Check if this is an optimistic update.
                    let revert_lane = update.revert_lane.get();
                    if revert_lane == NO_LANE {
                        // This is not an optimistic update, and we're going to apply it now.
                        // But, if there were earlier updates that were skipped, we need to
                        // leave this update in the queue so it can be rebased later.
                        if let Some(last) = &new_base_queue_last {
                            let clone = Rc::new(Update {
                                // This update is going to be committed so we never want uncommit
                                // it. Using NoLane works because 0 is a subset of all bitmasks, so
                                // this will never be skipped by the check above.
                                lane: Cell::new(NO_LANE),
                                revert_lane: Cell::new(NO_LANE),
                                gesture: RefCell::new(js::null()),
                                action: update.action,
                                has_eager_state: Cell::new(update.has_eager_state.get()),
                                eager_state: Cell::new(update.eager_state.get()),
                                next: RefCell::new(js::null()),
                            });
                            last.next.replace(Some(Rc::clone(&clone)));
                            new_base_queue_last = Some(clone);
                        }

                        // Check if this update is part of a pending async action. If so,
                        // we'll need to suspend until the action has finished, so that it's
                        // batched together with future updates in the same action.
                        if update_lane == peek_entangled_action_lane() {
                            did_read_from_entangled_async_action = true;
                        }
                    } else {
                        // This is an optimistic update. If the "revert" priority is
                        // sufficient, don't apply the update. Otherwise, apply the update,
                        // but leave it in the queue so it can be either reverted or
                        // rebased in a subsequent render.
                        if is_subset_of_lanes(RENDER_LANES.get(), revert_lane) {
                            // The transition that this optimistic update is associated with
                            // has finished. Pretend the update doesn't exist by skipping
                            // over it.

                            // Check if this update is part of a pending async action. If so,
                            // we'll need to suspend until the action has finished, so that it's
                            // batched together with future updates in the same action.
                            if revert_lane == peek_entangled_action_lane() {
                                did_read_from_entangled_async_action = true;
                            }
                            let next = update.next.borrow().clone();
                            match next {
                                Some(next) if !Rc::ptr_eq(&next, &first) => update = next,
                                _ => break,
                            }
                            continue;
                        } else {
                            let clone = Rc::new(Update {
                                // Once we commit an optimistic update, we shouldn't uncommit it
                                // until the transition it is associated with has finished
                                // (represented by revertLane). Using NoLane here works because 0
                                // is a subset of all bitmasks, so this will never be skipped by
                                // the check above.
                                lane: Cell::new(NO_LANE),
                                // Reuse the same revertLane so we know when the transition
                                // has finished.
                                revert_lane: Cell::new(update.revert_lane.get()),
                                gesture: RefCell::new(js::null()), // If it commits, it's no longer a gesture update.
                                action: update.action,
                                has_eager_state: Cell::new(update.has_eager_state.get()),
                                eager_state: Cell::new(update.eager_state.get()),
                                next: RefCell::new(js::null()),
                            });
                            match &new_base_queue_last {
                                None => {
                                    new_base_queue_first = Some(Rc::clone(&clone));
                                    new_base_queue_last = Some(clone);
                                    new_base_state = new_state;
                                }
                                Some(last) => {
                                    last.next.replace(Some(Rc::clone(&clone)));
                                    new_base_queue_last = Some(clone);
                                }
                            }
                            // Update the remaining priority in the queue.
                            // TODO: Don't need to accumulate this. Instead, we can remove
                            // renderLanes from the original lanes.
                            rendering_fiber
                                .lanes
                                .set(merge_lanes(rendering_fiber.lanes.get(), revert_lane));
                            mark_skipped_update_lanes(revert_lane);
                        }
                    }

                    // Process this update.
                    let action = update.action;
                    if SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV.get() {
                        reducer(new_state, action);
                    }
                    if update.has_eager_state.get() {
                        // If this update is a state update (not a reducer) and was processed eagerly,
                        // we can use the eagerly computed state
                        new_state = update.eager_state.get();
                    } else {
                        new_state = reducer(new_state, action);
                    }
                }
                let next = update.next.borrow().clone();
                // $FlowFixMe[invalid-compare]
                match next {
                    Some(next) if !Rc::ptr_eq(&next, &first) => update = next,
                    _ => break,
                }
            }

            match &new_base_queue_last {
                None => new_base_state = new_state,
                Some(last) => {
                    last.next.replace(new_base_queue_first);
                }
            }

            // Mark that the fiber performed work, but only if the new state is
            // different from the current state.
            if !OBJECT_IS.with(|is| is(&new_state, &hook.memoized_state.get())) {
                mark_work_in_progress_received_update();

                // Check if this update is part of a pending async action. If so, we'll
                // need to suspend until the action has finished, so that it's batched
                // together with future updates in the same action.
                // TODO: Once we support hooks inside useMemo (or an equivalent
                // memoization boundary like Forget), hoist this logic so that it only
                // suspends if the memo boundary produces a new value.
                if did_read_from_entangled_async_action
                    && let Some(entangled_action_thenable) = peek_entangled_action_thenable()
                {
                    // TODO: Instead of the throwing the thenable directly, throw a
                    // special object like `use` does so we can detect if it's captured
                    // by userspace.
                    js::throw(Some(js::unknown(entangled_action_thenable)));
                }
            }

            hook.memoized_state.set(new_state);
            hook.base_state.set(new_base_state);
            hook.base_queue.replace(new_base_queue_last);

            queue.last_rendered_state.set(new_state);
        }
    }

    if base_queue.is_none() {
        // `queue.lanes` is used for entangling transitions. We can set it back to
        // zero once the queue is empty.
        queue.lanes.set(NO_LANES);
    }

    let dispatch = queue.dispatch.borrow().clone();
    (hook.memoized_state.get(), dispatch)
}

fn rerender_reducer(
    reducer: &'static Reducer,
    _initial_arg: Option<&'static Unknown>,
    _init: Option<&'static Unknown>,
) -> StateAndDispatch {
    let hook = update_work_in_progress_hook();
    let queue: Option<&UpdateQueue> = unsafe { js::cast(hook.queue.get()) };

    let Some(queue) = queue else {
        panic!(concat!(
            "Should have a queue. You are likely calling Hooks conditionally, ",
            "which is not allowed. (https://react.dev/link/invalid-hook-call)"
        ));
    };

    queue.last_rendered_reducer.set(Some(reducer));

    // This is a re-render. Apply the new render phase updates to the previous
    // work-in-progress hook.
    let dispatch = queue.dispatch.borrow().clone();
    let last_render_phase_update = queue.pending.borrow().clone();
    let mut new_state = hook.memoized_state.get();
    if let Some(last_render_phase_update) = last_render_phase_update {
        // The queue doesn't persist past this render pass.
        queue.pending.replace(None);

        let first_render_phase_update = last_render_phase_update.next.borrow().clone().unwrap();
        let mut update = Rc::clone(&first_render_phase_update);
        loop {
            // Process this render phase update. We don't have to check the
            // priority because it will always be the same as the current
            // render's.
            let action = update.action;
            new_state = reducer(new_state, action);
            let next = update.next.borrow().clone().unwrap();
            update = next;
            if Rc::ptr_eq(&update, &first_render_phase_update) {
                break;
            }
        }

        // Mark that the fiber performed work, but only if the new state is
        // different from the current state.
        if !OBJECT_IS.with(|is| is(&new_state, &hook.memoized_state.get())) {
            mark_work_in_progress_received_update();
        }

        hook.memoized_state.set(new_state);
        // Don't persist the state accumulated from the render phase updates to
        // the base state unless the queue is empty.
        // TODO: Not sure if this is the desired semantics, but it's what we
        // do for gDSFP. I can't remember why.
        if hook.base_queue.borrow().is_none() {
            hook.base_state.set(new_state);
        }

        queue.last_rendered_state.set(new_state);
    }
    (new_state, dispatch)
}

fn mount_sync_external_store(
    subscribe: &'static Subscribe,
    get_snapshot: &'static GetSnapshot,
    // A function, or undefined where none was given.
    get_server_snapshot: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    let hook = mount_work_in_progress_hook();

    let next_snapshot;
    let is_hydrating = get_is_hydrating();
    if is_hydrating {
        if matches!(js::classify(get_server_snapshot), Kind::Undefined) {
            panic!(concat!(
                "Missing getServerSnapshot, which is required for ",
                "server-rendered content. Will revert to client rendering."
            ));
        }
        let get_server_snapshot: &GetSnapshot = unsafe { js::cast(get_server_snapshot) };
        next_snapshot = get_server_snapshot();
        if __DEV__
            && !DID_WARN_UNCACHED_GET_SNAPSHOT.get()
            && !js::strict_equals(next_snapshot, get_server_snapshot())
        {
            console::error((
                "The result of getServerSnapshot should be cached to avoid an infinite loop",
            ));
            DID_WARN_UNCACHED_GET_SNAPSHOT.set(true);
        }
    } else {
        next_snapshot = get_snapshot();
        if __DEV__ && !DID_WARN_UNCACHED_GET_SNAPSHOT.get() {
            let cached_snapshot = get_snapshot();
            if !OBJECT_IS.with(|is| is(&next_snapshot, &cached_snapshot)) {
                console::error((
                    "The result of getSnapshot should be cached to avoid an infinite loop",
                ));
                DID_WARN_UNCACHED_GET_SNAPSHOT.set(true);
            }
        }
        // Unless we're rendering a blocking lane, schedule a consistency check.
        // Right before committing, we will walk the tree and check if any of the
        // stores were mutated.
        //
        // We won't do this if we're hydrating server-rendered content, because if
        // the content is stale, it's already visible anyway. Instead we'll patch
        // it up in a passive effect.
        let root = get_work_in_progress_root();

        if root.is_none() {
            panic!(
                "Expected a work-in-progress root. This is a bug in React. Please file an issue."
            );
        }

        let root_render_lanes = get_work_in_progress_root_render_lanes();
        if !includes_blocking_lane(root_render_lanes) {
            push_store_consistency_check(&fiber, get_snapshot, next_snapshot);
        }
    }

    // Read the current snapshot from the store on every render. This breaks the
    // normal rules of React, and only works because store updates are
    // always synchronous.
    hook.memoized_state.set(next_snapshot);
    let inst = Rc::new(StoreInstance {
        value: Cell::new(next_snapshot),
        get_snapshot: Cell::new(get_snapshot),
    });
    hook.queue.set(Some(js::unknown_of(Rc::clone(&inst))));

    // Schedule an effect to subscribe to the store.
    mount_effect(
        Rc::from(function::bind(
            subscribe_to_store,
            Some(&Kind::Null),
            (Rc::clone(&fiber), Rc::clone(&inst), subscribe),
        )),
        Some(vec![Some(js::unknown(subscribe))]),
    );

    // Schedule an effect to update the mutable instance fields. We will update
    // this whenever subscribe, getSnapshot, or value changes. Because there's no
    // clean-up function, and we track the deps correctly, we can call pushEffect
    // directly, without storing any additional state. For the same reason, we
    // don't need to set a static flag, either.
    fiber.flags.set(fiber.flags.get() | PASSIVE_EFFECT);
    push_simple_effect(
        HOOK_HAS_EFFECT | HOOK_PASSIVE,
        create_effect_instance(),
        {
            let fiber = Rc::clone(&fiber);
            Rc::new(move || {
                update_store_instance(&fiber, &inst, next_snapshot, get_snapshot);
                None
            })
        },
        None,
    );

    next_snapshot
}

fn update_sync_external_store(
    subscribe: &'static Subscribe,
    get_snapshot: &'static GetSnapshot,
    // A function, or undefined where none was given.
    get_server_snapshot: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    let hook = update_work_in_progress_hook();
    // Read the current snapshot from the store on every render. This breaks the
    // normal rules of React, and only works because store updates are
    // always synchronous.
    let next_snapshot;
    let is_hydrating = get_is_hydrating();
    if is_hydrating {
        // Needed for strict mode double render
        if matches!(js::classify(get_server_snapshot), Kind::Undefined) {
            panic!(concat!(
                "Missing getServerSnapshot, which is required for ",
                "server-rendered content. Will revert to client rendering."
            ));
        }
        let get_server_snapshot: &GetSnapshot = unsafe { js::cast(get_server_snapshot) };
        next_snapshot = get_server_snapshot();
    } else {
        next_snapshot = get_snapshot();
        if __DEV__ && !DID_WARN_UNCACHED_GET_SNAPSHOT.get() {
            let cached_snapshot = get_snapshot();
            if !OBJECT_IS.with(|is| is(&next_snapshot, &cached_snapshot)) {
                console::error((
                    "The result of getSnapshot should be cached to avoid an infinite loop",
                ));
                DID_WARN_UNCACHED_GET_SNAPSHOT.set(true);
            }
        }
    }
    let prev_snapshot = CURRENT_HOOK
        .with_borrow(Clone::clone)
        .unwrap_or_else(|| Rc::clone(&hook))
        .memoized_state
        .get();
    let snapshot_changed = !OBJECT_IS.with(|is| is(&prev_snapshot, &next_snapshot));
    if snapshot_changed {
        hook.memoized_state.set(next_snapshot);
        mark_work_in_progress_received_update();
    }
    let inst: Rc<StoreInstance> = unsafe { js::cast(hook.queue.get()) };

    update_effect(
        Rc::from(function::bind(
            subscribe_to_store,
            Some(&Kind::Null),
            (Rc::clone(&fiber), Rc::clone(&inst), subscribe),
        )),
        Some(vec![Some(js::unknown(subscribe))]),
    );

    // Whenever getSnapshot or subscribe changes, we need to check in the
    // commit phase if there was an interleaved mutation. In concurrent mode
    // this can happen all the time, but even in synchronous mode, an earlier
    // effect may have mutated the store.
    let store_changed = !js::strict_equals(js::unknown(inst.get_snapshot.get()), js::unknown(get_snapshot))
        || snapshot_changed
        // Check if the subscribe function changed. We can save some memory by
        // checking whether we scheduled a subscription effect above.
        || WORK_IN_PROGRESS_HOOK
            .with_borrow(Clone::clone)
            .is_some_and(|work_in_progress_hook| {
                (unsafe { js::cast::<&Effect>(work_in_progress_hook.memoized_state.get()) }.tag
                    & HOOK_HAS_EFFECT)
                    != HOOK_NO_FLAGS
            });

    // Even if nothing changed during this render, we push the effect so it is
    // always in the effect list. That way it re-runs whenever the passive
    // effects are reconnected, like when a hidden Activity tree is shown again.
    // While the tree was hidden we were not subscribed to the store, so
    // mutations during that window notified nobody, and if the reveal didn't
    // re-render this component (or rendered before the mutation), nothing
    // would ever detect them. When nothing changed, the effect is pushed
    // without the HasEffect tag so a regular commit skips it.
    push_simple_effect(
        if store_changed {
            HOOK_HAS_EFFECT | HOOK_PASSIVE
        } else {
            HOOK_PASSIVE
        },
        create_effect_instance(),
        {
            let fiber = Rc::clone(&fiber);
            Rc::new(move || {
                update_store_instance(&fiber, &inst, next_snapshot, get_snapshot);
                None
            })
        },
        None,
    );

    if store_changed {
        fiber.flags.set(fiber.flags.get() | PASSIVE_EFFECT);

        // Unless we're rendering a blocking lane, schedule a consistency check.
        // Right before committing, we will walk the tree and check if any of the
        // stores were mutated.
        let root = get_work_in_progress_root();

        if root.is_none() {
            panic!(
                "Expected a work-in-progress root. This is a bug in React. Please file an issue."
            );
        }

        if !is_hydrating && !includes_blocking_lane(RENDER_LANES.get()) {
            push_store_consistency_check(&fiber, get_snapshot, next_snapshot);
        }
    }

    next_snapshot
}

fn push_store_consistency_check(
    fiber: &Fiber,
    get_snapshot: &'static GetSnapshot,
    rendered_snapshot: Option<&'static Unknown>,
) {
    fiber.flags.set(fiber.flags.get() | STORE_CONSISTENCY);
    let check = StoreConsistencyCheck {
        get_snapshot,
        value: rendered_snapshot,
    };
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    let component_update_queue: Option<Rc<FunctionComponentUpdateQueue>> =
        unsafe { js::cast(rendering_fiber.update_queue.get()) };
    match component_update_queue {
        None => {
            let component_update_queue = create_function_component_update_queue();
            rendering_fiber
                .update_queue
                .set(Some(js::unknown_of(Rc::clone(&component_update_queue))));
            component_update_queue
                .stores
                .replace(Some(Rc::new(RefCell::new(vec![check]))));
        }
        Some(component_update_queue) => {
            let stores = component_update_queue.stores.borrow().clone();
            match stores {
                None => {
                    component_update_queue
                        .stores
                        .replace(Some(Rc::new(RefCell::new(vec![check]))));
                }
                Some(stores) => stores.borrow_mut().push(check),
            }
        }
    }
}

fn update_store_instance(
    fiber: &Rc<Fiber>,
    inst: &StoreInstance,
    next_snapshot: Option<&'static Unknown>,
    get_snapshot: &'static GetSnapshot,
) {
    // These are updated in the passive phase
    inst.value.set(next_snapshot);
    inst.get_snapshot.set(get_snapshot);

    // Something may have been mutated in between render and commit. This could
    // have been in an event that fired before the passive effects, or it could
    // have been in a layout effect. In that case, we would have used the old
    // snapshot and getSnapshot values to bail out. We need to check one more
    // time. This effect also re-runs when a hidden Activity tree is revealed.
    if check_if_snapshot_changed(inst) {
        // Force a re-render.
        // We intentionally don't log update times and stacks here because this
        // was not an external trigger but rather an internal one.
        force_store_rerender(fiber);
    }
}

fn subscribe_to_store(
    fiber: &Rc<Fiber>,
    inst: &Rc<StoreInstance>,
    subscribe: &Subscribe,
) -> Destroy {
    let handle_store_change = {
        let fiber = Rc::clone(fiber);
        let inst = Rc::clone(inst);
        move || {
            // The store changed. Check if the snapshot changed since the last time we
            // read from the store.
            if check_if_snapshot_changed(&inst) {
                // Force a re-render.
                start_update_timer_by_lane(SYNC_LANE, "updateSyncExternalStore()", Some(&fiber));
                force_store_rerender(&fiber);
            }
        }
    };
    // Subscribe to the store and return a clean-up function.
    subscribe(Rc::new(handle_store_change))
}

fn check_if_snapshot_changed(inst: &StoreInstance) -> bool {
    let latest_get_snapshot = inst.get_snapshot.get();
    let prev_value = inst.value.get();
    match panic::catch_unwind(AssertUnwindSafe(latest_get_snapshot)) {
        Ok(next_value) => !OBJECT_IS.with(|is| is(&prev_value, &next_value)),
        Err(_) => true,
    }
}

fn force_store_rerender(fiber: &Rc<Fiber>) {
    let root = enqueue_concurrent_render_for_lane(fiber, SYNC_LANE);
    if let Some(root) = root {
        schedule_update_on_fiber(&root, fiber, SYNC_LANE);
    }
}

fn mount_state_impl(initial_state: Option<&'static Unknown>) -> Rc<Hook> {
    let hook = mount_work_in_progress_hook();
    let mut initial_state = initial_state;
    if let Kind::Function(initial_state_initializer) = js::classify(initial_state) {
        // $FlowFixMe[incompatible-use]: Flow doesn't like mixed types
        initial_state = initial_state_initializer();
        if SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV.get() {
            set_is_strict_mode_for_devtools(true);
            let _exit = ExitStrictModeForDevtools;
            // $FlowFixMe[incompatible-use]: Flow doesn't like mixed types
            initial_state_initializer();
        }
    }
    hook.memoized_state.set(initial_state);
    hook.base_state.set(initial_state);
    let queue = Rc::new(UpdateQueue {
        pending: RefCell::new(None),
        lanes: Cell::new(NO_LANES),
        dispatch: RefCell::new(None),
        last_rendered_reducer: Cell::new(Some(&basic_state_reducer)),
        last_rendered_state: Cell::new(initial_state),
    });
    hook.queue.set(Some(js::unknown_of(queue)));
    hook
}

fn mount_state(initial_state: Option<&'static Unknown>) -> StateAndDispatch {
    let hook = mount_state_impl(initial_state);
    let queue: Rc<UpdateQueue> = unsafe { js::cast(hook.queue.get()) };
    let dispatch: Rc<Dispatch> = Rc::from(function::bind(
        dispatch_set_state,
        Some(&Kind::Null),
        (
            CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap(),
            Rc::clone(&queue),
        ),
    ));
    queue.dispatch.replace(Some(Rc::clone(&dispatch)));
    (hook.memoized_state.get(), Some(dispatch))
}

fn update_state(initial_state: Option<&'static Unknown>) -> StateAndDispatch {
    update_reducer(&basic_state_reducer, initial_state, None)
}

fn rerender_state(initial_state: Option<&'static Unknown>) -> StateAndDispatch {
    rerender_reducer(&basic_state_reducer, initial_state, None)
}

fn mount_optimistic(
    passthrough: Option<&'static Unknown>,
    _reducer: Option<&'static Unknown>,
) -> StateAndDispatch {
    let hook = mount_work_in_progress_hook();
    hook.memoized_state.set(passthrough);
    hook.base_state.set(passthrough);
    let queue = Rc::new(UpdateQueue {
        pending: RefCell::new(None),
        lanes: Cell::new(NO_LANES),
        dispatch: RefCell::new(None),
        // Optimistic state does not use the eager update optimization.
        last_rendered_reducer: Cell::new(None),
        last_rendered_state: Cell::new(js::null()),
    });
    hook.queue.set(Some(js::unknown_of(Rc::clone(&queue))));
    // This is different than the normal setState function.
    let dispatch: Rc<Dispatch> = Rc::from(function::bind(
        dispatch_optimistic_set_state,
        Some(&Kind::Null),
        (
            CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap(),
            true,
            Rc::clone(&queue),
        ),
    ));
    queue.dispatch.replace(Some(Rc::clone(&dispatch)));
    (passthrough, Some(dispatch))
}

fn update_optimistic(
    passthrough: Option<&'static Unknown>,
    reducer: Option<&'static Unknown>,
) -> StateAndDispatch {
    let hook = update_work_in_progress_hook();
    let current_hook = CURRENT_HOOK.with_borrow(Clone::clone);
    update_optimistic_impl(&hook, current_hook.as_deref(), passthrough, reducer)
}

fn update_optimistic_impl(
    hook: &Hook,
    _current: Option<&Hook>,
    passthrough: Option<&'static Unknown>,
    reducer: Option<&'static Unknown>,
) -> StateAndDispatch {
    // Optimistic updates are always rebased on top of the latest value passed in
    // as an argument. It's called a passthrough because if there are no pending
    // updates, it will be returned as-is.
    //
    // Reset the base state to the passthrough. Future updates will be applied
    // on top of this.
    hook.base_state.set(passthrough);

    // If a reducer is not provided, default to the same one used by useState.
    let resolved_reducer: &'static Reducer = if js::type_of(reducer) == "function" {
        unsafe { js::cast(reducer) }
    } else {
        &basic_state_reducer
    };

    let current_hook = CURRENT_HOOK.with_borrow(Clone::clone);
    update_reducer_impl(hook, current_hook.as_deref(), resolved_reducer)
}

fn rerender_optimistic(
    passthrough: Option<&'static Unknown>,
    reducer: Option<&'static Unknown>,
) -> StateAndDispatch {
    // Unlike useState, useOptimistic doesn't support render phase updates.
    // Also unlike useState, we need to replay all pending updates again in case
    // the passthrough value changed.
    //
    // So instead of a forked re-render implementation that knows how to handle
    // render phase udpates, we can use the same implementation as during a
    // regular mount or update.
    let hook = update_work_in_progress_hook();

    if let Some(current_hook) = CURRENT_HOOK.with_borrow(Clone::clone) {
        // This is an update. Process the update queue.
        return update_optimistic_impl(&hook, Some(&current_hook), passthrough, reducer);
    }

    // This is a mount. No updates to process.

    // Reset the base state to the passthrough. Future updates will be applied
    // on top of this.
    hook.base_state.set(passthrough);
    let queue: &UpdateQueue = unsafe { js::cast(hook.queue.get()) };
    let dispatch = queue.dispatch.borrow().clone();
    (passthrough, dispatch)
}

/// What `useActionState` runs: the next state, of the previous one and the
/// payload.
pub type Action = Reducer;

// useActionState actions run sequentially, because each action receives the
// previous state as an argument. We store pending actions on a queue.
struct ActionStateQueue {
    // This is the most recent state returned from an action. It's updated as
    // soon as the action finishes running.
    state: Cell<Option<&'static Unknown>>,
    // A stable dispatch method, passed to the user.
    dispatch: RefCell<Option<Rc<ActionDispatch>>>,
    // This is the most recent action function that was rendered. It's updated
    // during the commit phase.
    // If it's null, it means the action queue errored and subsequent actions
    // should not run.
    #[cfg_attr(rust_js, rust_js::nullable)]
    action: Cell<Option<&'static Action>>,
    // This is a circular linked list of pending action payloads. It incudes the
    // action that is currently running.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pending: RefCell<Option<Rc<ActionStateQueueNode>>>,
}

#[cfg_attr(rust_js, rust_js::object)]
pub struct ActionStateQueueNode {
    payload: Option<&'static Unknown>,
    // This is the action implementation at the time it was dispatched.
    action: &'static Action,
    // This is never null because it's part of a circular linked list.
    next: RefCell<Option<Rc<ActionStateQueueNode>>>,

    // Whether or not the action was dispatched as part of a transition. We use
    // this to restore the transition context when the queued action is run. Once
    // we're able to track parallel async actions, this should be updated to
    // represent the specific transition instance the action is associated with.
    is_transition: Cell<bool>,

    // Implements the Thenable interface. We use it to suspend until the action
    // finishes.
    status: Cell<&'static str>,
    value: Cell<Option<&'static Unknown>>,
    reason: Cell<Option<&'static Unknown>>,
    listeners: RefCell<Vec<&'static dyn Fn()>>,
}

unsafe impl js::Defined for ActionStateQueue {}
unsafe impl js::Defined for ActionStateQueueNode {}

#[cfg_attr(rust_js, rust_js::object)]
impl ActionStateQueueNode {
    pub fn then(&self, listener: &'static dyn Fn()) {
        // We know the only thing that subscribes to these promises is `use` so
        // this implementation is simpler than a generic thenable. E.g. we don't
        // bother to check if the thenable is still pending because `use` already
        // does that.
        self.listeners.borrow_mut().push(listener);
    }
}

fn dispatch_action_state(
    fiber: &Rc<Fiber>,
    action_queue: &Rc<ActionStateQueue>,
    set_pending_state: &Rc<Dispatch>,
    set_state: &Rc<Dispatch>,
    payload: Option<&'static Unknown>,
) {
    if is_render_phase_update(fiber) {
        panic!("Cannot update action state while rendering.");
    }

    let current_action = action_queue.action.get();
    let Some(current_action) = current_action else {
        // An earlier action errored. Subsequent actions should not run.
        return;
    };

    let action_node = Rc::new(ActionStateQueueNode {
        payload,
        action: current_action,
        next: RefCell::new(js::null()), // circular
        is_transition: Cell::new(true),

        status: Cell::new("pending"),
        value: Cell::new(js::null()),
        reason: Cell::new(js::null()),
        listeners: RefCell::new(Vec::new()),
    });

    // Check if we're inside a transition. If so, we'll need to restore the
    // transition context when the action is run.
    let prev_transition = t(REACT_SHARED_INTERNALS);
    if prev_transition.is_some() {
        // Optimistically update the pending state, similar to useTransition.
        // This will be reverted automatically when all actions are finished.
        set_pending_state(Some(js::unknown(&true)), None);
        // `actionNode` is a thenable that resolves to the return value of
        // the action.
        set_state(Some(js::unknown_of(Rc::clone(&action_node))), None);
    } else {
        // This is not a transition.
        action_node.is_transition.set(false);
        set_state(Some(js::unknown_of(Rc::clone(&action_node))), None);
    }

    let last = action_queue.pending.borrow().clone();
    match last {
        None => {
            // There are no pending actions; this is the first one. We can run
            // it immediately.
            action_node.next.replace(Some(Rc::clone(&action_node)));
            action_queue.pending.replace(Some(Rc::clone(&action_node)));
            run_action_state_action(action_queue, &action_node);
        }
        Some(last) => {
            // There's already an action running. Add to the queue.
            let first = last.next.borrow().clone();
            action_node.next.replace(first);
            last.next.replace(Some(Rc::clone(&action_node)));
            action_queue.pending.replace(Some(action_node));
        }
    }
}

/// What `runActionStateAction`'s fork of `startTransition` does in its
/// `finally`: the outer Transition back, after the inner one's types are
/// handed to it.
struct FinishActionStateTransition<'a> {
    prev_transition: Option<&'static Transition>,
    current_transition: &'a Transition,
}

impl Drop for FinishActionStateTransition<'_> {
    fn drop(&mut self) {
        let current_types = self.current_transition.types.borrow().clone();
        if let Some(prev_transition) = self.prev_transition
            && let Some(current_types) = current_types
        {
            // If we created a new types set in the inner transition, we transfer it to the parent
            // since they should share the same set. They're conceptually entangled.
            let prev_types = prev_transition.types.borrow().clone();
            if __DEV__
                && let Some(prev_types) = prev_types
                && !Rc::ptr_eq(&prev_types, &current_types)
            {
                // Just assert that assumption holds that we're not overriding anything.
                console::error((concat!(
                    "We expected inner Transitions to have transferred the outer types set and ",
                    "that you cannot add to the outer Transition while inside the inner.",
                    "This is a bug in React."
                ),));
            }
            prev_transition.types.replace(Some(current_types));
        }
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);

        if __DEV__
            && self.prev_transition.is_none()
            && let Some(updated_fibers) =
                self.current_transition.updated_fibers.borrow_mut().as_mut()
        {
            let updated_fibers_count = updated_fibers.len();
            updated_fibers.clear();
            if updated_fibers_count > 10 {
                console::warn((concat!(
                    "Detected a large number of updates inside startTransition. ",
                    "If this is due to a subscription please re-write it to use React provided hooks. ",
                    "Otherwise concurrent mode guarantees are off the table."
                ),));
            }
        }
    }
}

fn run_action_state_action(action_queue: &Rc<ActionStateQueue>, node: &Rc<ActionStateQueueNode>) {
    // `node.action` represents the action function at the time it was dispatched.
    // If this action was queued, it might be stale, i.e. it's not necessarily the
    // most current implementation of the action, stored on `actionQueue`. This is
    // intentional. The conceptual model for queued actions is that they are
    // queued in a remote worker; the dispatch happens immediately, only the
    // execution is delayed.
    let action = node.action;
    let payload = node.payload;
    let prev_state = action_queue.state.get();

    if node.is_transition.get() {
        // The original dispatch was part of a transition. We restore its
        // transition context here.

        // This is a fork of startTransition
        let prev_transition = t(REACT_SHARED_INTERNALS);
        let mut current_transition = Transition {
            types: RefCell::new(None),
            gesture: None,
            name: None,
            start_time: Cell::new(None),
            updated_fibers: RefCell::new(None),
        };
        if ENABLE_VIEW_TRANSITION {
            *current_transition.types.get_mut() = match prev_transition {
                // If we're a nested transition, we should use the same set as the parent
                // since we're conceptually always joined into the same entangled transition.
                // In practice, this only matters if we add transition types in the inner
                // without setting state. In that case, the inner transition can finish
                // without waiting for the outer.
                Some(prev_transition) => prev_transition.types.borrow().clone(),
                None => None,
            };
        }
        if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
            current_transition.gesture = None;
        }
        if ENABLE_TRANSITION_TRACING {
            current_transition.name = None;
            current_transition.start_time.set(Some(-1.0));
        }
        if __DEV__ {
            *current_transition.updated_fibers.get_mut() = Some(HashSet::new());
        }
        set_t(REACT_SHARED_INTERNALS, Some(&current_transition));
        let _finish = FinishActionStateTransition {
            prev_transition,
            current_transition: &current_transition,
        };
        if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
            let return_value = action(prev_state, payload);
            if let Some(on_start_transition_finish) = s(REACT_SHARED_INTERNALS) {
                on_start_transition_finish(&current_transition, return_value);
            }
            handle_action_return_value(action_queue, node, return_value);
        })) {
            on_action_error(
                action_queue,
                node,
                error
                    .downcast_ref::<&'static JsError>()
                    .and_then(|error| error.as_unknown()),
            );
        }
    } else {
        // The original dispatch was not part of a transition.
        if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
            let return_value = action(prev_state, payload);
            handle_action_return_value(action_queue, node, return_value);
        })) {
            on_action_error(
                action_queue,
                node,
                error
                    .downcast_ref::<&'static JsError>()
                    .and_then(|error| error.as_unknown()),
            );
        }
    }
}

fn handle_action_return_value(
    action_queue: &Rc<ActionStateQueue>,
    node: &Rc<ActionStateQueueNode>,
    return_value: Option<&'static Unknown>,
) {
    if let Some(value) = return_value
        && js::type_of(return_value) == "object"
        // $FlowFixMe[method-unbinding]
        && js::type_of(js::get(value, "then")) == "function"
    {
        let thenable: &'static Thenable = unsafe { js::cast(return_value) };
        if __DEV__ {
            // Keep track of the number of async transitions still running so we can warn.
            REACT_SHARED_INTERNALS
                .set_async_transitions(REACT_SHARED_INTERNALS.async_transitions() + 1.0);
            thenable.then(
                |_| release_async_transition(),
                |_| release_async_transition(),
            );
        }
        // Attach a listener to read the return state of the action. As soon as
        // this resolves, we can run the next action in the sequence.
        thenable.then(
            {
                let action_queue = Rc::clone(action_queue);
                let node = Rc::clone(node);
                move |next_state| {
                    on_action_success(&action_queue, &node, next_state);
                }
            },
            {
                let action_queue = Rc::clone(action_queue);
                let node = Rc::clone(node);
                move |error| on_action_error(&action_queue, &node, error)
            },
        );

        if __DEV__ && !node.is_transition.get() {
            console::error((concat!(
                "An async function with useActionState was called outside of a transition. ",
                "This is likely not what you intended (for example, isPending will not update ",
                "correctly). Either call the returned function inside startTransition, or pass it ",
                "to an `action` or `formAction` prop."
            ),));
        }
    } else {
        let next_state = return_value;
        on_action_success(action_queue, node, next_state);
    }
}

fn on_action_success(
    action_queue: &Rc<ActionStateQueue>,
    action_node: &ActionStateQueueNode,
    next_state: Option<&'static Unknown>,
) {
    // The action finished running.
    action_node.status.set("fulfilled");
    action_node.value.set(next_state);
    notify_action_listeners(action_node);

    action_queue.state.set(next_state);

    // Pop the action from the queue and run the next pending action, if there
    // are any.
    let last = action_queue.pending.borrow().clone();
    if let Some(last) = last {
        let first = last.next.borrow().clone().unwrap();
        if Rc::ptr_eq(&first, &last) {
            // This was the last action in the queue.
            action_queue.pending.replace(None);
        } else {
            // Remove the first node from the circular queue.
            let next = first.next.borrow().clone().unwrap();
            last.next.replace(Some(Rc::clone(&next)));

            // Run the next action.
            run_action_state_action(action_queue, &next);
        }
    }
}

fn on_action_error(
    action_queue: &ActionStateQueue,
    action_node: &Rc<ActionStateQueueNode>,
    error: Option<&'static Unknown>,
) {
    // Mark all the following actions as rejected.
    let last = action_queue.pending.replace(None);
    if let Some(last) = last {
        let first = last.next.borrow().clone().unwrap();
        let mut action_node = Rc::clone(action_node);
        loop {
            action_node.status.set("rejected");
            action_node.reason.set(error);
            notify_action_listeners(&action_node);
            let next = action_node.next.borrow().clone().unwrap();
            action_node = next;
            if Rc::ptr_eq(&action_node, &first) {
                break;
            }
        }
    }

    // Prevent subsequent actions from being dispatched.
    action_queue.action.set(None);
}

fn notify_action_listeners(action_node: &ActionStateQueueNode) {
    // Notify React that the action has finished.
    let listeners = &action_node.listeners;
    let mut i = 0;
    while i < listeners.borrow().len() {
        // This is always a React internal listener, so we don't need to worry
        // about it throwing.
        let listener = listeners.borrow()[i];
        listener();
        i += 1;
    }
}

fn action_state_reducer(
    _old_state: Option<&'static Unknown>,
    new_state: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    new_state
}

fn mount_action_state(
    action: &'static Action,
    initial_state_prop: Option<&'static Unknown>,
    _permalink: Option<String>,
) -> ActionStateAndDispatch {
    let mut initial_state = initial_state_prop;
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    if get_is_hydrating() {
        let root = get_work_in_progress_root().unwrap();
        let ssr_form_state = root.form_state;
        // If a formState option was passed to the root, there are form state
        // markers that we need to hydrate. These indicate whether the form state
        // matches this hook instance.
        if let Some(ssr_form_state) = ssr_form_state {
            let is_matching = try_to_claim_next_hydratable_form_marker_instance(&rendering_fiber);
            if is_matching {
                initial_state = js::get(ssr_form_state, &0usize);
            }
        }
    }

    // State hook. The state is stored in a thenable which is then unwrapped by
    // the `use` algorithm during render.
    let state_hook = mount_work_in_progress_hook();
    state_hook.memoized_state.set(initial_state);
    state_hook.base_state.set(initial_state);
    // TODO: Typing this "correctly" results in recursion limit errors
    // const stateQueue: UpdateQueue<S | Awaited<S>, S | Awaited<S>> = {
    let state_queue = Rc::new(UpdateQueue {
        pending: RefCell::new(None),
        lanes: Cell::new(NO_LANES),
        dispatch: RefCell::new(js::null()),
        last_rendered_reducer: Cell::new(Some(&action_state_reducer)),
        last_rendered_state: Cell::new(initial_state),
    });
    state_hook
        .queue
        .set(Some(js::unknown_of(Rc::clone(&state_queue))));
    let set_state: Rc<Dispatch> = Rc::from(function::bind(
        dispatch_set_state,
        Some(&Kind::Null),
        (Rc::clone(&rendering_fiber), Rc::clone(&state_queue)),
    ));
    state_queue.dispatch.replace(Some(Rc::clone(&set_state)));

    // Pending state. This is used to store the pending state of the action.
    // Tracked optimistically, like a transition pending state.
    let pending_state_hook = mount_state_impl(Some(js::unknown(&false)));
    let set_pending_state: Rc<Dispatch> = Rc::from(function::bind(
        dispatch_optimistic_set_state,
        Some(&Kind::Null),
        (Rc::clone(&rendering_fiber), false, unsafe {
            js::cast::<Rc<UpdateQueue>>(pending_state_hook.queue.get())
        }),
    ));

    // Action queue hook. This is used to queue pending actions. The queue is
    // shared between all instances of the hook. Similar to a regular state queue,
    // but different because the actions are run sequentially, and they run in
    // an event instead of during render.
    let action_queue_hook = mount_work_in_progress_hook();
    let action_queue = Rc::new(ActionStateQueue {
        state: Cell::new(initial_state),
        dispatch: RefCell::new(js::null()), // circular
        action: Cell::new(Some(action)),
        pending: RefCell::new(None),
    });
    action_queue_hook
        .queue
        .set(Some(js::unknown_of(Rc::clone(&action_queue))));
    let dispatch: Rc<ActionDispatch> = Rc::from(function::bind(
        dispatch_action_state,
        Some(&Kind::Null),
        (
            Rc::clone(&rendering_fiber),
            Rc::clone(&action_queue),
            set_pending_state,
            set_state,
        ),
    ));
    action_queue.dispatch.replace(Some(Rc::clone(&dispatch)));

    // Stash the action function on the memoized state of the hook. We'll use this
    // to detect when the action function changes so we can update it in
    // an effect.
    action_queue_hook
        .memoized_state
        .set(Some(js::unknown(action)));

    (initial_state, dispatch, false)
}

fn update_action_state(
    action: &'static Action,
    initial_state: Option<&'static Unknown>,
    permalink: Option<String>,
) -> ActionStateAndDispatch {
    let state_hook = update_work_in_progress_hook();
    let current_state_hook = CURRENT_HOOK.with_borrow(Clone::clone);
    update_action_state_impl(
        &state_hook,
        current_state_hook.as_deref(),
        action,
        initial_state,
        permalink,
    )
}

fn update_action_state_impl(
    state_hook: &Hook,
    current_state_hook: Option<&Hook>,
    action: &'static Action,
    _initial_state: Option<&'static Unknown>,
    _permalink: Option<String>,
) -> ActionStateAndDispatch {
    let (action_result, _) =
        update_reducer_impl(state_hook, current_state_hook, &action_state_reducer);

    let (is_pending, _) = update_state(Some(js::unknown(&false)));

    // This will suspend until the action finishes.
    let state;
    if js::type_of(action_result) == "object"
        // $FlowFixMe[invalid-compare]
        && let Some(result) = action_result
        // $FlowFixMe[method-unbinding]
        && js::type_of(js::get(result, "then")) == "function"
    {
        match panic::catch_unwind(AssertUnwindSafe(|| {
            use_thenable(unsafe { js::cast(action_result) })
        })) {
            Ok(value) => state = value,
            Err(x) => {
                if let Some(&x) = x.downcast_ref::<&'static JsError>()
                    && SUSPENSE_EXCEPTION
                        .with(|exception| js::strict_equals(x.as_unknown(), *exception))
                {
                    // If we Suspend here, mark this separately so that we can track this
                    // as an Action in Profiling tools.
                    SUSPENSE_ACTION_EXCEPTION.with(|exception| js::throw(exception.as_unknown()));
                } else {
                    panic::resume_unwind(x);
                }
            }
        }
    } else {
        state = action_result;
    }

    let action_queue_hook = update_work_in_progress_hook();
    let action_queue: Rc<ActionStateQueue> = unsafe { js::cast(action_queue_hook.queue.get()) };
    let dispatch = action_queue.dispatch.borrow().clone().unwrap();

    // Check if a new action was passed. If so, update it in an effect.
    let prev_action = action_queue_hook.memoized_state.get();
    if !js::strict_equals(js::unknown(action), prev_action) {
        let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
        rendering_fiber
            .flags
            .set(rendering_fiber.flags.get() | PASSIVE_EFFECT);
        push_simple_effect(
            HOOK_HAS_EFFECT | HOOK_PASSIVE,
            create_effect_instance(),
            Rc::new(move || {
                action_state_action_effect(&action_queue, action);
                None
            }),
            None,
        );
    }

    (state, dispatch, unsafe { js::cast(is_pending) })
}

fn action_state_action_effect(action_queue: &ActionStateQueue, action: &'static Action) {
    action_queue.action.set(Some(action));
}

fn rerender_action_state(
    action: &'static Action,
    initial_state: Option<&'static Unknown>,
    permalink: Option<String>,
) -> ActionStateAndDispatch {
    // Unlike useState, useActionState doesn't support render phase updates.
    // Also unlike useState, we need to replay all pending updates again in case
    // the passthrough value changed.
    //
    // So instead of a forked re-render implementation that knows how to handle
    // render phase udpates, we can use the same implementation as during a
    // regular mount or update.
    let state_hook = update_work_in_progress_hook();
    let current_state_hook = CURRENT_HOOK.with_borrow(Clone::clone);

    if let Some(current_state_hook) = current_state_hook {
        // This is an update. Process the update queue.
        return update_action_state_impl(
            &state_hook,
            Some(&current_state_hook),
            action,
            initial_state,
            permalink,
        );
    }

    update_work_in_progress_hook(); // State

    // This is a mount. No updates to process.
    let state = state_hook.memoized_state.get();

    let action_queue_hook = update_work_in_progress_hook();
    let action_queue: &ActionStateQueue = unsafe { js::cast(action_queue_hook.queue.get()) };
    let dispatch = action_queue.dispatch.borrow().clone().unwrap();

    // This may have changed during the rerender.
    action_queue_hook
        .memoized_state
        .set(Some(js::unknown(action)));

    // For mount, pending is always false.
    (state, dispatch, false)
}

fn push_simple_effect(
    tag: HookFlags,
    inst: Rc<EffectInstance>,
    create: Rc<Create>,
    deps: Option<Deps>,
) -> Rc<Effect> {
    let effect = Rc::new(Effect {
        tag,
        create,
        deps,
        inst,
        // Circular
        next: RefCell::new(js::null()),
    });
    push_effect_impl(effect)
}

fn push_effect_impl(effect: Rc<Effect>) -> Rc<Effect> {
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    let component_update_queue: Rc<FunctionComponentUpdateQueue> =
        match unsafe { js::cast(rendering_fiber.update_queue.get()) } {
            Some(component_update_queue) => component_update_queue,
            None => {
                let component_update_queue = create_function_component_update_queue();
                rendering_fiber
                    .update_queue
                    .set(Some(js::unknown_of(Rc::clone(&component_update_queue))));
                component_update_queue
            }
        };
    let last_effect = component_update_queue.last_effect.borrow().clone();
    match last_effect {
        None => {
            effect.next.replace(Some(Rc::clone(&effect)));
            component_update_queue
                .last_effect
                .replace(Some(Rc::clone(&effect)));
        }
        Some(last_effect) => {
            let first_effect = last_effect.next.borrow().clone();
            last_effect.next.replace(Some(Rc::clone(&effect)));
            effect.next.replace(first_effect);
            component_update_queue
                .last_effect
                .replace(Some(Rc::clone(&effect)));
        }
    }
    effect
}

fn create_effect_instance() -> Rc<EffectInstance> {
    Rc::new(EffectInstance {
        destroy: Cell::new(None),
    })
}

fn mount_ref(initial_value: Option<&'static Unknown>) -> Rc<RefObject> {
    let hook = mount_work_in_progress_hook();
    let r#ref = Rc::new(RefObject {
        current: Cell::new(initial_value),
    });
    hook.memoized_state
        .set(Some(js::unknown_of(Rc::clone(&r#ref))));
    r#ref
}

fn update_ref(_initial_value: Option<&'static Unknown>) -> Rc<RefObject> {
    let hook = update_work_in_progress_hook();
    unsafe { js::cast(hook.memoized_state.get()) }
}

fn mount_effect_impl(
    fiber_flags: Flags,
    hook_flags: HookFlags,
    create: Rc<Create>,
    deps: Option<Deps>,
) {
    let hook = mount_work_in_progress_hook();
    let next_deps = match deps {
        None => js::null(),
        deps => deps,
    };
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    rendering_fiber
        .flags
        .set(rendering_fiber.flags.get() | fiber_flags);
    hook.memoized_state
        .set(Some(js::unknown_of(push_simple_effect(
            HOOK_HAS_EFFECT | hook_flags,
            create_effect_instance(),
            create,
            next_deps,
        ))));
}

fn update_effect_impl(
    fiber_flags: Flags,
    hook_flags: HookFlags,
    create: Rc<Create>,
    deps: Option<Deps>,
) {
    let hook = update_work_in_progress_hook();
    let next_deps = match deps {
        None => js::null(),
        deps => deps,
    };
    let effect: &Effect = unsafe { js::cast(hook.memoized_state.get()) };
    let inst = Rc::clone(&effect.inst);

    // currentHook is null on initial mount when rerendering after a render phase
    // state update or for strict mode.
    if let Some(current_hook) = CURRENT_HOOK.with_borrow(Clone::clone)
        && let Some(deps) = &next_deps
    {
        let prev_effect = current_hook.memoized_state.get();
        let prev_deps = js::get(prev_effect, "deps");
        // $FlowFixMe[incompatible-type] (@poteto)
        if are_hook_inputs_equal(deps, prev_deps) {
            hook.memoized_state
                .set(Some(js::unknown_of(push_simple_effect(
                    hook_flags, inst, create, next_deps,
                ))));
            return;
        }
    }

    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    rendering_fiber
        .flags
        .set(rendering_fiber.flags.get() | fiber_flags);

    hook.memoized_state
        .set(Some(js::unknown_of(push_simple_effect(
            HOOK_HAS_EFFECT | hook_flags,
            inst,
            create,
            next_deps,
        ))));
}

fn mount_effect(create: Rc<Create>, deps: Option<Deps>) {
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    if __DEV__ && (rendering_fiber.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
        mount_effect_impl(
            MOUNT_PASSIVE_DEV_EFFECT | PASSIVE_EFFECT | PASSIVE_STATIC_EFFECT,
            HOOK_PASSIVE,
            create,
            deps,
        );
    } else {
        mount_effect_impl(
            PASSIVE_EFFECT | PASSIVE_STATIC_EFFECT,
            HOOK_PASSIVE,
            create,
            deps,
        );
    }
}

fn update_effect(create: Rc<Create>, deps: Option<Deps>) {
    update_effect_impl(PASSIVE_EFFECT, HOOK_PASSIVE, create, deps);
}

fn use_effect_event_impl(payload: EventFunctionPayload) {
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    rendering_fiber
        .flags
        .set(rendering_fiber.flags.get() | UPDATE_EFFECT);
    let component_update_queue: Option<Rc<FunctionComponentUpdateQueue>> =
        unsafe { js::cast(rendering_fiber.update_queue.get()) };
    match component_update_queue {
        None => {
            let component_update_queue = create_function_component_update_queue();
            rendering_fiber
                .update_queue
                .set(Some(js::unknown_of(Rc::clone(&component_update_queue))));
            component_update_queue.events.replace(Some(vec![payload]));
        }
        Some(component_update_queue) => {
            let mut events = component_update_queue.events.borrow_mut();
            match events.as_mut() {
                None => *events = Some(vec![payload]),
                Some(events) => events.push(payload),
            }
        }
    }
}

/// What `useEffectEvent` gives: a function calling what the component last
/// gave with its arguments.
pub type EventFunction = dyn Fn(&dyn Rest<dyn IntoUnknown>) -> Option<&'static Unknown>;

/// What `useEffectEvent` is given: any function.
pub type EventFunctionImpl = dyn Fn() -> Option<&'static Unknown>;

fn mount_event(callback: &'static EventFunctionImpl) -> Rc<EventFunction> {
    let hook = mount_work_in_progress_hook();
    let r#ref = Rc::new(EventFunctionRef {
        r#impl: Cell::new(callback),
    });
    hook.memoized_state
        .set(Some(js::unknown_of(Rc::clone(&r#ref))));
    // $FlowFixMe[incompatible-type]
    Rc::new(move |args: &dyn Rest<dyn IntoUnknown>| {
        if is_invalid_execution_context_for_event_function() {
            panic!("A function wrapped in useEffectEvent can't be called during rendering.");
        }
        function::apply(r#ref.r#impl.get(), None, Some(&args))
    })
}

fn update_event(callback: &'static EventFunctionImpl) -> Rc<EventFunction> {
    let hook = update_work_in_progress_hook();
    let r#ref: Rc<EventFunctionRef> = unsafe { js::cast(hook.memoized_state.get()) };
    use_effect_event_impl(EventFunctionPayload {
        r#ref: Rc::clone(&r#ref),
        next_impl: callback,
    });
    // $FlowFixMe[incompatible-type]
    Rc::new(move |args: &dyn Rest<dyn IntoUnknown>| {
        if is_invalid_execution_context_for_event_function() {
            panic!("A function wrapped in useEffectEvent can't be called during rendering.");
        }
        function::apply(r#ref.r#impl.get(), None, Some(&args))
    })
}

fn mount_insertion_effect(create: Rc<Create>, deps: Option<Deps>) {
    mount_effect_impl(UPDATE_EFFECT, HOOK_INSERTION, create, deps);
}

fn update_insertion_effect(create: Rc<Create>, deps: Option<Deps>) {
    update_effect_impl(UPDATE_EFFECT, HOOK_INSERTION, create, deps)
}

fn mount_layout_effect(create: Rc<Create>, deps: Option<Deps>) {
    let mut fiber_flags: Flags = UPDATE_EFFECT | LAYOUT_STATIC_EFFECT;
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    if __DEV__ && (rendering_fiber.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
        fiber_flags |= MOUNT_LAYOUT_DEV_EFFECT;
    }
    mount_effect_impl(fiber_flags, HOOK_LAYOUT, create, deps)
}

fn update_layout_effect(create: Rc<Create>, deps: Option<Deps>) {
    update_effect_impl(UPDATE_EFFECT, HOOK_LAYOUT, create, deps)
}

/// What a ref callback is: called with the instance, or `null`; what it
/// gives is the cleanup it's called with `null` in place of, if it's a
/// function.
type RefCallback = dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>;

fn imperative_handle_effect(
    create: Option<&'static Unknown>,
    r#ref: Option<&'static Unknown>,
) -> Destroy {
    if js::type_of(r#ref) == "function" {
        let ref_callback: &'static RefCallback = unsafe { js::cast(r#ref) };
        let create: &dyn Fn() -> Option<&'static Unknown> = unsafe { js::cast(create) };
        let inst = create();
        let ref_cleanup = ref_callback(inst);
        Some(js::unknown_of(move || {
            if let Kind::Function(ref_cleanup) = js::classify(ref_cleanup) {
                // $FlowFixMe[incompatible-use] we need to assume no parameters
                ref_cleanup();
            } else {
                ref_callback(js::null());
            }
        }))
    } else if let Some(ref_object) = r#ref {
        if __DEV__ && !ref_object.has_own_property("current") {
            console::error((
                concat!(
                    "Expected useImperativeHandle() first argument to either be a ",
                    "ref callback or React.createRef() object. Instead received: %s."
                ),
                &("an object with keys {".to_string()
                    + &js::object::keys(ref_object).join(", ")
                    + "}"),
            ));
        }
        let create: &dyn Fn() -> Option<&'static Unknown> = unsafe { js::cast(create) };
        let inst = create();
        js::set(ref_object, "current", inst);
        Some(js::unknown_of(move || {
            js::set(ref_object, "current", js::null::<&Unknown>());
        }))
    } else {
        None
    }
}

fn mount_imperative_handle(
    r#ref: Option<&'static Unknown>,
    create: Option<&'static Unknown>,
    deps: Option<Deps>,
) {
    if __DEV__ && js::type_of(create) != "function" {
        console::error((
            concat!(
                "Expected useImperativeHandle() second argument to be a function ",
                "that creates a handle. Instead received: %s."
            ),
            // $FlowFixMe[invalid-compare]
            &if !matches!(js::classify(create), Kind::Null) {
                js::type_of(create)
            } else {
                "null".to_string()
            },
        ));
    }

    // TODO: If deps are provided, should we skip comparing the ref itself?
    let effect_deps = deps.map(|deps| [deps.as_slice(), &[r#ref]].concat());

    let mut fiber_flags: Flags = UPDATE_EFFECT | LAYOUT_STATIC_EFFECT;
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    if __DEV__ && (rendering_fiber.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
        fiber_flags |= MOUNT_LAYOUT_DEV_EFFECT;
    }
    mount_effect_impl(
        fiber_flags,
        HOOK_LAYOUT,
        Rc::from(function::bind(
            imperative_handle_effect,
            Some(&Kind::Null),
            (create, r#ref),
        )),
        effect_deps,
    );
}

fn update_imperative_handle(
    r#ref: Option<&'static Unknown>,
    create: Option<&'static Unknown>,
    deps: Option<Deps>,
) {
    if __DEV__ && js::type_of(create) != "function" {
        console::error((
            concat!(
                "Expected useImperativeHandle() second argument to be a function ",
                "that creates a handle. Instead received: %s."
            ),
            // $FlowFixMe[invalid-compare]
            &if !matches!(js::classify(create), Kind::Null) {
                js::type_of(create)
            } else {
                "null".to_string()
            },
        ));
    }

    // TODO: If deps are provided, should we skip comparing the ref itself?
    let effect_deps = deps.map(|deps| [deps.as_slice(), &[r#ref]].concat());

    update_effect_impl(
        UPDATE_EFFECT,
        HOOK_LAYOUT,
        Rc::from(function::bind(
            imperative_handle_effect,
            Some(&Kind::Null),
            (create, r#ref),
        )),
        effect_deps,
    );
}

/// What `useDebugValue` formats its value with, for DevTools.
pub type DebugValueFormatter = dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>;

fn mount_debug_value(
    _value: Option<&'static Unknown>,
    _formatter_fn: Option<&'static DebugValueFormatter>,
) {
    // This hook is normally a no-op.
    // The react-debug-hooks package injects its own implementation
    // so that e.g. DevTools can display custom hook values.
}

#[cfg_attr(rust_js, rust_js::name = "updateDebugValue")]
const UPDATE_DEBUG_VALUE: fn(Option<&'static Unknown>, Option<&'static DebugValueFormatter>) =
    mount_debug_value;

/// What `useCallback` and `useMemo` keep: the value, and the deps it was
/// made of.
type Memoized = (Option<&'static Unknown>, Option<Deps>);

fn mount_callback(
    callback: Option<&'static Unknown>,
    deps: Option<Deps>,
) -> Option<&'static Unknown> {
    let hook = mount_work_in_progress_hook();
    let next_deps = match deps {
        None => js::null(),
        deps => deps,
    };
    let memoized: Memoized = (callback, next_deps);
    hook.memoized_state.set(Some(js::unknown_of(memoized)));
    callback
}

fn update_callback(
    callback: Option<&'static Unknown>,
    deps: Option<Deps>,
) -> Option<&'static Unknown> {
    let hook = update_work_in_progress_hook();
    let next_deps = match deps {
        None => js::null(),
        deps => deps,
    };
    let prev_state = hook.memoized_state.get();
    if let Some(next_deps) = &next_deps {
        let prev_deps = js::get(prev_state, &1usize);
        if are_hook_inputs_equal(next_deps, prev_deps) {
            return js::get(prev_state, &0usize);
        }
    }
    let memoized: Memoized = (callback, next_deps);
    hook.memoized_state.set(Some(js::unknown_of(memoized)));
    callback
}

/// What `useMemo` calls for its value.
pub type MemoCreate = dyn Fn() -> Option<&'static Unknown>;

fn mount_memo(next_create: &'static MemoCreate, deps: Option<Deps>) -> Option<&'static Unknown> {
    let hook = mount_work_in_progress_hook();
    let next_deps = match deps {
        None => js::null(),
        deps => deps,
    };
    let next_value = next_create();
    if SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV.get() {
        set_is_strict_mode_for_devtools(true);
        let _exit = ExitStrictModeForDevtools;
        next_create();
    }
    let memoized: Memoized = (next_value, next_deps);
    hook.memoized_state.set(Some(js::unknown_of(memoized)));
    next_value
}

fn update_memo(next_create: &'static MemoCreate, deps: Option<Deps>) -> Option<&'static Unknown> {
    let hook = update_work_in_progress_hook();
    let next_deps = match deps {
        None => js::null(),
        deps => deps,
    };
    let prev_state = hook.memoized_state.get();
    // Assume these are defined. If they're not, areHookInputsEqual will warn.
    if let Some(next_deps) = &next_deps {
        let prev_deps = js::get(prev_state, &1usize);
        if are_hook_inputs_equal(next_deps, prev_deps) {
            return js::get(prev_state, &0usize);
        }
    }
    let next_value = next_create();
    if SHOULD_DOUBLE_INVOKE_USER_FNS_IN_HOOKS_DEV.get() {
        set_is_strict_mode_for_devtools(true);
        let _exit = ExitStrictModeForDevtools;
        next_create();
    }
    let memoized: Memoized = (next_value, next_deps);
    hook.memoized_state.set(Some(js::unknown_of(memoized)));
    next_value
}

fn mount_deferred_value(
    value: Option<&'static Unknown>,
    initial_value: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let hook = mount_work_in_progress_hook();
    mount_deferred_value_impl(&hook, value, initial_value)
}

fn update_deferred_value(
    value: Option<&'static Unknown>,
    initial_value: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let hook = update_work_in_progress_hook();
    let resolved_current_hook = CURRENT_HOOK.with_borrow(Clone::clone).unwrap();
    let prev_value = resolved_current_hook.memoized_state.get();
    update_deferred_value_impl(&hook, prev_value, value, initial_value)
}

fn rerender_deferred_value(
    value: Option<&'static Unknown>,
    initial_value: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let hook = update_work_in_progress_hook();
    match CURRENT_HOOK.with_borrow(Clone::clone) {
        None => {
            // This is a rerender during a mount.
            mount_deferred_value_impl(&hook, value, initial_value)
        }
        Some(current_hook) => {
            // This is a rerender during an update.
            let prev_value = current_hook.memoized_state.get();
            update_deferred_value_impl(&hook, prev_value, value, initial_value)
        }
    }
}

fn is_rendering_deferred_work() -> bool {
    if !includes_some_lane(RENDER_LANES.get(), DEFERRED_LANE) {
        // None of the render lanes are deferred lanes.
        return false;
    }
    // At least one of the render lanes are deferred lanes. However, if the
    // current render is also batched together with an update, then we can't
    // say that the render is wholly the result of deferred work. We can check
    // this by checking if the root render lanes contain any "update" lanes, i.e.
    // lanes that are only assigned to updates, like setState.
    let root_render_lanes = get_work_in_progress_root_render_lanes();
    !includes_some_lane(root_render_lanes, UPDATE_LANES)
}

fn mount_deferred_value_impl(
    hook: &Hook,
    value: Option<&'static Unknown>,
    initial_value: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if
    // When `initialValue` is provided, we defer the initial render even if the
    // current render is not synchronous.
    !matches!(js::classify(initial_value), Kind::Undefined)
        // However, to avoid waterfalls, we do not defer if this render
        // was itself spawned by an earlier useDeferredValue. Check if DeferredLane
        // is part of the render lanes.
        && !is_rendering_deferred_work()
    {
        // Render with the initial value
        hook.memoized_state.set(initial_value);

        // Schedule a deferred render to switch to the final value.
        let deferred_lane = request_deferred_lane();
        let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
        rendering_fiber
            .lanes
            .set(merge_lanes(rendering_fiber.lanes.get(), deferred_lane));
        mark_skipped_update_lanes(deferred_lane);

        initial_value
    } else {
        hook.memoized_state.set(value);
        value
    }
}

fn update_deferred_value_impl(
    hook: &Hook,
    prev_value: Option<&'static Unknown>,
    value: Option<&'static Unknown>,
    initial_value: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if OBJECT_IS.with(|is| is(&value, &prev_value)) {
        // The incoming value is referentially identical to the currently rendered
        // value, so we can bail out quickly.
        value
    } else {
        // Received a new value that's different from the current value.

        // Check if we're inside a hidden tree
        if is_current_tree_hidden() {
            // Revealing a prerendered tree is considered the same as mounting new
            // one, so we reuse the "mount" path in this case.
            let result_value = mount_deferred_value_impl(hook, value, initial_value);
            // Unlike during an actual mount, we need to mark this as an update if
            // the value changed.
            if !OBJECT_IS.with(|is| is(&result_value, &prev_value)) {
                mark_work_in_progress_received_update();
            }
            return result_value;
        }

        let should_defer_value =
            !includes_only_non_urgent_lanes(RENDER_LANES.get()) && !is_rendering_deferred_work();
        if should_defer_value {
            // This is an urgent update. Since the value has changed, keep using the
            // previous value and spawn a deferred render to update it later.

            // Schedule a deferred render
            let deferred_lane = request_deferred_lane();
            let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
            rendering_fiber
                .lanes
                .set(merge_lanes(rendering_fiber.lanes.get(), deferred_lane));
            mark_skipped_update_lanes(deferred_lane);

            // Reuse the previous value. We do not need to mark this as an update,
            // because we did not render a new value.
            prev_value
        } else {
            // This is not an urgent update, so we can use the latest value regardless
            // of what it is. No need to defer it.

            // Mark this as an update to prevent the fiber from bailing out.
            mark_work_in_progress_received_update();
            hook.memoized_state.set(value);
            value
        }
    }
}

fn release_async_transition() {
    if __DEV__ {
        REACT_SHARED_INTERNALS
            .set_async_transitions(REACT_SHARED_INTERNALS.async_transitions() - 1.0);
    }
}

/// What `startTransition`'s `finally` does: the update priority back, then
/// the outer Transition, after the inner one's types are handed to it.
struct FinishTransition<'a> {
    previous_priority: EventPriority,
    prev_transition: Option<&'static Transition>,
    current_transition: &'a Transition,
}

impl Drop for FinishTransition<'_> {
    fn drop(&mut self) {
        set_current_update_priority(self.previous_priority);

        let current_types = self.current_transition.types.borrow().clone();
        if let Some(prev_transition) = self.prev_transition
            && let Some(current_types) = current_types
        {
            // If we created a new types set in the inner transition, we transfer it to the parent
            // since they should share the same set. They're conceptually entangled.
            let prev_types = prev_transition.types.borrow().clone();
            if __DEV__
                && let Some(prev_types) = prev_types
                && !Rc::ptr_eq(&prev_types, &current_types)
            {
                // Just assert that assumption holds that we're not overriding anything.
                console::error((concat!(
                    "We expected inner Transitions to have transferred the outer types set and ",
                    "that you cannot add to the outer Transition while inside the inner.",
                    "This is a bug in React."
                ),));
            }
            prev_transition.types.replace(Some(current_types));
        }
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);

        if __DEV__ && self.prev_transition.is_none() {
            let updated_fibers_count = self
                .current_transition
                .updated_fibers
                .borrow_mut()
                .as_mut()
                .map(|updated_fibers| {
                    let updated_fibers_count = updated_fibers.len();
                    updated_fibers.clear();
                    updated_fibers_count
                });
            if let Some(updated_fibers_count) = updated_fibers_count
                && updated_fibers_count > 10
            {
                console::warn((concat!(
                    "Detected a large number of updates inside startTransition. ",
                    "If this is due to a subscription please re-write it to use React provided hooks. ",
                    "Otherwise concurrent mode guarantees are off the table."
                ),));
            }
        }
    }
}

/// shared/ReactTypes' `RejectedThenable`.
struct RejectedThenable {
    then: fn(),
    status: &'static str,
    reason: Option<&'static Unknown>,
}

unsafe impl js::Defined for RejectedThenable {}

fn start_transition(
    fiber: &Rc<Fiber>,
    queue: &Rc<UpdateQueue>,
    pending_state: Option<&'static Unknown>,
    finished_state: Option<&'static Unknown>,
    callback: &dyn Fn() -> Option<&'static Unknown>,
    options: Option<&StartTransitionOptions>,
) {
    let previous_priority = get_current_update_priority();
    set_current_update_priority(higher_event_priority(
        previous_priority,
        CONTINUOUS_EVENT_PRIORITY,
    ));

    let prev_transition = t(REACT_SHARED_INTERNALS);
    let mut current_transition = Transition {
        types: RefCell::new(None),
        gesture: None,
        name: None,
        start_time: Cell::new(None),
        updated_fibers: RefCell::new(None),
    };
    if ENABLE_VIEW_TRANSITION {
        *current_transition.types.get_mut() = match prev_transition {
            // If we're a nested transition, we should use the same set as the parent
            // since we're conceptually always joined into the same entangled transition.
            // In practice, this only matters if we add transition types in the inner
            // without setting state. In that case, the inner transition can finish
            // without waiting for the outer.
            Some(prev_transition) => prev_transition.types.borrow().clone(),
            None => None,
        };
    }
    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        current_transition.gesture = None;
    }
    if ENABLE_TRANSITION_TRACING {
        current_transition.name = options.and_then(|options| options.name.clone());
        current_transition.start_time.set(Some(now()));
    }
    if __DEV__ {
        *current_transition.updated_fibers.get_mut() = Some(HashSet::new());
    }

    // We don't really need to use an optimistic update here, because we
    // schedule a second "revert" update below (which we use to suspend the
    // transition until the async action scope has finished). But we'll use an
    // optimistic update anyway to make it less likely the behavior accidentally
    // diverges; for example, both an optimistic update and this one should
    // share the same lane.
    set_t(REACT_SHARED_INTERNALS, Some(&current_transition));
    dispatch_optimistic_set_state(fiber, false, queue, pending_state, None);

    let _finish = FinishTransition {
        previous_priority,
        prev_transition,
        current_transition: &current_transition,
    };
    if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
        let return_value = callback();
        if let Some(on_start_transition_finish) = s(REACT_SHARED_INTERNALS) {
            on_start_transition_finish(&current_transition, return_value);
        }

        // Check if we're inside an async action scope. If so, we'll entangle
        // this new action with the existing scope.
        //
        // If we're not already inside an async action scope, and this action is
        // async, then we'll create a new async scope.
        //
        // In the async case, the resulting render will suspend until the async
        // action scope has finished.
        if let Some(value) = return_value
            && js::type_of(return_value) == "object"
            && js::type_of(js::get(value, "then")) == "function"
        {
            let thenable: &'static Thenable = unsafe { js::cast(return_value) };
            if __DEV__ {
                // Keep track of the number of async transitions still running so we can warn.
                REACT_SHARED_INTERNALS
                    .set_async_transitions(REACT_SHARED_INTERNALS.async_transitions() + 1.0);
                thenable.then(
                    |_| release_async_transition(),
                    |_| release_async_transition(),
                );
            }
            // Create a thenable that resolves to `finishedState` once the async
            // action has completed.
            let thenable_for_finished_state = chain_thenable_value(thenable, finished_state);
            dispatch_set_state_internal(
                fiber,
                queue,
                Some(js::unknown(thenable_for_finished_state)),
                request_update_lane(fiber),
            );
        } else {
            dispatch_set_state_internal(fiber, queue, finished_state, request_update_lane(fiber));
        }
    })) {
        // This is a trick to get the `useTransition` hook to rethrow the error.
        // When it unwraps the thenable with the `use` algorithm, the error
        // will be thrown.
        let rejected_thenable = RejectedThenable {
            then: || {},
            status: "rejected",
            reason: error
                .downcast_ref::<&'static JsError>()
                .and_then(|error| error.as_unknown()),
        };
        dispatch_set_state_internal(
            fiber,
            queue,
            Some(js::unknown_of(rejected_thenable)),
            request_update_lane(fiber),
        );
    }
}

#[cfg_attr(rust_js, rust_js::name = "noop")]
const NOOP: fn() -> Option<&'static Unknown> = || None;

/// What a form's action is: called with the form's data.
pub type FormAction = dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>;

// ReactFiberConfig's `TransitionStatus`, and the form's data.
pub fn start_host_transition(
    form_fiber: &Rc<Fiber>,
    pending_state: Option<&'static Unknown>,
    action: Option<&'static FormAction>,
    form_data: Option<&'static Unknown>,
) {
    if form_fiber.tag.get() != HOST_COMPONENT {
        panic!(concat!(
            "Expected the form instance to be a HostComponent. This ",
            "is a bug in React."
        ));
    }

    let state_hook = ensure_form_component_is_stateful(form_fiber);

    let queue: Rc<UpdateQueue> = unsafe { js::cast(state_hook.queue.get()) };

    start_host_action_timer(form_fiber);

    // TODO: `startTransition` both sets the pending state and dispatches
    // the action, if one is provided. Consider refactoring these two
    // concerns to avoid the extra lambda.
    let callback: &dyn Fn() -> Option<&'static Unknown> = match action {
        // No action was provided, but we still call `startTransition` to
        // set the pending form status.
        None => &NOOP,
        Some(action) => &move || {
            // Automatically reset the form when the action completes.
            request_form_reset(form_fiber);
            action(form_data)
        },
    };
    start_transition(
        form_fiber,
        &queue,
        pending_state,
        NO_PENDING_HOST_TRANSITION,
        callback,
        None,
    );
}

fn ensure_form_component_is_stateful(form_fiber: &Fiber) -> Rc<Hook> {
    let existing_state_hook: Option<Rc<Hook>> =
        unsafe { js::cast(form_fiber.memoized_state.get()) };
    if let Some(existing_state_hook) = existing_state_hook {
        // This fiber was already upgraded to be stateful.
        return existing_state_hook;
    }

    // Upgrade this host component fiber to be stateful. We're going to pretend
    // it was stateful all along so we can reuse most of the implementation
    // for function components and useTransition.
    //
    // Create the state hook used by TransitionAwareHostComponent. This is
    // essentially an inlined version of mountState.
    let new_queue = Rc::new(UpdateQueue {
        pending: RefCell::new(None),
        lanes: Cell::new(NO_LANES),
        // We're going to cheat and intentionally not create a bound dispatch
        // method, because we can call it directly in startTransition.
        dispatch: RefCell::new(js::null()),
        last_rendered_reducer: Cell::new(Some(&basic_state_reducer)),
        last_rendered_state: Cell::new(NO_PENDING_HOST_TRANSITION),
    });

    let state_hook = Rc::new(Hook {
        memoized_state: Cell::new(NO_PENDING_HOST_TRANSITION),
        base_state: Cell::new(NO_PENDING_HOST_TRANSITION),
        base_queue: RefCell::new(None),
        queue: Cell::new(Some(js::unknown_of(new_queue))),
        next: RefCell::new(None),
    });

    // We use another state hook to track whether the form needs to be reset.
    // The state is an empty object. To trigger a reset, we update the state
    // to a new object. Then during rendering, we detect that the state has
    // changed and schedule a commit effect.
    let initial_reset_state = Some(js::unknown(js::dict::new::<()>()));
    let new_reset_state_queue = Rc::new(UpdateQueue {
        pending: RefCell::new(None),
        lanes: Cell::new(NO_LANES),
        // We're going to cheat and intentionally not create a bound dispatch
        // method, because we can call it directly in startTransition.
        dispatch: RefCell::new(js::null()),
        last_rendered_reducer: Cell::new(Some(&basic_state_reducer)),
        last_rendered_state: Cell::new(initial_reset_state),
    });
    let reset_state_hook = Rc::new(Hook {
        memoized_state: Cell::new(initial_reset_state),
        base_state: Cell::new(initial_reset_state),
        base_queue: RefCell::new(None),
        queue: Cell::new(Some(js::unknown_of(new_reset_state_queue))),
        next: RefCell::new(None),
    });
    state_hook.next.replace(Some(reset_state_hook));

    // Add the hook list to both fiber alternates. The idea is that the fiber
    // had this hook all along.
    form_fiber
        .memoized_state
        .set(Some(js::unknown_of(Rc::clone(&state_hook))));
    let alternate = form_fiber.alternate.borrow().clone();
    if let Some(alternate) = alternate {
        alternate
            .memoized_state
            .set(Some(js::unknown_of(Rc::clone(&state_hook))));
    }

    state_hook
}

pub fn request_form_reset(form_fiber: &Rc<Fiber>) {
    let transition = request_current_transition();

    match transition {
        None => {
            if __DEV__ {
                // An optimistic update occurred, but startTransition is not on the stack.
                // The form reset will be scheduled at default (sync) priority, which
                // is probably not what the user intended. Most likely because the
                // requestFormReset call happened after an `await`.
                // TODO: Theoretically, requestFormReset is still useful even for
                // non-transition updates because it allows you to update defaultValue
                // synchronously and then wait to reset until after the update commits.
                // I've chosen to warn anyway because it's more likely the `await` mistake
                // described above. But arguably we shouldn't.
                console::error((concat!(
                    "requestFormReset was called outside a transition or action. To ",
                    "fix, move to an action, or wrap with startTransition."
                ),));
            }
        }
        Some(transition)
            if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled)
                && transition.gesture.is_some() =>
        {
            panic!(concat!(
                "Cannot requestFormReset() inside a startGestureTransition. ",
                "There should be no side-effects associated with starting a ",
                "Gesture until its Action is invoked. Move side-effects to the ",
                "Action instead."
            ));
        }
        Some(_) => {}
    }

    let mut state_hook = ensure_form_component_is_stateful(form_fiber);
    let new_reset_state = Some(js::unknown(js::dict::new::<()>()));
    if state_hook.next.borrow().is_none() {
        // Hack alert. If formFiber is the workInProgress Fiber then
        // we might get a broken intermediate state. Try the alternate
        // instead.
        // TODO: We should really stash the Queue somewhere stateful
        // just like how setState binds the Queue.
        let alternate = form_fiber.alternate.borrow().clone().unwrap();
        state_hook = unsafe { js::cast(alternate.memoized_state.get()) };
    }
    let reset_state_hook = state_hook.next.borrow().clone().unwrap();
    let reset_state_queue: Rc<UpdateQueue> = unsafe { js::cast(reset_state_hook.queue.get()) };
    dispatch_set_state_internal(
        form_fiber,
        &reset_state_queue,
        new_reset_state,
        request_update_lane(form_fiber),
    );
}

/// What `useTransition` gives to start a Transition: of the callback to
/// call in it.
pub type StartTransition =
    dyn Fn(&dyn Fn() -> Option<&'static Unknown>, Option<&StartTransitionOptions>);

fn mount_transition() -> (bool, Rc<StartTransition>) {
    let state_hook = mount_state_impl(Some(js::unknown(&false)));
    // The `start` method never changes.
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap();
    let queue: Rc<UpdateQueue> = unsafe { js::cast(state_hook.queue.get()) };
    let start: Rc<StartTransition> = Rc::new(move |callback, options| {
        start_transition(
            &rendering_fiber,
            &queue,
            Some(js::unknown(&true)),
            Some(js::unknown(&false)),
            callback,
            options,
        )
    });
    let hook = mount_work_in_progress_hook();
    hook.memoized_state
        .set(Some(js::unknown_of(Rc::clone(&start))));
    (false, start)
}

fn update_transition() -> (bool, Rc<StartTransition>) {
    let (boolean_or_thenable, _) = update_state(Some(js::unknown(&false)));
    let hook = update_work_in_progress_hook();
    let start: Rc<StartTransition> = unsafe { js::cast(hook.memoized_state.get()) };
    let is_pending = match js::classify(boolean_or_thenable) {
        Kind::Bool(boolean) => boolean,
        // This will suspend until the async action scope has finished.
        _ => unsafe { js::cast(use_thenable(js::cast(boolean_or_thenable))) },
    };
    (is_pending, start)
}

fn rerender_transition() -> (bool, Rc<StartTransition>) {
    let (boolean_or_thenable, _) = rerender_state(Some(js::unknown(&false)));
    let hook = update_work_in_progress_hook();
    let start: Rc<StartTransition> = unsafe { js::cast(hook.memoized_state.get()) };
    let is_pending = match js::classify(boolean_or_thenable) {
        Kind::Bool(boolean) => boolean,
        // This will suspend until the async action scope has finished.
        _ => unsafe { js::cast(use_thenable(js::cast(boolean_or_thenable))) },
    };
    (is_pending, start)
}

// ReactFiberConfig's `TransitionStatus`.
fn use_host_transition_status() -> Option<&'static Unknown> {
    read_context(&HOST_TRANSITION_CONTEXT)
}

fn mount_id() -> String {
    let hook = mount_work_in_progress_hook();

    let root = get_work_in_progress_root().unwrap();
    // TODO: In Fizz, id generation is specific to each server config. Maybe we
    // should do this in Fiber, too? Deferring this decision for now because
    // there's no other place to store the prefix except for an internal field on
    // the public createRoot object, which the fiber tree does not currently have
    // a reference to.
    let identifier_prefix = &root.identifier_prefix;

    let mut id;
    if get_is_hydrating() {
        let tree_id = get_tree_id();

        // Use a captial R prefix for server-generated ids.
        id = "_".to_string() + identifier_prefix + "R_" + &tree_id;

        // Unless this is the first id at this level, append a number at the end
        // that represents the position of this useId hook among all the useId
        // hooks for this fiber.
        let local_id = LOCAL_ID_COUNTER.get();
        LOCAL_ID_COUNTER.set(local_id + 1);
        if local_id > 0 {
            id += &("H".to_string() + &js::number::to_string(local_id.into(), Some(32)));
        }

        id += "_";
    } else {
        // Use a lowercase r prefix for client-generated ids.
        let global_client_id = GLOBAL_CLIENT_ID_COUNTER.get();
        GLOBAL_CLIENT_ID_COUNTER.set(global_client_id + 1);
        id = "_".to_string()
            + identifier_prefix
            + "r_"
            + &js::number::to_string(global_client_id.into(), Some(32))
            + "_";
    }

    hook.memoized_state.set(Some(js::unknown_of(id.clone())));
    id
}

fn update_id() -> String {
    let hook = update_work_in_progress_hook();
    let id: String = unsafe { js::cast(hook.memoized_state.get()) };
    id
}

/// What `useCacheRefresh` gives: what refreshes the cache, of what makes
/// its next value, and the value it seeds it with.
pub type CacheRefresh = dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>);

fn mount_refresh() -> Rc<CacheRefresh> {
    let hook = mount_work_in_progress_hook();
    let refresh: Rc<CacheRefresh> = Rc::from(function::bind(
        refresh_cache,
        Some(&Kind::Null),
        (CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).unwrap(),),
    ));
    hook.memoized_state
        .set(Some(js::unknown_of(Rc::clone(&refresh))));
    refresh
}

fn update_refresh() -> Rc<CacheRefresh> {
    let hook = update_work_in_progress_hook();
    unsafe { js::cast(hook.memoized_state.get()) }
}

/// A refresh's update of a cache boundary: the cache it's refreshed with.
struct CacheRefreshPayload {
    cache: Rc<Cache>,
}

unsafe impl js::Defined for CacheRefreshPayload {}

fn refresh_cache(
    fiber: &Rc<Fiber>,
    seed_key: Option<&'static Unknown>,
    seed_value: Option<&'static Unknown>,
) {
    // TODO: Does Cache work in legacy mode? Should decide and write a test.
    // TODO: Consider warning if the refresh is at discrete priority, or if we
    // otherwise suspect that it wasn't batched properly.
    let mut provider = fiber.r#return.borrow().clone();
    while let Some(current) = provider {
        match current.tag.get() {
            CACHE_COMPONENT | HOST_ROOT => {
                // Schedule an update on the cache boundary to trigger a refresh.
                let lane = request_update_lane(&current);
                let refresh_update = create_legacy_queue_update(lane);
                let root = enqueue_legacy_queue_update(&current, &refresh_update, lane);
                if let Some(root) = &root {
                    start_update_timer_by_lane(lane, "refresh()", Some(fiber));
                    schedule_update_on_fiber(root, &current, lane);
                    entangle_legacy_queue_transitions(root, &current, lane);
                }

                // TODO: If a refresh never commits, the new cache created here must be
                // released. A simple case is start refreshing a cache boundary, but then
                // unmount that boundary before the refresh completes.
                let seeded_cache = create_cache();
                if seed_key.is_some() && root.is_some() {
                    if ENABLE_LEGACY_CACHE.with(|&enabled| enabled) {
                        // Seed the cache with the value passed by the caller. This could be
                        // from a server mutation, or it could be a streaming response.
                        let seed_key: &'static dyn Fn() -> Option<&'static Unknown> =
                            unsafe { js::cast(seed_key) };
                        seeded_cache.data.set(seed_key, seed_value);
                    } else if __DEV__ {
                        console::error((
                            "The seed argument is not enabled outside experimental channels.",
                        ));
                    }
                }

                let payload = CacheRefreshPayload {
                    cache: seeded_cache,
                };
                refresh_update.payload.set(Some(js::unknown_of(payload)));
                return;
            }
            _ => {}
        }
        provider = current.r#return.borrow().clone();
    }
    // TODO: Warn if unmounted?
}

fn dispatch_reducer_action(
    fiber: &Rc<Fiber>,
    queue: &Rc<UpdateQueue>,
    action: Option<&'static Unknown>,
    callback: Option<&'static Unknown>,
) {
    if __DEV__ && js::type_of(callback) == "function" {
        console::error((concat!(
            "State updates from the useState() and useReducer() Hooks don't support the ",
            "second callback argument. To execute a side effect after ",
            "rendering, declare it in the component body with useEffect()."
        ),));
    }

    let lane = request_update_lane(fiber);

    let update = Rc::new(Update {
        lane: Cell::new(lane),
        revert_lane: Cell::new(NO_LANE),
        gesture: RefCell::new(js::null()),
        action,
        has_eager_state: Cell::new(false),
        eager_state: Cell::new(js::null()),
        next: RefCell::new(js::null()),
    });

    if is_render_phase_update(fiber) {
        enqueue_render_phase_update(queue, &update);
    } else {
        let root = enqueue_concurrent_hook_update(fiber, queue, &update, lane);
        if let Some(root) = root {
            start_update_timer_by_lane(lane, "dispatch()", Some(fiber));
            schedule_update_on_fiber(&root, fiber, lane);
            entangle_transition_update(&root, queue, lane);
        }
    }

    mark_update_in_dev_tools(fiber, lane, action);
}

fn dispatch_set_state(
    fiber: &Rc<Fiber>,
    queue: &Rc<UpdateQueue>,
    action: Option<&'static Unknown>,
    callback: Option<&'static Unknown>,
) {
    if __DEV__ && js::type_of(callback) == "function" {
        console::error((concat!(
            "State updates from the useState() and useReducer() Hooks don't support the ",
            "second callback argument. To execute a side effect after ",
            "rendering, declare it in the component body with useEffect()."
        ),));
    }

    let lane = request_update_lane(fiber);
    let did_schedule_update = dispatch_set_state_internal(fiber, queue, action, lane);
    if did_schedule_update {
        start_update_timer_by_lane(lane, "setState()", Some(fiber));
    }
    mark_update_in_dev_tools(fiber, lane, action);
}

/// What `dispatchSetStateInternal`'s `finally` does: in DEV, the dispatcher
/// it replaced back.
struct RestoreDispatcherInDev(Option<&'static SharedDispatcher>);

impl Drop for RestoreDispatcherInDev {
    fn drop(&mut self) {
        if __DEV__ {
            REACT_SHARED_INTERNALS.set_h(self.0);
        }
    }
}

fn dispatch_set_state_internal(
    fiber: &Rc<Fiber>,
    queue: &Rc<UpdateQueue>,
    action: Option<&'static Unknown>,
    lane: Lane,
) -> bool {
    let update = Rc::new(Update {
        lane: Cell::new(lane),
        revert_lane: Cell::new(NO_LANE),
        gesture: RefCell::new(js::null()),
        action,
        has_eager_state: Cell::new(false),
        eager_state: Cell::new(js::null()),
        next: RefCell::new(js::null()),
    });

    if is_render_phase_update(fiber) {
        enqueue_render_phase_update(queue, &update);
    } else {
        let alternate = fiber.alternate.borrow().clone();
        if fiber.lanes.get() == NO_LANES
            && alternate.is_none_or(|alternate| alternate.lanes.get() == NO_LANES)
        {
            // The queue is currently empty, which means we can eagerly compute the
            // next state before entering the render phase. If the new state is the
            // same as the current state, we may be able to bail out entirely.
            let last_rendered_reducer = queue.last_rendered_reducer.get();
            if let Some(last_rendered_reducer) = last_rendered_reducer {
                let mut prev_dispatcher = None;
                if __DEV__ {
                    prev_dispatcher = REACT_SHARED_INTERNALS.h();
                    INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV
                        .with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                }
                let _restore = RestoreDispatcherInDev(prev_dispatcher);
                let did_bail_out = panic::catch_unwind(AssertUnwindSafe(|| {
                    let current_state = queue.last_rendered_state.get();
                    let eager_state = last_rendered_reducer(current_state, action);
                    // Stash the eagerly computed state, and the reducer used to compute
                    // it, on the update object. If the reducer hasn't changed by the
                    // time we enter the render phase, then the eager state can be used
                    // without calling the reducer again.
                    update.has_eager_state.set(true);
                    update.eager_state.set(eager_state);
                    if OBJECT_IS.with(|is| is(&eager_state, &current_state)) {
                        // Fast path. We can bail out without scheduling React to re-render.
                        // It's still possible that we'll need to rebase this update later,
                        // if the component re-renders for a different reason and by that
                        // time the reducer has changed.
                        // TODO: Do we still need to entangle transitions in this case?
                        enqueue_concurrent_hook_update_and_eagerly_bailout(fiber, queue, &update);
                        return true;
                    }
                    false
                }));
                // An error is suppressed. It will throw again in the render phase.
                if let Ok(true) = did_bail_out {
                    return false;
                }
            }
        }

        let root = enqueue_concurrent_hook_update(fiber, queue, &update, lane);
        if let Some(root) = root {
            schedule_update_on_fiber(&root, fiber, lane);
            entangle_transition_update(&root, queue, lane);
            return true;
        }
    }
    false
}

fn dispatch_optimistic_set_state(
    fiber: &Rc<Fiber>,
    throw_if_during_render: bool,
    queue: &Rc<UpdateQueue>,
    action: Option<&'static Unknown>,
    _callback: Option<&'static Unknown>,
) {
    let transition = request_current_transition();

    if __DEV__ && transition.is_none() {
        // An optimistic update occurred, but startTransition is not on the stack.
        // There are two likely scenarios.

        // One possibility is that the optimistic update is triggered by a regular
        // event handler (e.g. `onSubmit`) instead of an action. This is a mistake
        // and we will warn.

        // The other possibility is the optimistic update is inside an async
        // action, but after an `await`. In this case, we can make it "just work"
        // by associating the optimistic update with the pending async action.

        // Technically it's possible that the optimistic update is unrelated to
        // the pending action, but we don't have a way of knowing this for sure
        // because browsers currently do not provide a way to track async scope.
        // (The AsyncContext proposal, if it lands, will solve this in the
        // future.) However, this is no different than the problem of unrelated
        // transitions being grouped together — it's not wrong per se, but it's
        // not ideal.

        // Once AsyncContext starts landing in browsers, we will provide better
        // warnings in development for these cases.
        if peek_entangled_action_lane() != NO_LANE {
            // There is a pending async action. Don't warn.
        } else {
            // There's no pending async action. The most likely cause is that we're
            // inside a regular event handler (e.g. onSubmit) instead of an action.
            console::error((concat!(
                "An optimistic state update occurred outside a transition or ",
                "action. To fix, move the update to an action, or wrap ",
                "with startTransition."
            ),));
        }
    }

    // For regular Transitions an optimistic update commits synchronously.
    // For gesture Transitions an optimistic update commits on the GestureLane.
    let lane = if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled)
        && transition.is_some_and(|transition| transition.gesture.is_some())
    {
        GESTURE_LANE
    } else {
        SYNC_LANE
    };
    let update = Rc::new(Update {
        lane: Cell::new(lane),
        // After committing, the optimistic update is "reverted" using the same
        // lane as the transition it's associated with.
        revert_lane: Cell::new(request_transition_lane(transition)),
        gesture: RefCell::new(js::null()),
        action,
        has_eager_state: Cell::new(false),
        eager_state: Cell::new(js::null()),
        next: RefCell::new(js::null()),
    });

    if is_render_phase_update(fiber) {
        // When calling startTransition during render, this warns instead of
        // throwing because throwing would be a breaking change. setOptimisticState
        // is a new API so it's OK to throw.
        if throw_if_during_render {
            panic!("Cannot update optimistic state while rendering.");
        } else {
            // startTransition was called during render. We don't need to do anything
            // besides warn here because the render phase update would be overidden by
            // the second update, anyway. We can remove this branch and make it throw
            // in a future release.
            if __DEV__ {
                console::error(("Cannot call startTransition while rendering.",));
            }
        }
    } else {
        let root = enqueue_concurrent_hook_update(fiber, queue, &update, lane);
        if let Some(root) = root {
            // NOTE: The optimistic update implementation assumes that the transition
            // will never be attempted before the optimistic update. This currently
            // holds because the optimistic update is always synchronous. If we ever
            // change that, we'll need to account for this.
            start_update_timer_by_lane(lane, "setOptimistic()", Some(fiber));
            schedule_update_on_fiber(&root, fiber, lane);
            // Optimistic updates are always synchronous, so we don't need to call
            // entangleTransitionUpdate here.
            if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled)
                && let Some(transition) = transition
                && let Some(provider) = transition.gesture
            {
                // If this was a gesture, ensure we have a scheduled gesture and that
                // we associate this update with this specific gesture instance.
                let gesture = schedule_gesture(&root, provider);
                update.gesture.replace(Some(Rc::clone(&gesture)));
                // Ensure the gesture always uses the same revert lane. This can happen for
                // two startGestureTransition calls to the same provider in different events.
                if gesture.revert_lane.get() == NO_LANE {
                    gesture.revert_lane.set(update.revert_lane.get());
                } else {
                    update.revert_lane.set(gesture.revert_lane.get());
                }
            }
        }
    }

    mark_update_in_dev_tools(fiber, lane, action);
}

fn is_render_phase_update(fiber: &Rc<Fiber>) -> bool {
    let alternate = fiber.alternate.borrow().clone();
    let rendering_fiber = CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone);
    rendering_fiber.as_ref() == Some(fiber) || (alternate.is_some() && alternate == rendering_fiber)
}

fn enqueue_render_phase_update(queue: &UpdateQueue, update: &Rc<Update>) {
    // This is a render phase update. Stash it in a lazily-created map of
    // queue -> linked list of updates. After this render pass, we'll restart
    // and apply the stashed updates on top of the work-in-progress hook.
    DID_SCHEDULE_RENDER_PHASE_UPDATE.set(true);
    DID_SCHEDULE_RENDER_PHASE_UPDATE_DURING_THIS_PASS.set(true);
    let pending = queue.pending.borrow().clone();
    match pending {
        None => {
            // This is the first update. Create a circular list.
            update.next.replace(Some(Rc::clone(update)));
        }
        Some(pending) => {
            let next = pending.next.borrow().clone();
            update.next.replace(next);
            pending.next.replace(Some(Rc::clone(update)));
        }
    }
    queue.pending.replace(Some(Rc::clone(update)));
}

// TODO: Move to ReactFiberConcurrentUpdates?
fn entangle_transition_update(root: &FiberRoot, queue: &UpdateQueue, lane: Lane) {
    if is_transition_lane(lane) {
        let mut queue_lanes = queue.lanes.get();

        // If any entangled lanes are no longer pending on the root, then they
        // must have finished. We can remove them from the shared queue, which
        // represents a superset of the actually pending lanes. In some cases we
        // may entangle more than we need to, but that's OK. In fact it's worse if
        // we *don't* entangle when we should.
        queue_lanes = intersect_lanes(queue_lanes, root.pending_lanes.get());

        // Entangle the new transition lane with the other transition lanes.
        let new_queue_lanes = merge_lanes(queue_lanes, lane);
        queue.lanes.set(new_queue_lanes);
        // Even if queue.lanes already include lane, we don't know for certain if
        // the lane finished since the last time we entangled it. So we need to
        // entangle it again, just to be sure.
        mark_root_entangled(root, new_queue_lanes);
    }
}

fn mark_update_in_dev_tools(fiber: &Fiber, lane: Lane, _action: Option<&'static Unknown>) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_state_update_scheduled(fiber, lane);
    }
}

/// What a DEV dispatcher's hook does in its `finally`: the dispatcher it
/// replaced back.
struct RestoreDispatcher(Option<&'static SharedDispatcher>);

impl Drop for RestoreDispatcher {
    fn drop(&mut self) {
        REACT_SHARED_INTERNALS.set_h(self.0);
    }
}

#[cfg_attr(rust_js, rust_js::name = "ContextOnlyDispatcher")]
pub static CONTEXT_ONLY_DISPATCHER: Dispatcher = Dispatcher {
    read_context: read_context::<Option<&'static Unknown>>,

    r#use,
    use_callback: |_, _| throw_invalid_hook_error(),
    use_context: |_| throw_invalid_hook_error(),
    use_effect: |_, _| throw_invalid_hook_error(),
    use_imperative_handle: |_, _, _| throw_invalid_hook_error(),
    use_layout_effect: |_, _| throw_invalid_hook_error(),
    use_insertion_effect: |_, _| throw_invalid_hook_error(),
    use_memo: |_, _| throw_invalid_hook_error(),
    use_reducer: |_, _, _| throw_invalid_hook_error(),
    use_ref: |_| throw_invalid_hook_error(),
    use_state: |_| throw_invalid_hook_error(),
    use_debug_value: |_, _| throw_invalid_hook_error(),
    use_deferred_value: |_, _| throw_invalid_hook_error(),
    use_transition: || throw_invalid_hook_error(),
    use_sync_external_store: |_, _, _| throw_invalid_hook_error(),
    use_id: || throw_invalid_hook_error(),
    use_host_transition_status: || throw_invalid_hook_error(),
    use_form_state: |_, _, _| throw_invalid_hook_error(),
    use_action_state: |_, _, _| throw_invalid_hook_error(),
    use_optimistic: |_, _| throw_invalid_hook_error(),
    use_memo_cache: |_| throw_invalid_hook_error(),
    use_cache_refresh: || throw_invalid_hook_error(),
    use_effect_event: |_| throw_invalid_hook_error(),
};

#[cfg_attr(rust_js, rust_js::name = "HooksDispatcherOnMount")]
static HOOKS_DISPATCHER_ON_MOUNT: Dispatcher = Dispatcher {
    read_context: read_context::<Option<&'static Unknown>>,

    r#use,
    use_callback: mount_callback,
    use_context: read_context::<Option<&'static Unknown>>,
    use_effect: mount_effect,
    use_imperative_handle: mount_imperative_handle,
    use_layout_effect: mount_layout_effect,
    use_insertion_effect: mount_insertion_effect,
    use_memo: mount_memo,
    use_reducer: mount_reducer,
    use_ref: mount_ref,
    use_state: mount_state,
    use_debug_value: mount_debug_value,
    use_deferred_value: mount_deferred_value,
    use_transition: mount_transition,
    use_sync_external_store: mount_sync_external_store,
    use_id: mount_id,
    use_host_transition_status,
    use_form_state: mount_action_state,
    use_action_state: mount_action_state,
    use_optimistic: mount_optimistic,
    use_memo_cache,
    use_cache_refresh: mount_refresh,
    use_effect_event: mount_event,
};

#[cfg_attr(rust_js, rust_js::name = "HooksDispatcherOnUpdate")]
static HOOKS_DISPATCHER_ON_UPDATE: Dispatcher = Dispatcher {
    read_context: read_context::<Option<&'static Unknown>>,

    r#use,
    use_callback: update_callback,
    use_context: read_context::<Option<&'static Unknown>>,
    use_effect: update_effect,
    use_imperative_handle: update_imperative_handle,
    use_insertion_effect: update_insertion_effect,
    use_layout_effect: update_layout_effect,
    use_memo: update_memo,
    use_reducer: update_reducer,
    use_ref: update_ref,
    use_state: update_state,
    use_debug_value: UPDATE_DEBUG_VALUE,
    use_deferred_value: update_deferred_value,
    use_transition: update_transition,
    use_sync_external_store: update_sync_external_store,
    use_id: update_id,
    use_host_transition_status,
    use_form_state: update_action_state,
    use_action_state: update_action_state,
    use_optimistic: update_optimistic,
    use_memo_cache,
    use_cache_refresh: update_refresh,
    use_effect_event: update_event,
};

#[cfg_attr(rust_js, rust_js::name = "HooksDispatcherOnRerender")]
static HOOKS_DISPATCHER_ON_RERENDER: Dispatcher = Dispatcher {
    read_context: read_context::<Option<&'static Unknown>>,

    r#use,
    use_callback: update_callback,
    use_context: read_context::<Option<&'static Unknown>>,
    use_effect: update_effect,
    use_imperative_handle: update_imperative_handle,
    use_insertion_effect: update_insertion_effect,
    use_layout_effect: update_layout_effect,
    use_memo: update_memo,
    use_reducer: rerender_reducer,
    use_ref: update_ref,
    use_state: rerender_state,
    use_debug_value: UPDATE_DEBUG_VALUE,
    use_deferred_value: rerender_deferred_value,
    use_transition: rerender_transition,
    use_sync_external_store: update_sync_external_store,
    use_id: update_id,
    use_host_transition_status,
    use_form_state: rerender_action_state,
    use_action_state: rerender_action_state,
    use_optimistic: rerender_optimistic,
    use_memo_cache,
    use_cache_refresh: update_refresh,
    use_effect_event: update_event,
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "HooksDispatcherOnMountInDEV")]
    static HOOKS_DISPATCHER_ON_MOUNT_IN_DEV: RefCell<Option<Rc<Dispatcher>>> =
        const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::name = "HooksDispatcherOnMountWithHookTypesInDEV")]
    static HOOKS_DISPATCHER_ON_MOUNT_WITH_HOOK_TYPES_IN_DEV: RefCell<Option<Rc<Dispatcher>>> =
        const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::name = "HooksDispatcherOnUpdateInDEV")]
    static HOOKS_DISPATCHER_ON_UPDATE_IN_DEV: RefCell<Option<Rc<Dispatcher>>> =
        const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::name = "HooksDispatcherOnRerenderInDEV")]
    static HOOKS_DISPATCHER_ON_RERENDER_IN_DEV: RefCell<Option<Rc<Dispatcher>>> =
        const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::name = "InvalidNestedHooksDispatcherOnMountInDEV")]
    static INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV: RefCell<Option<Rc<Dispatcher>>> =
        const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::name = "InvalidNestedHooksDispatcherOnUpdateInDEV")]
    static INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV: RefCell<Option<Rc<Dispatcher>>> =
        const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::name = "InvalidNestedHooksDispatcherOnRerenderInDEV")]
    static INVALID_NESTED_HOOKS_DISPATCHER_ON_RERENDER_IN_DEV: RefCell<Option<Rc<Dispatcher>>> =
        const { RefCell::new(js::null()) };
}

js::on_load! {
    if __DEV__ {
        fn warn_invalid_context_access() {
            console::error((concat!(
                "Context can only be read while React is rendering. ",
                "In classes, you can read it in the render method or getDerivedStateFromProps. ",
                "In function components, you can read it directly in the function body, but not ",
                "inside Hooks like useReducer() or useMemo()."
            ),));
        }

        fn warn_invalid_hook_access() {
            console::error((concat!(
                "Do not call Hooks inside useEffect(...), useMemo(...), or other built-in Hooks. ",
                "You can only call Hooks at the top level of your React function. ",
                "For more information, see ",
                "https://react.dev/link/rules-of-hooks"
            ),));
        }

        HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.set(Some(Rc::new(Dispatcher {
            read_context: |context| read_context(context),
            r#use,
            use_callback: |callback, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCallback));
                mount_hook_types_dev();
                check_deps_are_array_dev(deps.as_ref());
                mount_callback(callback, deps)
            },
            use_context: |context| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseContext));
                mount_hook_types_dev();
                read_context(context)
            },
            use_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffect));
                mount_hook_types_dev();
                check_deps_are_array_dev(deps.as_ref());
                mount_effect(create, deps)
            },
            use_imperative_handle: |r#ref, create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseImperativeHandle));
                mount_hook_types_dev();
                check_deps_are_array_dev(deps.as_ref());
                mount_imperative_handle(r#ref, create, deps)
            },
            use_insertion_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseInsertionEffect));
                mount_hook_types_dev();
                check_deps_are_array_dev(deps.as_ref());
                mount_insertion_effect(create, deps)
            },
            use_layout_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseLayoutEffect));
                mount_hook_types_dev();
                check_deps_are_array_dev(deps.as_ref());
                mount_layout_effect(create, deps)
            },
            use_memo: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseMemo));
                mount_hook_types_dev();
                check_deps_are_array_dev(deps.as_ref());
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_memo(create, deps)
            },
            use_reducer: |reducer, initial_arg, init| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseReducer));
                mount_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_reducer(reducer, initial_arg, init)
            },
            use_ref: |initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseRef));
                mount_hook_types_dev();
                mount_ref(initial_value)
            },
            use_state: |initial_state| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseState));
                mount_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_state(initial_state)
            },
            use_debug_value: |value, formatter_fn| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDebugValue));
                mount_hook_types_dev();
                mount_debug_value(value, formatter_fn)
            },
            use_deferred_value: |value, initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDeferredValue));
                mount_hook_types_dev();
                mount_deferred_value(value, initial_value)
            },
            use_transition: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseTransition));
                mount_hook_types_dev();
                mount_transition()
            },
            use_sync_external_store: |subscribe, get_snapshot, get_server_snapshot| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseSyncExternalStore));
                mount_hook_types_dev();
                mount_sync_external_store(subscribe, get_snapshot, get_server_snapshot)
            },
            use_id: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseId));
                mount_hook_types_dev();
                mount_id()
            },
            use_form_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseFormState));
                mount_hook_types_dev();
                warn_on_use_form_state_in_dev();
                mount_action_state(action, initial_state, permalink)
            },
            use_action_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseActionState));
                mount_hook_types_dev();
                mount_action_state(action, initial_state, permalink)
            },
            use_optimistic: |passthrough, reducer| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseOptimistic));
                mount_hook_types_dev();
                mount_optimistic(passthrough, reducer)
            },
            use_host_transition_status,
            use_memo_cache,
            use_cache_refresh: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCacheRefresh));
                mount_hook_types_dev();
                mount_refresh()
            },
            use_effect_event: |callback| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffectEvent));
                mount_hook_types_dev();
                mount_event(callback)
            },
        })));

        HOOKS_DISPATCHER_ON_MOUNT_WITH_HOOK_TYPES_IN_DEV.set(Some(Rc::new(Dispatcher {
            read_context: |context| read_context(context),
            r#use,
            use_callback: |callback, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCallback));
                update_hook_types_dev();
                mount_callback(callback, deps)
            },
            use_context: |context| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseContext));
                update_hook_types_dev();
                read_context(context)
            },
            use_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffect));
                update_hook_types_dev();
                mount_effect(create, deps)
            },
            use_imperative_handle: |r#ref, create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseImperativeHandle));
                update_hook_types_dev();
                mount_imperative_handle(r#ref, create, deps)
            },
            use_insertion_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseInsertionEffect));
                update_hook_types_dev();
                mount_insertion_effect(create, deps)
            },
            use_layout_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseLayoutEffect));
                update_hook_types_dev();
                mount_layout_effect(create, deps)
            },
            use_memo: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseMemo));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_memo(create, deps)
            },
            use_reducer: |reducer, initial_arg, init| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseReducer));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_reducer(reducer, initial_arg, init)
            },
            use_ref: |initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseRef));
                update_hook_types_dev();
                mount_ref(initial_value)
            },
            use_state: |initial_state| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseState));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_state(initial_state)
            },
            use_debug_value: |value, formatter_fn| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDebugValue));
                update_hook_types_dev();
                mount_debug_value(value, formatter_fn)
            },
            use_deferred_value: |value, initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDeferredValue));
                update_hook_types_dev();
                mount_deferred_value(value, initial_value)
            },
            use_transition: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseTransition));
                update_hook_types_dev();
                mount_transition()
            },
            use_sync_external_store: |subscribe, get_snapshot, get_server_snapshot| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseSyncExternalStore));
                update_hook_types_dev();
                mount_sync_external_store(subscribe, get_snapshot, get_server_snapshot)
            },
            use_id: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseId));
                update_hook_types_dev();
                mount_id()
            },
            use_form_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseFormState));
                update_hook_types_dev();
                warn_on_use_form_state_in_dev();
                mount_action_state(action, initial_state, permalink)
            },
            use_action_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseActionState));
                update_hook_types_dev();
                mount_action_state(action, initial_state, permalink)
            },
            use_optimistic: |passthrough, reducer| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseOptimistic));
                update_hook_types_dev();
                mount_optimistic(passthrough, reducer)
            },
            use_host_transition_status,
            use_memo_cache,
            use_cache_refresh: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCacheRefresh));
                update_hook_types_dev();
                mount_refresh()
            },
            use_effect_event: |callback| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffectEvent));
                update_hook_types_dev();
                mount_event(callback)
            },
        })));

        HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.set(Some(Rc::new(Dispatcher {
            read_context: |context| read_context(context),
            r#use,
            use_callback: |callback, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCallback));
                update_hook_types_dev();
                update_callback(callback, deps)
            },
            use_context: |context| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseContext));
                update_hook_types_dev();
                read_context(context)
            },
            use_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffect));
                update_hook_types_dev();
                update_effect(create, deps)
            },
            use_imperative_handle: |r#ref, create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseImperativeHandle));
                update_hook_types_dev();
                update_imperative_handle(r#ref, create, deps)
            },
            use_insertion_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseInsertionEffect));
                update_hook_types_dev();
                update_insertion_effect(create, deps)
            },
            use_layout_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseLayoutEffect));
                update_hook_types_dev();
                update_layout_effect(create, deps)
            },
            use_memo: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseMemo));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_memo(create, deps)
            },
            use_reducer: |reducer, initial_arg, init| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseReducer));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_reducer(reducer, initial_arg, init)
            },
            use_ref: |initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseRef));
                update_hook_types_dev();
                update_ref(initial_value)
            },
            use_state: |initial_state| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseState));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_state(initial_state)
            },
            use_debug_value: |value, formatter_fn| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDebugValue));
                update_hook_types_dev();
                UPDATE_DEBUG_VALUE(value, formatter_fn)
            },
            use_deferred_value: |value, initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDeferredValue));
                update_hook_types_dev();
                update_deferred_value(value, initial_value)
            },
            use_transition: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseTransition));
                update_hook_types_dev();
                update_transition()
            },
            use_sync_external_store: |subscribe, get_snapshot, get_server_snapshot| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseSyncExternalStore));
                update_hook_types_dev();
                update_sync_external_store(subscribe, get_snapshot, get_server_snapshot)
            },
            use_id: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseId));
                update_hook_types_dev();
                update_id()
            },
            use_form_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseFormState));
                update_hook_types_dev();
                warn_on_use_form_state_in_dev();
                update_action_state(action, initial_state, permalink)
            },
            use_action_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseActionState));
                update_hook_types_dev();
                update_action_state(action, initial_state, permalink)
            },
            use_optimistic: |passthrough, reducer| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseOptimistic));
                update_hook_types_dev();
                update_optimistic(passthrough, reducer)
            },
            use_host_transition_status,
            use_memo_cache,
            use_cache_refresh: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCacheRefresh));
                update_hook_types_dev();
                update_refresh()
            },
            use_effect_event: |callback| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffectEvent));
                update_hook_types_dev();
                update_event(callback)
            },
        })));

        HOOKS_DISPATCHER_ON_RERENDER_IN_DEV.set(Some(Rc::new(Dispatcher {
            read_context: |context| read_context(context),
            r#use,
            use_callback: |callback, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCallback));
                update_hook_types_dev();
                update_callback(callback, deps)
            },
            use_context: |context| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseContext));
                update_hook_types_dev();
                read_context(context)
            },
            use_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffect));
                update_hook_types_dev();
                update_effect(create, deps)
            },
            use_imperative_handle: |r#ref, create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseImperativeHandle));
                update_hook_types_dev();
                update_imperative_handle(r#ref, create, deps)
            },
            use_insertion_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseInsertionEffect));
                update_hook_types_dev();
                update_insertion_effect(create, deps)
            },
            use_layout_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseLayoutEffect));
                update_hook_types_dev();
                update_layout_effect(create, deps)
            },
            use_memo: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseMemo));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_RERENDER_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_memo(create, deps)
            },
            use_reducer: |reducer, initial_arg, init| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseReducer));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_RERENDER_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                rerender_reducer(reducer, initial_arg, init)
            },
            use_ref: |initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseRef));
                update_hook_types_dev();
                update_ref(initial_value)
            },
            use_state: |initial_state| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseState));
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_RERENDER_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                rerender_state(initial_state)
            },
            use_debug_value: |value, formatter_fn| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDebugValue));
                update_hook_types_dev();
                UPDATE_DEBUG_VALUE(value, formatter_fn)
            },
            use_deferred_value: |value, initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDeferredValue));
                update_hook_types_dev();
                rerender_deferred_value(value, initial_value)
            },
            use_transition: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseTransition));
                update_hook_types_dev();
                rerender_transition()
            },
            use_sync_external_store: |subscribe, get_snapshot, get_server_snapshot| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseSyncExternalStore));
                update_hook_types_dev();
                update_sync_external_store(subscribe, get_snapshot, get_server_snapshot)
            },
            use_id: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseId));
                update_hook_types_dev();
                update_id()
            },
            use_form_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseFormState));
                update_hook_types_dev();
                warn_on_use_form_state_in_dev();
                rerender_action_state(action, initial_state, permalink)
            },
            use_action_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseActionState));
                update_hook_types_dev();
                rerender_action_state(action, initial_state, permalink)
            },
            use_optimistic: |passthrough, reducer| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseOptimistic));
                update_hook_types_dev();
                rerender_optimistic(passthrough, reducer)
            },
            use_host_transition_status,
            use_memo_cache,
            use_cache_refresh: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCacheRefresh));
                update_hook_types_dev();
                update_refresh()
            },
            use_effect_event: |callback| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffectEvent));
                update_hook_types_dev();
                update_event(callback)
            },
        })));

        INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.set(Some(Rc::new(Dispatcher {
                read_context: |context| {
                    warn_invalid_context_access();
                    read_context(context)
                },
            r#use: |usable| {
                warn_invalid_hook_access();
                r#use(usable)
            },
            use_callback: |callback, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCallback));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_callback(callback, deps)
            },
            use_context: |context| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseContext));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                read_context(context)
            },
            use_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffect));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_effect(create, deps)
            },
            use_imperative_handle: |r#ref, create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseImperativeHandle));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_imperative_handle(r#ref, create, deps)
            },
            use_insertion_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseInsertionEffect));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_insertion_effect(create, deps)
            },
            use_layout_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseLayoutEffect));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_layout_effect(create, deps)
            },
            use_memo: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseMemo));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_memo(create, deps)
            },
            use_reducer: |reducer, initial_arg, init| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseReducer));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_reducer(reducer, initial_arg, init)
            },
            use_ref: |initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseRef));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_ref(initial_value)
            },
            use_state: |initial_state| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseState));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_MOUNT_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                mount_state(initial_state)
            },
            use_debug_value: |value, formatter_fn| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDebugValue));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_debug_value(value, formatter_fn)
            },
            use_deferred_value: |value, initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDeferredValue));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_deferred_value(value, initial_value)
            },
            use_transition: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseTransition));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_transition()
            },
            use_sync_external_store: |subscribe, get_snapshot, get_server_snapshot| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseSyncExternalStore));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_sync_external_store(subscribe, get_snapshot, get_server_snapshot)
            },
            use_id: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseId));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_id()
            },
            use_form_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseFormState));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_action_state(action, initial_state, permalink)
            },
            use_action_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseActionState));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_action_state(action, initial_state, permalink)
            },
            use_optimistic: |passthrough, reducer| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseOptimistic));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_optimistic(passthrough, reducer)
            },
            use_memo_cache: |size| {
                warn_invalid_hook_access();
                use_memo_cache(size)
            },
            use_host_transition_status,
            use_cache_refresh: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCacheRefresh));
                mount_hook_types_dev();
                mount_refresh()
            },
            use_effect_event: |callback| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffectEvent));
                warn_invalid_hook_access();
                mount_hook_types_dev();
                mount_event(callback)
            },
        })));

        INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.set(Some(Rc::new(Dispatcher {
                read_context: |context| {
                    warn_invalid_context_access();
                    read_context(context)
                },
            r#use: |usable| {
                warn_invalid_hook_access();
                r#use(usable)
            },
            use_callback: |callback, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCallback));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_callback(callback, deps)
            },
            use_context: |context| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseContext));
                warn_invalid_hook_access();
                update_hook_types_dev();
                read_context(context)
            },
            use_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffect));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_effect(create, deps)
            },
            use_imperative_handle: |r#ref, create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseImperativeHandle));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_imperative_handle(r#ref, create, deps)
            },
            use_insertion_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseInsertionEffect));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_insertion_effect(create, deps)
            },
            use_layout_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseLayoutEffect));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_layout_effect(create, deps)
            },
            use_memo: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseMemo));
                warn_invalid_hook_access();
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_memo(create, deps)
            },
            use_reducer: |reducer, initial_arg, init| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseReducer));
                warn_invalid_hook_access();
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_reducer(reducer, initial_arg, init)
            },
            use_ref: |initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseRef));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_ref(initial_value)
            },
            use_state: |initial_state| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseState));
                warn_invalid_hook_access();
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_state(initial_state)
            },
            use_debug_value: |value, formatter_fn| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDebugValue));
                warn_invalid_hook_access();
                update_hook_types_dev();
                UPDATE_DEBUG_VALUE(value, formatter_fn)
            },
            use_deferred_value: |value, initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDeferredValue));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_deferred_value(value, initial_value)
            },
            use_transition: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseTransition));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_transition()
            },
            use_sync_external_store: |subscribe, get_snapshot, get_server_snapshot| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseSyncExternalStore));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_sync_external_store(subscribe, get_snapshot, get_server_snapshot)
            },
            use_id: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseId));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_id()
            },
            use_form_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseFormState));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_action_state(action, initial_state, permalink)
            },
            use_action_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseActionState));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_action_state(action, initial_state, permalink)
            },
            use_optimistic: |passthrough, reducer| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseOptimistic));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_optimistic(passthrough, reducer)
            },
            use_memo_cache: |size| {
                warn_invalid_hook_access();
                use_memo_cache(size)
            },
            use_host_transition_status,
            use_cache_refresh: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCacheRefresh));
                update_hook_types_dev();
                update_refresh()
            },
            use_effect_event: |callback| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffectEvent));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_event(callback)
            },
        })));

        INVALID_NESTED_HOOKS_DISPATCHER_ON_RERENDER_IN_DEV.set(Some(Rc::new(Dispatcher {
                read_context: |context| {
                    warn_invalid_context_access();
                    read_context(context)
                },
            r#use: |usable| {
                warn_invalid_hook_access();
                r#use(usable)
            },
            use_callback: |callback, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCallback));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_callback(callback, deps)
            },
            use_context: |context| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseContext));
                warn_invalid_hook_access();
                update_hook_types_dev();
                read_context(context)
            },
            use_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffect));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_effect(create, deps)
            },
            use_imperative_handle: |r#ref, create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseImperativeHandle));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_imperative_handle(r#ref, create, deps)
            },
            use_insertion_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseInsertionEffect));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_insertion_effect(create, deps)
            },
            use_layout_effect: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseLayoutEffect));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_layout_effect(create, deps)
            },
            use_memo: |create, deps| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseMemo));
                warn_invalid_hook_access();
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                update_memo(create, deps)
            },
            use_reducer: |reducer, initial_arg, init| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseReducer));
                warn_invalid_hook_access();
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                rerender_reducer(reducer, initial_arg, init)
            },
            use_ref: |initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseRef));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_ref(initial_value)
            },
            use_state: |initial_state| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseState));
                warn_invalid_hook_access();
                update_hook_types_dev();
                let prev_dispatcher = REACT_SHARED_INTERNALS.h();
                INVALID_NESTED_HOOKS_DISPATCHER_ON_UPDATE_IN_DEV.with_borrow(|d| REACT_SHARED_INTERNALS.set_h(d.as_deref()));
                let _restore = RestoreDispatcher(prev_dispatcher);
                rerender_state(initial_state)
            },
            use_debug_value: |value, formatter_fn| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDebugValue));
                warn_invalid_hook_access();
                update_hook_types_dev();
                UPDATE_DEBUG_VALUE(value, formatter_fn)
            },
            use_deferred_value: |value, initial_value| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseDeferredValue));
                warn_invalid_hook_access();
                update_hook_types_dev();
                rerender_deferred_value(value, initial_value)
            },
            use_transition: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseTransition));
                warn_invalid_hook_access();
                update_hook_types_dev();
                rerender_transition()
            },
            use_sync_external_store: |subscribe, get_snapshot, get_server_snapshot| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseSyncExternalStore));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_sync_external_store(subscribe, get_snapshot, get_server_snapshot)
            },
            use_id: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseId));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_id()
            },
            use_form_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseFormState));
                warn_invalid_hook_access();
                update_hook_types_dev();
                rerender_action_state(action, initial_state, permalink)
            },
            use_action_state: |action, initial_state, permalink| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseActionState));
                warn_invalid_hook_access();
                update_hook_types_dev();
                rerender_action_state(action, initial_state, permalink)
            },
            use_optimistic: |passthrough, reducer| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseOptimistic));
                warn_invalid_hook_access();
                update_hook_types_dev();
                rerender_optimistic(passthrough, reducer)
            },
            use_memo_cache: |size| {
                warn_invalid_hook_access();
                use_memo_cache(size)
            },
            use_host_transition_status,
            use_cache_refresh: || {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseCacheRefresh));
                update_hook_types_dev();
                update_refresh()
            },
            use_effect_event: |callback| {
                CURRENT_HOOK_NAME_IN_DEV.set(Some(HookType::UseEffectEvent));
                warn_invalid_hook_access();
                update_hook_types_dev();
                update_event(callback)
            },
        })));

    }
}
