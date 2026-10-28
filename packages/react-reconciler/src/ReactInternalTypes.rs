// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use js::{JsError, JsObject, Map, Set, Unknown, WeakMap};
use react_isomorphic::react_context::ReactContext;
use react_isomorphic::react_create_ref::RefObject;
use react_isomorphic::react_jsx_element::ReactKey;
use react_isomorphic::react_start_transition::Transition;
use shared::react_shared_internals::HooksDispatcher;
use webapi::ConsoleTask;

pub use shared::react_shared_internals::AsyncDispatcher;

use crate::react_fiber_cache_component::Cache;
use crate::react_fiber_concurrent_updates::ConcurrentUpdate;
use crate::react_fiber_flags::Flags;
use crate::react_fiber_gesture_scheduler::ScheduledGesture;
use crate::react_fiber_hooks::{
    Action, ActionStateAndDispatch, CacheRefresh, Create, DebugValueFormatter, Deps, EventFunction,
    EventFunctionImpl, GetSnapshot, MemoCreate, Reducer, StartTransition, StateAndDispatch,
    Subscribe,
};
use crate::react_fiber_lane::{Lane, LaneMap, Lanes};
use crate::react_fiber_suspense_component::Wakeable;
use crate::react_fiber_tracing_marker_component::{SuspenseInfo, TracingMarkerInstance};
use crate::react_root_tags::RootTag;
use crate::react_type_of_mode::TypeOfMode;
use crate::react_work_tags::WorkTag;

// Unwind Circular: moved from ReactFiberHooks.old
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookType {
    #[cfg_attr(rust_js, rust_js::name = "useState")]
    UseState,
    #[cfg_attr(rust_js, rust_js::name = "useReducer")]
    UseReducer,
    #[cfg_attr(rust_js, rust_js::name = "useContext")]
    UseContext,
    #[cfg_attr(rust_js, rust_js::name = "useRef")]
    UseRef,
    #[cfg_attr(rust_js, rust_js::name = "useEffect")]
    UseEffect,
    #[cfg_attr(rust_js, rust_js::name = "useEffectEvent")]
    UseEffectEvent,
    #[cfg_attr(rust_js, rust_js::name = "useInsertionEffect")]
    UseInsertionEffect,
    #[cfg_attr(rust_js, rust_js::name = "useLayoutEffect")]
    UseLayoutEffect,
    #[cfg_attr(rust_js, rust_js::name = "useCallback")]
    UseCallback,
    #[cfg_attr(rust_js, rust_js::name = "useMemo")]
    UseMemo,
    #[cfg_attr(rust_js, rust_js::name = "useImperativeHandle")]
    UseImperativeHandle,
    #[cfg_attr(rust_js, rust_js::name = "useDebugValue")]
    UseDebugValue,
    #[cfg_attr(rust_js, rust_js::name = "useDeferredValue")]
    UseDeferredValue,
    #[cfg_attr(rust_js, rust_js::name = "useTransition")]
    UseTransition,
    #[cfg_attr(rust_js, rust_js::name = "useSyncExternalStore")]
    UseSyncExternalStore,
    #[cfg_attr(rust_js, rust_js::name = "useId")]
    UseId,
    #[cfg_attr(rust_js, rust_js::name = "useCacheRefresh")]
    UseCacheRefresh,
    #[cfg_attr(rust_js, rust_js::name = "useOptimistic")]
    UseOptimistic,
    #[cfg_attr(rust_js, rust_js::name = "useFormState")]
    UseFormState,
    #[cfg_attr(rust_js, rust_js::name = "useActionState")]
    UseActionState,
}

// Safety: a JS string, the hook's name.
unsafe impl js::Defined for HookType {}

impl HookType {
    /// The hook's name, as React's warnings print it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            HookType::UseState => "useState",
            HookType::UseReducer => "useReducer",
            HookType::UseContext => "useContext",
            HookType::UseRef => "useRef",
            HookType::UseEffect => "useEffect",
            HookType::UseEffectEvent => "useEffectEvent",
            HookType::UseInsertionEffect => "useInsertionEffect",
            HookType::UseLayoutEffect => "useLayoutEffect",
            HookType::UseCallback => "useCallback",
            HookType::UseMemo => "useMemo",
            HookType::UseImperativeHandle => "useImperativeHandle",
            HookType::UseDebugValue => "useDebugValue",
            HookType::UseDeferredValue => "useDeferredValue",
            HookType::UseTransition => "useTransition",
            HookType::UseSyncExternalStore => "useSyncExternalStore",
            HookType::UseId => "useId",
            HookType::UseCacheRefresh => "useCacheRefresh",
            HookType::UseOptimistic => "useOptimistic",
            HookType::UseFormState => "useFormState",
            HookType::UseActionState => "useActionState",
        }
    }
}

/// A context a fiber read, and the value it read of it.
pub struct ContextDependency<T: 'static> {
    pub context: Rc<ReactContext<T>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub next: RefCell<Option<Rc<ContextDependency<Option<&'static Unknown>>>>>,
    pub memoized_value: Cell<T>,
}

/// What a fiber depends on: the contexts it read, and the lanes they
/// changed in.
pub struct Dependencies {
    pub lanes: Cell<Lanes>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub first_context: RefCell<Option<Rc<ContextDependency<Option<&'static Unknown>>>>>,
    // DEV-only: ReactFiberThenable's `ThenableState`.
    #[cfg_attr(rust_js, rust_js::name = "_debugThenableState")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_thenable_state: Cell<Option<&'static Unknown>>,
}

/// One of a memo cache's arrays, which a compiled component reads and writes.
pub type MemoCacheArray = Rc<RefCell<Vec<Option<&'static Unknown>>>>;

pub struct MemoCache {
    pub data: Rc<RefCell<Vec<MemoCacheArray>>>,
    pub index: Cell<usize>,
}

/// What a fiber's `ref` is: a function called with the instance, or an
/// object whose `current` is set to it.
#[derive(Clone, Copy)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Ref {
    // What it gives is the cleanup it's called with `null` in place of, if
    // it's a function.
    Callback(&'static dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>),
    // shared/ReactTypes' `RefObject`.
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Object(&'static Unknown),
}

// Safety: a ref is a function or an object, never `undefined` nor `null`.
unsafe impl js::Defined for Ref {}

/// A fiber's deletions: a shared array, as upstream's live one, which
/// deletion work reads while it may grow.
pub type Deletions = Rc<RefCell<Vec<Rc<Fiber>>>>;

// A Fiber is work on a Component that needs to be done or was done. There can
// be more than one per component.
pub struct Fiber {
    // These first fields are conceptually members of an Instance. This used to
    // be split into a separate type and intersected with the other Fiber fields,
    // but until Flow fixes its intersection bugs, we've merged them into a
    // single type.

    // An Instance is shared between all versions of a component. We can easily
    // break this out into a separate object to avoid copying so much to the
    // alternate versions of the tree. We put this on a single object for now to
    // minimize the number of objects created during the initial render.

    // Tag identifying the type of fiber.
    pub tag: Cell<WorkTag>,

    // Unique identifier of this child.
    pub key: RefCell<Option<ReactKey>>,

    // The value of element.type which is used to preserve the identity during
    // reconciliation of this child.
    pub element_type: Cell<Option<&'static Unknown>>,

    // The resolved function/class/ associated with this fiber.
    pub r#type: Cell<Option<&'static Unknown>>,

    // The local state associated with this fiber.
    pub state_node: Cell<Option<&'static Unknown>>,

    // Conceptual aliases
    // parent : Instance -> return The parent happens to be the same as the
    // return fiber since we've merged the fiber and instance.

    // Remaining fields belong to Fiber

    // The Fiber to return to after finishing processing this one.
    // This is effectively the parent, but there can be multiple parents (two)
    // so this is only the parent of the thing we're currently processing.
    // It is conceptually the same as the return address of a stack frame.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#return: RefCell<Option<Rc<Fiber>>>,

    // Singly Linked List Tree Structure.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub child: RefCell<Option<Rc<Fiber>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub sibling: RefCell<Option<Rc<Fiber>>>,
    pub index: Cell<usize>,

    // The ref last used to attach this node.
    // I'll avoid adding an owner field for prod and model that as functions.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#ref: Cell<Option<Ref>>,

    pub ref_cleanup: Cell<Option<&'static Unknown>>,

    // Input is the data coming into process this fiber. Arguments. Props.
    pub pending_props: Cell<Option<&'static Unknown>>, // This type will be more specific once we overload the tag.
    pub memoized_props: Cell<Option<&'static Unknown>>, // The props used to create the output.

    // A queue of state updates and callbacks.
    pub update_queue: Cell<Option<&'static Unknown>>,

    // The state used to create the output
    pub memoized_state: Cell<Option<&'static Unknown>>,

    // Dependencies (contexts, events) for this fiber, if it has any
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub dependencies: RefCell<Option<Rc<Dependencies>>>,

    // Bitfield that describes properties about the fiber and its subtree. E.g.
    // the ConcurrentMode flag indicates whether the subtree should be async-by-
    // default. When a fiber is created, it inherits the mode of its
    // parent. Additional flags can be set at creation time, but after that the
    // value should remain unchanged throughout the fiber's lifetime, particularly
    // before its child fibers are created.
    pub mode: Cell<TypeOfMode>,

    // Effect
    pub flags: Cell<Flags>,
    pub subtree_flags: Cell<Flags>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub deletions: RefCell<Option<Deletions>>,

    pub lanes: Cell<Lanes>,
    pub child_lanes: Cell<Lanes>,

    // This is a pooled version of a Fiber. Every fiber that gets updated will
    // eventually have a pair. There are cases when we can clean up pairs to save
    // memory if we need to.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub alternate: RefCell<Option<Rc<Fiber>>>,

    // Time spent rendering this Fiber and its descendants for the current update.
    // This tells us how well the tree makes use of sCU for memoization.
    // It is reset to 0 each time we render and only updated when we don't bailout.
    // This field is only set when the enableProfilerTimer flag is enabled.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub actual_duration: Cell<Option<f64>>,

    // If the Fiber is currently active in the "render" phase,
    // This marks the time at which the work began.
    // This field is only set when the enableProfilerTimer flag is enabled.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub actual_start_time: Cell<Option<f64>>,

    // Duration of the most recent render time for this Fiber.
    // This value is not updated when we bailout for memoization purposes.
    // This field is only set when the enableProfilerTimer flag is enabled.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub self_base_duration: Cell<Option<f64>>,

    // Sum of base times for all descendants of this Fiber.
    // This value bubbles up during the "complete" phase.
    // This field is only set when the enableProfilerTimer flag is enabled.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub tree_base_duration: Cell<Option<f64>>,

    // Conceptual aliases
    // workInProgress : Fiber ->  alternate The alternate used for reuse happens
    // to be the same as work in progress.
    // __DEV__ only
    #[cfg_attr(rust_js, rust_js::name = "_debugInfo")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_info: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::name = "_debugOwner")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_owner: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::name = "_debugStack")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_stack: Cell<Option<&'static JsError>>,
    #[cfg_attr(rust_js, rust_js::name = "_debugTask")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_task: Cell<Option<&'static ConsoleTask>>,
    #[cfg_attr(rust_js, rust_js::name = "_debugNeedsRemount")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_needs_remount: Cell<Option<bool>>,

    // Used to verify that the order of hooks does not change between renders.
    #[cfg_attr(rust_js, rust_js::name = "_debugHookTypes")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_hook_types: RefCell<Option<Rc<RefCell<Vec<HookType>>>>>,
}

// A fiber is itself: two are equal only where they're one, as a JS `Set`
// of them holds each.
impl PartialEq for Fiber {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Fiber {}

impl Hash for Fiber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

/// What an uncaught or a recoverable error is reported with.
pub struct ErrorInfo {
    pub component_stack: Option<String>,
}

/// What a caught error is reported with.
pub struct CaughtErrorInfo {
    pub component_stack: Option<String>,
    pub error_boundary: Option<&'static Unknown>,
}

pub struct SuspenseHydrationCallbacks {
    // ReactFiberConfig's `SuspenseInstance | ActivityInstance`.
    pub on_hydrated: Option<&'static dyn Fn(&'static Unknown)>,
    pub on_deleted: Option<&'static dyn Fn(&'static Unknown)>,
}

/// What a transition's tracing deleted.
pub struct TransitionTracingDeletion {
    pub r#type: String,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub name: Option<String>,
    pub end_time: f64,
}

/// What a root reports an uncaught or a recoverable error with.
pub type OnError = dyn Fn(Option<&'static Unknown>, &ErrorInfo);

/// What a root reports an error an error boundary caught with.
pub type OnCaughtError = dyn Fn(Option<&'static Unknown>, &CaughtErrorInfo);

/// What a root shows a default Transition's indicator with, which gives
/// what hides it, if anything.
pub type OnDefaultTransitionIndicator = dyn Fn() -> Option<&'static dyn Fn()>;

pub type OnTransitionStart = dyn Fn(&str, f64);
pub type OnTransitionProgress = dyn Fn(&str, f64, f64, &[SuspenseInfo]);
pub type OnTransitionIncomplete = dyn Fn(&str, f64, &[TransitionTracingDeletion]);
pub type OnTransitionComplete = dyn Fn(&str, f64, f64);
pub type OnMarkerProgress = dyn Fn(&str, &str, f64, f64, &[SuspenseInfo]);
pub type OnMarkerIncomplete = dyn Fn(&str, &str, f64, &[TransitionTracingDeletion]);
pub type OnMarkerComplete = dyn Fn(&str, &str, f64, f64);

pub struct TransitionTracingCallbacks {
    pub on_transition_start: Option<&'static OnTransitionStart>,
    pub on_transition_progress: Option<&'static OnTransitionProgress>,
    pub on_transition_incomplete: Option<&'static OnTransitionIncomplete>,
    pub on_transition_complete: Option<&'static OnTransitionComplete>,
    pub on_marker_progress: Option<&'static OnMarkerProgress>,
    pub on_marker_incomplete: Option<&'static OnMarkerIncomplete>,
    pub on_marker_complete: Option<&'static OnMarkerComplete>,
}

// Exported FiberRoot type includes all properties,
// To avoid requiring potentially error-prone :any casts throughout the project.
// The types are defined separately within this file to ensure they stay in sync.
pub struct FiberRoot {
    // The type of root (legacy, batched, concurrent, etc.)
    pub tag: RootTag,

    // Any additional information from the host associated with this root.
    // ReactFiberConfig's `Container`.
    pub container_info: &'static Unknown,
    // Used only by persistent updates.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending_children: Cell<Option<&'static Unknown>>,
    // The currently active root fiber. This is the mutable root of the tree.
    pub current: RefCell<Rc<Fiber>>,

    // A `WeakMap` where the host has one, a `Map` where it hasn't.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub ping_cache: Cell<Option<&'static WeakMap<Wakeable, &'static Set<Lanes>>>>,

    // Timeout handle returned by setTimeout. Used to cancel a pending timeout, if
    // it's superseded by a new one.
    // ReactFiberConfig's `TimeoutHandle | NoTimeout`.
    pub timeout_handle: Cell<&'static Unknown>,
    // When a root has a pending commit scheduled, calling this function will
    // cancel it.
    // TODO: Can this be consolidated with timeoutHandle?
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub cancel_pending_commit: RefCell<Option<Box<dyn Fn()>>>,
    // Top context object, used by renderSubtreeIntoContainer
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub context: Cell<Option<&'static JsObject>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending_context: Cell<Option<&'static JsObject>>,

    // Used to create a linked list that represent all the roots that have
    // pending work scheduled on them.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub next: RefCell<Option<Rc<FiberRoot>>>,

    // Node returned by Scheduler.scheduleCallback. Represents the next rendering
    // task that the root will work on.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub callback_node: Cell<Option<&'static Unknown>>,
    pub callback_priority: Cell<Lane>,
    pub expiration_times: RefCell<LaneMap<f64>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub hidden_updates: RefCell<LaneMap<Option<Vec<Rc<ConcurrentUpdate>>>>>,

    pub pending_lanes: Cell<Lanes>,
    pub suspended_lanes: Cell<Lanes>,
    pub pinged_lanes: Cell<Lanes>,
    pub warm_lanes: Cell<Lanes>,
    pub expired_lanes: Cell<Lanes>,
    pub indicator_lanes: Cell<Lanes>, // enableDefaultTransitionIndicator only
    pub error_recovery_disabled_lanes: Cell<Lanes>,
    pub shell_suspend_counter: Cell<u32>,

    pub entangled_lanes: Cell<Lanes>,
    pub entanglements: RefCell<LaneMap<Lanes>>,

    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pooled_cache: RefCell<Option<Rc<Cache>>>,
    pub pooled_cache_lanes: Cell<Lanes>,

    // TODO: In Fizz, id generation is specific to each server config. Maybe we
    // should do this in Fiber, too? Deferring this decision for now because
    // there's no other place to store the prefix except for an internal field on
    // the public createRoot object, which the fiber tree does not currently have
    // a reference to.
    pub identifier_prefix: String,

    pub on_uncaught_error: &'static OnError,
    pub on_caught_error: &'static OnCaughtError,
    pub on_recoverable_error: &'static OnError,

    // enableDefaultTransitionIndicator only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub on_default_transition_indicator: Cell<Option<&'static OnDefaultTransitionIndicator>>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub pending_indicator: Cell<Option<&'static dyn Fn()>>,

    // shared/ReactTypes' `ReactFormState<any, any>`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub form_state: Option<&'static Unknown>,

    // enableViewTransition only
    // react/src/ReactTransitionType's `TransitionTypes`.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub transition_types: RefCell<Option<Rc<RefCell<Vec<String>>>>>, // TODO: Make this a LaneMap.
    // enableGestureTransition only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub pending_gestures: RefCell<Option<Rc<ScheduledGesture>>>,
    // ReactFiberConfig's `Instance`.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub gesture_clone: Cell<Option<&'static Unknown>>,

    // The follow fields are only used by enableSuspenseCallback for hydration.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub hydration_callbacks: Cell<Option<&'static SuspenseHydrationCallbacks>>,

    // The following attributes are only used by DevTools and are only present in DEV builds.
    // They enable DevTools Profiler UI to show which Fiber(s) scheduled a given commit.
    pub memoized_updaters: RefCell<HashSet<Rc<Fiber>>>,
    pub pending_updaters_lane_map: RefCell<LaneMap<HashSet<Rc<Fiber>>>>,

    // The following fields are only used in transition tracing in Profile builds
    // react/src/ReactStartTransition's `Transition`s.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub transition_callbacks: Cell<Option<&'static TransitionTracingCallbacks>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transition_lanes: RefCell<LaneMap<Option<&'static Set<Transition>>>>,
    // Transitions on the root can be represented as a bunch of tracing markers.
    // Each entangled group of transitions can be treated as a tracing marker.
    // It will have a set of pending suspense boundaries. These transitions
    // are considered complete when the pending suspense boundaries set is
    // empty. We can represent this as a Map of transitions to suspense
    // boundary sets
    pub incomplete_transitions: &'static Map<Transition, Rc<TracingMarkerInstance>>,

    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub effect_duration: Cell<Option<f64>>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub passive_effect_duration: Cell<Option<f64>>,

    // __DEV__ only
    #[cfg_attr(rust_js, rust_js::name = "_debugRootType")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_root_type: Cell<Option<&'static str>>,
}

unsafe impl js::Defined for FiberRoot {}

pub struct Dispatcher {
    pub r#use: fn(Option<&'static Unknown>) -> Option<&'static Unknown>,
    pub read_context: fn(&Rc<ReactContext<Option<&'static Unknown>>>) -> Option<&'static Unknown>,
    pub use_state: fn(Option<&'static Unknown>) -> StateAndDispatch,
    pub use_reducer: fn(
        &'static Reducer,
        Option<&'static Unknown>,
        Option<&'static Unknown>,
    ) -> StateAndDispatch,
    pub use_context: fn(&Rc<ReactContext<Option<&'static Unknown>>>) -> Option<&'static Unknown>,
    pub use_ref: fn(Option<&'static Unknown>) -> Rc<RefObject>,
    pub use_effect: fn(Rc<Create>, Option<Deps>),
    pub use_effect_event: fn(&'static EventFunctionImpl) -> Rc<EventFunction>,
    pub use_insertion_effect: fn(Rc<Create>, Option<Deps>),
    pub use_layout_effect: fn(Rc<Create>, Option<Deps>),
    pub use_callback: fn(Option<&'static Unknown>, Option<Deps>) -> Option<&'static Unknown>,
    pub use_memo: fn(&'static MemoCreate, Option<Deps>) -> Option<&'static Unknown>,
    // The ref: an object whose `current` is set, a function called with the
    // handle, or nothing; and what creates the handle.
    pub use_imperative_handle: fn(Option<&'static Unknown>, Option<&'static Unknown>, Option<Deps>),
    pub use_debug_value: fn(Option<&'static Unknown>, Option<&'static DebugValueFormatter>),
    pub use_deferred_value:
        fn(Option<&'static Unknown>, Option<&'static Unknown>) -> Option<&'static Unknown>,
    pub use_transition: fn() -> (bool, Rc<StartTransition>),
    pub use_sync_external_store: fn(
        &'static Subscribe,
        &'static GetSnapshot,
        // A function, or undefined where none was given.
        Option<&'static Unknown>,
    ) -> Option<&'static Unknown>,
    pub use_id: fn() -> String,
    pub use_cache_refresh: fn() -> Rc<CacheRefresh>,
    pub use_memo_cache: fn(usize) -> MemoCacheArray,
    // ReactFiberConfig's `TransitionStatus`.
    pub use_host_transition_status: fn() -> Option<&'static Unknown>,
    // The passthrough, and the reducer, if it's a function.
    pub use_optimistic: fn(Option<&'static Unknown>, Option<&'static Unknown>) -> StateAndDispatch,
    pub use_form_state:
        fn(&'static Action, Option<&'static Unknown>, Option<String>) -> ActionStateAndDispatch,
    pub use_action_state:
        fn(&'static Action, Option<&'static Unknown>, Option<String>) -> ActionStateAndDispatch,
}

// Safety: a JS object, `ReactSharedInternals.H`.
unsafe impl js::Defined for Dispatcher {}

impl HooksDispatcher for Dispatcher {}
