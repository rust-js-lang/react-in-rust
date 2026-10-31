// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This is a renderer of React that doesn't have a render target output.
// It is useful to demonstrate the internals of the reconciler in isolation
// and for testing semantics of reconciliation separate from the host
// environment.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{
    JsError, JsObject, Kind, Map, Promise, PropertyDescriptor, Rest, Unknown, function, object,
};
use react_isomorphic::react_start_transition::{set_t, t};
use react_reconciler::react_event_priorities::EventPriority;
use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;
use react_reconciler::react_internal_types::{
    CaughtErrorInfo, ErrorInfo, FiberRoot, OnCaughtError, OnDefaultTransitionIndicator, OnError,
    TransitionTracingCallbacks,
};
use react_reconciler::react_reconciler_constants::{
    CONCURRENT_ROOT, DEFAULT_EVENT_PRIORITY, DISCRETE_EVENT_PRIORITY, IDLE_EVENT_PRIORITY,
    LEGACY_ROOT, NO_EVENT_PRIORITY,
};
use react_reconciler::react_root_tags::RootTag;
use shared::check_string_coercion::check_prop_string_coercion;
use shared::environment::__DEV__;
use shared::is_array::is_array;
use shared::react_feature_flags::DISABLE_LEGACY_MODE;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use shared::react_symbols::{REACT_ELEMENT_TYPE, REACT_FRAGMENT_TYPE};
use shared::react_types::ReactPortal;
#[cfg_attr(rust_js, rust_js::name = "ReactVersion")]
use shared::react_version::REACT_VERSION;
use webapi::console;

#[cfg_attr(rust_js, rust_js::name = "DefaultConfig")]
use crate::react_fiber_config_noop::ReactFiberConfigNoop as DefaultConfig;
use crate::react_fiber_config_noop::{
    Child, Container, HostContext, Instance, PublicInstance, TextInstance,
};

unsafe extern "Rust" {
    #[link_name = "scheduler/unstable_mock#* as Scheduler"]
    safe static SCHEDULER: &'static JsObject;
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "scheduler/unstable_mock#* as Scheduler.unstable_now"
)]
fn unstable_now() -> f64 {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "scheduler/unstable_mock#* as Scheduler.unstable_flushNumberOfYields"
)]
fn unstable_flush_number_of_yields(count: f64) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "scheduler/unstable_mock#* as Scheduler.unstable_clearLog"
)]
fn unstable_clear_log() -> Vec<Option<&'static Unknown>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "scheduler/unstable_mock#* as Scheduler.unstable_advanceTime"
)]
fn unstable_advance_time(ms: f64) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "scheduler/unstable_mock#* as Scheduler.unstable_flushExpired"
)]
fn unstable_flush_expired() -> Vec<Option<&'static Unknown>> {
    unreachable!()
}

/// A host component's props, as JS gives them.
pub struct Props {
    pub prop: Option<&'static Unknown>,
    pub hidden: Option<bool>,
    pub children: Option<&'static Unknown>,
    pub bottom: Option<f64>,
    pub left: Option<f64>,
    pub right: Option<f64>,
    pub top: Option<f64>,
    // `string` to Flow, and tested so.
    pub src: Option<&'static Unknown>,
}

unsafe impl js::Defined for Props {}

/// What `createRoot` is given.
pub struct CreateRootOptions {
    #[cfg_attr(rust_js, rust_js::name = "unstable_transitionCallbacks")]
    pub unstable_transition_callbacks: Option<&'static TransitionTracingCallbacks>,
    pub on_uncaught_error: Option<&'static OnError>,
    pub on_caught_error: Option<&'static OnCaughtError>,
    pub on_default_transition_indicator: Option<&'static OnDefaultTransitionIndicator>,
}

unsafe impl js::Defined for CreateRootOptions {}

type InstanceMeasurement = Option<&'static Unknown>;

pub struct SuspenseyCommitSubscription {
    pub pending_count: Cell<f64>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub commit: Cell<Option<&'static dyn Fn()>>,
}

pub type SuspendedState = SuspenseyCommitSubscription;

pub type TransitionStatus = Unknown;

pub type FormInstance = Instance;

pub type RunningViewTransition = Unknown;

pub type ViewTransitionInstance = Unknown;

pub type GestureTimeline = Unknown;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "NO_CONTEXT")]
    static NO_CONTEXT: &'static HostContext = object::spread(None);
    #[cfg_attr(rust_js, rust_js::name = "UPPERCASE_CONTEXT")]
    static UPPERCASE_CONTEXT: &'static HostContext = object::spread(None);
}

js::on_load! {
    if __DEV__ {
        object::freeze(NO_CONTEXT.with(|context| *context));
    }
}

/// `Container | Instance`, a parent of host children.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ContainerOrInstance<'a> {
    Container(&'a Container),
    Instance(&'a Instance),
}

type SuspenseyThingCache = Map<str, Rc<SuspenseyThingRecord>>;

struct SuspenseyThingRecord {
    status: Cell<&'static str>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    subscriptions: RefCell<Option<Vec<Rc<SuspenseyCommitSubscription>>>>,
}

/// What `stopTrackingHostCounters` gives: the counter of the mode.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum HostCounters {
    Mutation(MutationCounters),
    Persistence(PersistenceCounters),
}

pub struct MutationCounters {
    pub host_update_counter: f64,
}

pub struct PersistenceCounters {
    pub host_clone_counter: f64,
}

// The ReactNoop object's functions, by what they take and give.
type OfRoot = dyn Fn(Option<&str>);
type RootChildren = dyn Fn(Option<&str>) -> Option<Rc<RefCell<Vec<Child>>>>;
type RootJsx = dyn Fn(Option<&str>) -> Option<&'static Unknown>;
type GetRoot = dyn Fn(Option<&str>) -> Option<Rc<FiberRoot>>;
type GetOrCreateRootContainer = dyn Fn(Option<&str>, RootTag) -> &'static Container;
type CreateRoot = dyn Fn(Option<&'static CreateRootOptions>) -> Root;
type SuspenseyThingStatus = dyn Fn(&str) -> Option<&'static str>;
type OfId = dyn Fn(&str);
type CreatePortal =
    dyn Fn(Option<&'static Unknown>, &'static Container, Option<&'static Unknown>) -> ReactPortal;
type Callback = dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>;
type BatchedUpdates = dyn Fn(&Callback, Option<&'static Unknown>) -> Option<&'static Unknown>;
type DeferredUpdates = dyn Fn(&dyn Fn() -> Option<&'static Unknown>) -> Option<&'static Unknown>;
type DiscreteCallback = dyn Fn(
    Option<&'static Unknown>,
    Option<&'static Unknown>,
    Option<&'static Unknown>,
    Option<&'static Unknown>,
) -> Option<&'static Unknown>;
type DiscreteUpdates = dyn Fn(
    &DiscreteCallback,
    Option<&'static Unknown>,
    Option<&'static Unknown>,
    Option<&'static Unknown>,
    Option<&'static Unknown>,
) -> Option<&'static Unknown>;
type Render = dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>);
type RenderToRootWithId = dyn Fn(Option<&'static Unknown>, &str, Option<&'static Unknown>);
type FindInstance = dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>;
type Flush = dyn Fn() -> Vec<Option<&'static Unknown>>;
type RunWithPriority =
    dyn Fn(EventPriority, &dyn Fn() -> Option<&'static Unknown>) -> Option<&'static Unknown>;
type IdleUpdates = dyn Fn(&dyn Fn() -> Option<&'static Unknown>);
type FlushSync = dyn Fn(Option<&dyn Fn() -> Option<&'static Unknown>>) -> Option<&'static Unknown>;
// A root's.
type RenderChildren = dyn Fn(Option<&'static Unknown>);
type Children = dyn Fn() -> Option<Rc<RefCell<Vec<Child>>>>;
type ChildrenJsx = dyn Fn() -> Option<&'static Unknown>;

/// The test renderer createReactNoop gives.
pub struct ReactNoop {
    #[cfg_attr(rust_js, rust_js::name = "_Scheduler")]
    pub scheduler: &'static JsObject,
    pub get_children: Box<OfRoot>,
    pub get_pending_children: Box<OfRoot>,
    /// Prefer using `getChildrenAsJSX`.
    /// Using the returned children in `.toEqual` has very poor performance on mismatch due to deep equality checking of fiber structures.
    /// Make sure you deeply remove enumerable properties before passing it to `.toEqual`, or, better, use `getChildrenAsJSX` or `toMatchRenderedOutput`.
    pub dangerously_get_children: Box<RootChildren>,
    /// Prefer using `getPendingChildrenAsJSX`.
    /// Using the returned children in `.toEqual` has very poor performance on mismatch due to deep equality checking of fiber structures.
    /// Make sure you deeply remove enumerable properties before passing it to `.toEqual`, or, better, use `getChildrenAsJSX` or `toMatchRenderedOutput`.
    pub dangerously_get_pending_children: Box<RootChildren>,
    pub get_or_create_root_container: Box<GetOrCreateRootContainer>,
    // TODO: Replace ReactNoop.render with createRoot + root.render
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
    // Shortcut for testing a single root
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
    // TODO: Remove this after callers migrate to alternatives.
    #[cfg_attr(rust_js, rust_js::name = "unstable_runWithPriority")]
    pub unstable_run_with_priority: Box<RunWithPriority>,
    pub batched_updates: Box<BatchedUpdates>,
    pub deferred_updates: Box<DeferredUpdates>,
    pub discrete_updates: Box<DiscreteUpdates>,
    pub idle_updates: Box<IdleUpdates>,
    pub flush_sync: Box<FlushSync>,
    pub flush_passive_effects: Box<dyn Fn() -> bool>,
    // Logs the current state of the tree.
    pub dump_tree: Box<OfRoot>,
    pub get_root: Box<GetRoot>,
}

/// What `createRoot` gives.
pub struct Root {
    #[cfg_attr(rust_js, rust_js::name = "_Scheduler")]
    pub scheduler: &'static JsObject,
    pub render: Box<RenderChildren>,
    pub get_children: Box<Children>,
    #[cfg_attr(rust_js, rust_js::name = "getChildrenAsJSX")]
    pub get_children_as_jsx: Box<ChildrenJsx>,
}

/// What `createLegacyRoot` gives.
pub struct LegacyRoot {
    #[cfg_attr(rust_js, rust_js::name = "_Scheduler")]
    pub scheduler: &'static JsObject,
    pub render: Box<RenderChildren>,
    pub get_children: Box<Children>,
    #[cfg_attr(rust_js, rust_js::name = "getChildrenAsJSX")]
    pub get_children_as_jsx: Box<ChildrenJsx>,
    pub legacy: bool,
}

type CommitCallback = Box<dyn Fn(&'static dyn Fn()) -> Box<dyn Fn()>>;
type ScheduleMicrotask = fn(Box<dyn FnOnce()>);

// The host configs' functions, by what they take and give.
type CreateInstance = dyn Fn(
    &str,
    &'static Props,
    &'static Container,
    &'static HostContext,
    &'static Unknown,
) -> Rc<Instance>;
type CreateTextInstance =
    dyn Fn(&str, &'static Container, &'static HostContext, &'static Unknown) -> Rc<TextInstance>;
type ScheduleTimeout = fn(Box<dyn FnOnce()>, Option<f64>) -> &'static js::TimeoutId;
type PreloadInstance = dyn Fn(&'static Instance, &str, &'static Props) -> bool;
type SuspendInstance = dyn Fn(&Rc<SuspendedState>, &'static Instance, &str, &'static Props);
type CommitUpdate = dyn Fn(&'static Instance, &str, Option<&'static Props>, &'static Props);
type CommitTextUpdate = dyn Fn(&'static TextInstance, &str, &str);
type StartViewTransition = fn(
    &'static Container,
    Option<&'static Unknown>,
    &'static dyn Fn(),
    &'static dyn Fn(),
    &'static dyn Fn(),
    &'static dyn Fn(),
    &'static dyn Fn() -> Option<&'static Unknown>,
    &'static dyn Fn(Option<&'static Unknown>),
    &'static dyn Fn(&str), // Profiling-only
    &'static dyn Fn(),     // Profiling-only
) -> Option<&'static RunningViewTransition>;
type StartGestureTransition = fn(
    &'static Container,
    Option<&'static GestureTimeline>,
    f64,
    f64,
    Option<&'static Unknown>,
    &'static dyn Fn(),
    &'static dyn Fn(),
    &'static dyn Fn(Option<&'static Unknown>),
) -> Option<&'static RunningViewTransition>;
type CloneHiddenInstance = dyn Fn(&'static Instance, &str, &'static Props) -> Rc<Instance>;

/// The host config's functions every mode shares.
struct SharedHostConfig {
    renderer_version: &'static str,
    renderer_package_name: &'static str,
    #[cfg_attr(rust_js, rust_js::nullable)]
    extra_dev_tools_config: Option<&'static Unknown>,
    get_root_host_context: fn() -> &'static HostContext,
    get_child_host_context: fn(&'static HostContext, &str) -> &'static HostContext,
    get_public_instance: fn(&'static Instance) -> &'static PublicInstance,
    #[cfg_attr(rust_js, rust_js::name = "HostTransitionContext", rust_js::nullable)]
    host_transition_context: Option<&'static Unknown>,
    create_instance: Box<CreateInstance>,
    append_initial_child: fn(&Instance, Child),
    finalize_initial_children: fn(&Instance, &str, &'static Props) -> bool,
    should_set_text_content: fn(&str, &'static Props) -> bool,
    create_text_instance: Box<CreateTextInstance>,
    create_fragment_instance: fn(Option<&'static Unknown>) -> Option<&'static Unknown>,
    update_fragment_instance_fiber: fn(Option<&'static Unknown>, Option<&'static Unknown>),
    commit_new_child_to_fragment_instance: fn(Option<&'static Unknown>, Option<&'static Unknown>),
    delete_child_from_fragment_instance: fn(Option<&'static Unknown>, Option<&'static Unknown>),
    schedule_timeout: ScheduleTimeout,
    cancel_timeout: fn(Option<&js::TimeoutId>),
    no_timeout: f64,
    supports_microtasks: bool,
    schedule_microtask: ScheduleMicrotask,
    prepare_for_commit: fn() -> Option<&'static Unknown>,
    reset_after_commit: fn(),
    set_current_update_priority: Box<dyn Fn(EventPriority)>,
    get_current_update_priority: Box<dyn Fn() -> EventPriority>,
    resolve_update_priority: Box<dyn Fn() -> EventPriority>,
    track_scheduler_event: fn(),
    resolve_event_type: fn() -> Option<String>,
    resolve_event_time_stamp: fn() -> f64,
    should_attempt_eager_transition: fn() -> bool,
    is_primary_renderer: bool,
    warns_if_not_acting: bool,
    get_instance_from_node: fn() -> !,
    before_active_instance_blur: fn(),
    after_active_instance_blur: fn(),
    prepare_portal_mount: fn(),
    detach_deleted_instance: fn(),
    request_post_paint_callback: fn(&dyn Fn(f64)),
    may_suspend_commit: fn(&str, &'static Props) -> bool,
    may_suspend_commit_on_update: fn(&str, &'static Props, &'static Props) -> bool,
    may_suspend_commit_in_sync_render: fn(&str, &'static Props) -> bool,
    preload_instance: Box<PreloadInstance>,
    start_suspending_commit: fn() -> Rc<SuspendedState>,
    suspend_instance: Box<SuspendInstance>,
    suspend_on_active_view_transition: fn(&SuspendedState, &'static Container),
    wait_for_commit_to_be_ready: fn(&Rc<SuspendedState>, f64) -> Option<CommitCallback>,
    get_suspended_commit_reason: fn(&SuspendedState, &'static Container) -> Option<String>,
    #[cfg_attr(rust_js, rust_js::name = "NotPendingTransition", rust_js::nullable)]
    not_pending_transition: Option<&'static TransitionStatus>,
    reset_form_instance: fn(&'static Instance),
    bind_to_console: fn(&str, &[Option<&'static Unknown>], &str) -> &'static Unknown,
}

unsafe impl js::Defined for SharedHostConfig {}

/// The host config's functions of the mutation mode.
struct MutationHostConfig {
    supports_mutation: bool,
    clone_mutable_instance: fn() -> !,
    clone_mutable_text_instance: fn() -> !,
    commit_mount: fn(&'static Instance, &str, &'static Props),
    commit_update: Box<CommitUpdate>,
    commit_text_update: Box<CommitTextUpdate>,
    append_child: fn(&'static Instance, Child),
    append_child_to_container: fn(&'static Container, Child),
    insert_before: fn(&'static Instance, Child, Child),
    insert_in_container_before: fn(&'static Container, Child, Child),
    remove_child: fn(&'static Instance, Child),
    remove_child_from_container: fn(&'static Container, Child),
    clear_container: fn(&'static Container),
    hide_instance: fn(&'static Instance),
    hide_text_instance: fn(&'static TextInstance),
    unhide_instance: fn(&'static Instance, &'static Props),
    unhide_text_instance: fn(&'static TextInstance, &str),
    apply_view_transition_name: fn(&'static Instance, &str, Option<&str>),
    restore_view_transition_name: fn(&'static Instance, &'static Props),
    cancel_view_transition_name: fn(&'static Instance, &str, &'static Props),
    cancel_root_view_transition_name: fn(&'static Container),
    restore_root_view_transition_name: fn(&'static Container),
    clone_root_view_transition_container: fn(&'static Container) -> !,
    remove_root_view_transition_clone: fn(&'static Container, &'static Instance) -> !,
    measure_instance: fn(&'static Instance) -> InstanceMeasurement,
    measure_cloned_instance: fn(&'static Instance) -> InstanceMeasurement,
    was_instance_in_viewport: fn(InstanceMeasurement) -> bool,
    has_instance_changed: fn(InstanceMeasurement, InstanceMeasurement) -> bool,
    has_instance_affected_parent: fn(InstanceMeasurement, InstanceMeasurement) -> bool,
    start_view_transition: StartViewTransition,
    start_gesture_transition: StartGestureTransition,
    stop_view_transition: fn(Option<&'static RunningViewTransition>),
    add_view_transition_finished_listener:
        fn(Option<&'static RunningViewTransition>, &'static dyn Fn()),
    create_view_transition_instance: fn(&str) -> Option<&'static ViewTransitionInstance>,
    get_current_gesture_offset: fn(Option<&'static GestureTimeline>) -> f64,
    reset_text_content: fn(&'static Instance),
}

unsafe impl js::Defined for MutationHostConfig {}

/// The host config's functions of the persistence mode.
struct PersistenceHostConfig {
    supports_persistence: bool,
    clone_instance: Rc<CloneInstance>,
    create_container_child_set: fn() -> Rc<RefCell<Vec<Child>>>,
    append_child_to_container_child_set: fn(&RefCell<Vec<Child>>, Child),
    finalize_container_children: fn(&'static Container, Rc<RefCell<Vec<Child>>>),
    replace_container_children: fn(&'static Container, Rc<RefCell<Vec<Child>>>),
    clone_hidden_instance: Box<CloneHiddenInstance>,
    clone_hidden_text_instance: fn(&'static TextInstance, &str) -> Rc<TextInstance>,
}

unsafe impl js::Defined for PersistenceHostConfig {}

type CloneInstance = dyn Fn(
    &Instance,
    &str,
    Option<&'static Props>,
    &'static Props,
    bool,
    Option<Rc<RefCell<Vec<Child>>>>,
) -> Rc<Instance>;

/// Puts back what `idleUpdates`, `unstable_runWithPriority` or `flushSync`
/// changed, as upstream's `finally` does.
struct RestorePriority<'a> {
    priority: &'a Cell<EventPriority>,
    previous: EventPriority,
}

impl Drop for RestorePriority<'_> {
    fn drop(&mut self) {
        self.priority.set(self.previous);
    }
}

fn create_react_noop(
    reconciler: &dyn Fn(&JsObject) -> &'static ReactFiberReconciler,
    use_mutation: bool,
) -> ReactNoop {
    let instance_counter = Rc::new(Cell::new(0.0));
    let host_update_counter = Rc::new(Cell::new(0.0));
    let host_clone_counter = Rc::new(Cell::new(0.0));

    fn append_child_to_container_or_instance(parent_instance: ContainerOrInstance, child: Child) {
        let prev_parent = match &child {
            Child::Instance(instance) => instance.parent.get(),
            Child::Text(text_instance) => text_instance.parent.get(),
        };
        // A container has no `id`.
        let parent_id = match parent_instance {
            ContainerOrInstance::Container(_) => None,
            ContainerOrInstance::Instance(instance) => Some(instance.id),
        };

        if prev_parent != Some(-1.0) && prev_parent != parent_id {
            panic!("Reparenting is not allowed");
        }

        match &child {
            Child::Instance(instance) => instance.parent.set(parent_id),
            Child::Text(text_instance) => text_instance.parent.set(parent_id),
        }
        let children = match parent_instance {
            ContainerOrInstance::Container(container) => Rc::clone(&container.children.borrow()),
            ContainerOrInstance::Instance(instance) => Rc::clone(&instance.children),
        };
        let index = children
            .borrow()
            .iter()
            .position(|c| js::unknown(c) == js::unknown(&child));
        if let Some(index) = index {
            children.borrow_mut().remove(index);
        }
        children.borrow_mut().push(child);
    }

    fn append_child_to_container(parent_instance: &'static Container, child: Child) {
        if js::type_of(js::get(js::unknown(parent_instance), "rootID")) != "string" {
            // Some calls to this aren't typesafe.
            // This helps surface mistakes in tests.
            panic!("appendChildToContainer() first argument is not a container.");
        }
        append_child_to_container_or_instance(
            ContainerOrInstance::Container(parent_instance),
            child,
        );
    }

    fn append_child(parent_instance: &'static Instance, child: Child) {
        if js::type_of(js::get(js::unknown(parent_instance), "rootID")) == "string" {
            // Some calls to this aren't typesafe.
            // This helps surface mistakes in tests.
            panic!("appendChild() first argument is not an instance.");
        }
        append_child_to_container_or_instance(
            ContainerOrInstance::Instance(parent_instance),
            child,
        );
    }

    fn insert_in_container_or_instance_before(
        parent_instance: ContainerOrInstance,
        child: Child,
        before_child: Child,
    ) {
        let children = match parent_instance {
            ContainerOrInstance::Container(container) => Rc::clone(&container.children.borrow()),
            ContainerOrInstance::Instance(instance) => Rc::clone(&instance.children),
        };
        let index = children
            .borrow()
            .iter()
            .position(|c| js::unknown(c) == js::unknown(&child));
        if let Some(index) = index {
            children.borrow_mut().remove(index);
        }
        let before_index = children
            .borrow()
            .iter()
            .position(|c| js::unknown(c) == js::unknown(&before_child));
        let Some(before_index) = before_index else {
            panic!("This child does not exist.");
        };
        children.borrow_mut().insert(before_index, child);
    }

    fn insert_in_container_before(
        parent_instance: &'static Container,
        child: Child,
        before_child: Child,
    ) {
        if js::type_of(js::get(js::unknown(parent_instance), "rootID")) != "string" {
            // Some calls to this aren't typesafe.
            // This helps surface mistakes in tests.
            panic!("insertInContainerBefore() first argument is not a container.");
        }
        insert_in_container_or_instance_before(
            ContainerOrInstance::Container(parent_instance),
            child,
            before_child,
        );
    }

    fn insert_before(parent_instance: &'static Instance, child: Child, before_child: Child) {
        if js::type_of(js::get(js::unknown(parent_instance), "rootID")) == "string" {
            // Some calls to this aren't typesafe.
            // This helps surface mistakes in tests.
            panic!("insertBefore() first argument is not an instance.");
        }
        insert_in_container_or_instance_before(
            ContainerOrInstance::Instance(parent_instance),
            child,
            before_child,
        );
    }

    fn clear_container(container: &'static Container) {
        container.children.borrow().borrow_mut().clear();
    }

    fn remove_child_from_container_or_instance(parent_instance: ContainerOrInstance, child: Child) {
        let children = match parent_instance {
            ContainerOrInstance::Container(container) => Rc::clone(&container.children.borrow()),
            ContainerOrInstance::Instance(instance) => Rc::clone(&instance.children),
        };
        let index = children
            .borrow()
            .iter()
            .position(|c| js::unknown(c) == js::unknown(&child));
        let Some(index) = index else {
            panic!("This child does not exist.");
        };
        children.borrow_mut().remove(index);
    }

    fn remove_child_from_container(parent_instance: &'static Container, child: Child) {
        if js::type_of(js::get(js::unknown(parent_instance), "rootID")) != "string" {
            // Some calls to this aren't typesafe.
            // This helps surface mistakes in tests.
            panic!("removeChildFromContainer() first argument is not a container.");
        }
        remove_child_from_container_or_instance(
            ContainerOrInstance::Container(parent_instance),
            child,
        );
    }

    fn remove_child(parent_instance: &'static Instance, child: Child) {
        if js::type_of(js::get(js::unknown(parent_instance), "rootID")) == "string" {
            // Some calls to this aren't typesafe.
            // This helps surface mistakes in tests.
            panic!("removeChild() first argument is not an instance.");
        }
        remove_child_from_container_or_instance(
            ContainerOrInstance::Instance(parent_instance),
            child,
        );
    }

    let clone_instance = {
        let host_clone_counter = Rc::clone(&host_clone_counter);
        move |instance: &Instance,
              r#type: &str,
              _old_props: Option<&'static Props>,
              new_props: &'static Props,
              keep_children: bool,
              children: Option<Rc<RefCell<Vec<Child>>>>|
              -> Rc<Instance> {
            if __DEV__ {
                check_prop_string_coercion(new_props.children, "children");
            }
            let clone = Instance {
                id: instance.id,
                r#type: r#type.to_owned(),
                parent: Cell::new(instance.parent.get()),
                children: if keep_children {
                    Rc::clone(&instance.children)
                } else {
                    children.unwrap_or_default()
                },
                text: RefCell::new(if should_set_text_content(r#type, new_props) {
                    Some(compute_text(
                        &js::concat_text(new_props.children).unwrap(),
                        instance.context,
                    ))
                } else {
                    js::null()
                }),
                prop: Cell::new(new_props.prop),
                hidden: Cell::new(new_props.hidden.is_some_and(|hidden| hidden)),
                context: instance.context,
                src: RefCell::new(None),
            };

            if r#type == "suspensey-thing"
                && let Kind::String(src) = js::classify(new_props.src)
            {
                clone.src.replace(Some(src.to_owned()));
            }

            object::define_property(
                js::unknown(&clone),
                "id",
                &PropertyDescriptor {
                    value: Some(&clone.id),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            object::define_property(
                js::unknown(&clone),
                "parent",
                &PropertyDescriptor {
                    value: Some(&clone.parent.get()),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            object::define_property(
                js::unknown(&clone),
                "text",
                &PropertyDescriptor {
                    value: Some(&{ clone.text.borrow().clone() }),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            object::define_property(
                js::unknown(&clone),
                "context",
                &PropertyDescriptor {
                    value: Some(clone.context),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            host_clone_counter.set(host_clone_counter.get() + 1.0);
            Rc::new(clone)
        }
    };

    fn should_set_text_content(r#type: &str, props: &Props) -> bool {
        if r#type == "errorInBeginPhase" {
            panic!("Error in host config.");
        }
        matches!(
            js::classify(props.children),
            Kind::String(_) | Kind::Number(_) | Kind::BigInt(_)
        )
    }

    fn compute_text(raw_text: &str, host_context: &HostContext) -> String {
        if UPPERCASE_CONTEXT.with(|context| std::ptr::eq(host_context, *context)) {
            raw_text.to_uppercase()
        } else {
            raw_text.to_owned()
        }
    }

    let suspensey_thing_cache: Rc<RefCell<Option<&'static SuspenseyThingCache>>> =
        Rc::new(RefCell::new(js::null()));

    fn start_suspending_commit() -> Rc<SuspendedState> {
        // Represents a subscription for all the suspensey things that block a
        // particular commit. Once they've all loaded, the commit phase can proceed.
        Rc::new(SuspenseyCommitSubscription {
            pending_count: Cell::new(0.0),
            commit: Cell::new(None),
        })
    }

    let suspend_instance = {
        let suspensey_thing_cache = Rc::clone(&suspensey_thing_cache);
        move |state: &Rc<SuspendedState>,
              _instance: &'static Instance,
              r#type: &str,
              props: &'static Props| {
            let src = props.src;
            if r#type == "suspensey-thing"
                && let Kind::String(src) = js::classify(src)
            {
                // Attach a listener to the suspensey thing and create a subscription
                // object that uses reference counting to track when all the suspensey
                // things have loaded.
                let record = suspensey_thing_cache.borrow().unwrap().get(src);
                let Some(record) = record else {
                    panic!("Could not find record for key.");
                };
                if record.status.get() == "fulfilled" {
                    // Already loaded.
                } else if record.status.get() == "pending" {
                    state.pending_count.set(state.pending_count.get() + 1.0);
                    // Stash the subscription on the record. In `resolveSuspenseyThing`,
                    // we'll use this fire the commit once all the things have loaded.
                    if record.subscriptions.borrow().is_none() {
                        record.subscriptions.replace(Some(Vec::new()));
                    }
                    record
                        .subscriptions
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .push(Rc::clone(state));
                }
            } else {
                panic!(
                    "Did not expect this host component to be visited when suspending the commit. Did you check the SuspendCommit flag?"
                );
            }
        }
    };

    fn wait_for_commit_to_be_ready(
        state: &Rc<SuspendedState>,
        _timeout_offset: f64,
    ) -> Option<CommitCallback> {
        if state.pending_count.get() > 0.0 {
            let state = Rc::clone(state);
            return Some(Box::new(
                move |commit: &'static dyn Fn()| -> Box<dyn Fn()> {
                    state.commit.set(Some(commit));
                    let state = Rc::clone(&state);
                    let cancel_commit = move || {
                        state.commit.set(None);
                    };
                    Box::new(cancel_commit)
                },
            ));
        }
        js::null()
    }

    let current_update_priority = Rc::new(Cell::new(NO_EVENT_PRIORITY));
    let current_event_priority = Rc::new(Cell::new(DEFAULT_EVENT_PRIORITY));

    let set_current_update_priority = {
        let current_update_priority = Rc::clone(&current_update_priority);
        move |new_priority: EventPriority| {
            current_update_priority.set(new_priority);
        }
    };

    let get_current_update_priority = {
        let current_update_priority = Rc::clone(&current_update_priority);
        move || -> EventPriority { current_update_priority.get() }
    };

    let shared_host_config = object::spread((
        Some(js::unknown(&DefaultConfig)),
        Some(js::unknown(&SharedHostConfig {
            renderer_version: REACT_VERSION,
            renderer_package_name: "react-noop",

            extra_dev_tools_config: None,

            get_root_host_context: || NO_CONTEXT.with(|context| *context),

            get_child_host_context: |parent_host_context, r#type| {
                if r#type == "offscreen" {
                    return parent_host_context;
                }
                if r#type == "uppercase" {
                    return UPPERCASE_CONTEXT.with(|context| *context);
                }
                NO_CONTEXT.with(|context| *context)
            },

            get_public_instance: |instance| instance,

            host_transition_context: None,

            create_instance: Box::new({
                let instance_counter = Rc::clone(&instance_counter);
                move |r#type,
                      props,
                      _root_container_instance,
                      host_context,
                      internal_instance_handle| {
                    if r#type == "errorInCompletePhase" {
                        panic!("Error in host config.");
                    }
                    if __DEV__ {
                        // The `if` statement here prevents auto-disabling of the safe coercion
                        // ESLint rule, so we must manually disable it below.
                        if should_set_text_content(r#type, props) {
                            check_prop_string_coercion(props.children, "children");
                        }
                    }
                    let id = instance_counter.get();
                    instance_counter.set(id + 1.0);
                    let inst = Instance {
                        id,
                        r#type: r#type.to_owned(),
                        children: Rc::new(RefCell::new(Vec::new())),
                        parent: Cell::new(Some(-1.0)),
                        text: RefCell::new(if should_set_text_content(r#type, props) {
                            Some(compute_text(
                                &js::concat_text(props.children).unwrap(),
                                host_context,
                            ))
                        } else {
                            js::null()
                        }),
                        prop: Cell::new(props.prop),
                        hidden: Cell::new(props.hidden.is_some_and(|hidden| hidden)),
                        context: host_context,
                        src: RefCell::new(None),
                    };

                    if r#type == "suspensey-thing"
                        && let Kind::String(src) = js::classify(props.src)
                    {
                        inst.src.replace(Some(src.to_owned()));
                    }

                    // Hide from unit tests
                    object::define_property(
                        js::unknown(&inst),
                        "id",
                        &PropertyDescriptor {
                            value: Some(&inst.id),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    object::define_property(
                        js::unknown(&inst),
                        "parent",
                        &PropertyDescriptor {
                            value: Some(&inst.parent.get()),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    object::define_property(
                        js::unknown(&inst),
                        "text",
                        &PropertyDescriptor {
                            value: Some(&{ inst.text.borrow().clone() }),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    object::define_property(
                        js::unknown(&inst),
                        "context",
                        &PropertyDescriptor {
                            value: Some(inst.context),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    object::define_property(
                        js::unknown(&inst),
                        "fiber",
                        &PropertyDescriptor {
                            value: Some(internal_instance_handle),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    Rc::new(inst)
                }
            }),

            append_initial_child: |parent_instance, child| {
                let prev_parent = match &child {
                    Child::Instance(instance) => instance.parent.get(),
                    Child::Text(text_instance) => text_instance.parent.get(),
                };
                if prev_parent != Some(-1.0) && prev_parent != Some(parent_instance.id) {
                    panic!("Reparenting is not allowed");
                }
                match &child {
                    Child::Instance(instance) => instance.parent.set(Some(parent_instance.id)),
                    Child::Text(text_instance) => {
                        text_instance.parent.set(Some(parent_instance.id))
                    }
                }
                parent_instance.children.borrow_mut().push(child);
            },

            finalize_initial_children: |_dom_element, _type, _props| false,

            should_set_text_content,

            create_text_instance: Box::new({
                let instance_counter = Rc::clone(&instance_counter);
                move |text, _root_container_instance, host_context, _internal_instance_handle| {
                    let mut text = text.to_owned();
                    if UPPERCASE_CONTEXT.with(|context| std::ptr::eq(host_context, *context)) {
                        text = text.to_uppercase();
                    }
                    let id = instance_counter.get();
                    instance_counter.set(id + 1.0);
                    let inst = TextInstance {
                        text: RefCell::new(text),
                        id,
                        parent: Cell::new(Some(-1.0)),
                        hidden: Cell::new(false),
                        context: host_context,
                    };
                    // Hide from unit tests
                    object::define_property(
                        js::unknown(&inst),
                        "id",
                        &PropertyDescriptor {
                            value: Some(&inst.id),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    object::define_property(
                        js::unknown(&inst),
                        "parent",
                        &PropertyDescriptor {
                            value: Some(&inst.parent.get()),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    object::define_property(
                        js::unknown(&inst),
                        "context",
                        &PropertyDescriptor {
                            value: Some(inst.context),
                            enumerable: Some(false),
                            ..Default::default()
                        },
                    );
                    Rc::new(inst)
                }
            }),

            create_fragment_instance: |_fragment_fiber| js::null(),

            update_fragment_instance_fiber: |_fragment_fiber, _fragment_instance| {
                // Noop
            },

            commit_new_child_to_fragment_instance: |_child, _fragment_instance| {
                // Noop
            },

            delete_child_from_fragment_instance: |_child, _fragment_instance| {
                // Noop
            },

            schedule_timeout: js::set_timeout,
            cancel_timeout: js::clear_timeout,
            no_timeout: -1.0,

            supports_microtasks: true,
            schedule_microtask: if js::is_function(webapi::global::queue_microtask) {
                webapi::global::queue_microtask
            } else if js::is_defined(js::class::<Promise<()>>) {
                |callback| {
                    js::promise::resolve(js::null::<&Unknown>())
                        .then::<(), _>(move |_| callback())
                        .catch(|error: &'static JsError| {
                            js::set_timeout(move || js::throw(error.as_unknown()), None);
                        });
                }
            } else {
                |callback| {
                    js::set_timeout(callback, None);
                }
            },

            prepare_for_commit: || js::null(),

            reset_after_commit: || {},

            set_current_update_priority: Box::new(set_current_update_priority.clone()),
            get_current_update_priority: Box::new(get_current_update_priority.clone()),

            resolve_update_priority: Box::new({
                let current_update_priority = Rc::clone(&current_update_priority);
                let current_event_priority = Rc::clone(&current_event_priority);
                move || {
                    if current_update_priority.get() != NO_EVENT_PRIORITY {
                        return current_update_priority.get();
                    }
                    current_event_priority.get()
                }
            }),

            track_scheduler_event: || {},

            resolve_event_type: || js::null(),

            resolve_event_time_stamp: || -1.1,

            should_attempt_eager_transition: || false,

            is_primary_renderer: true,
            warns_if_not_acting: true,

            get_instance_from_node: || panic!("Not yet implemented."),

            before_active_instance_blur: || {
                // NO-OP
            },

            after_active_instance_blur: || {
                // NO-OP
            },

            prepare_portal_mount: || {
                // NO-OP
            },

            detach_deleted_instance: || {},

            request_post_paint_callback: |callback| {
                let end_time = unstable_now();
                callback(end_time);
            },

            may_suspend_commit: |r#type, props| {
                // Asks whether it's possible for this combination of type and props
                // to ever need to suspend. This is different from asking whether it's
                // currently ready because even if it's ready now, it might get purged
                // from the cache later.
                r#type == "suspensey-thing" && matches!(js::classify(props.src), Kind::String(_))
            },

            may_suspend_commit_on_update: |r#type, old_props, new_props| {
                // Asks whether it's possible for this combination of type and props
                // to ever need to suspend. This is different from asking whether it's
                // currently ready because even if it's ready now, it might get purged
                // from the cache later.
                r#type == "suspensey-thing"
                    && matches!(js::classify(new_props.src), Kind::String(_))
                    && new_props.src != old_props.src
            },

            may_suspend_commit_in_sync_render: |_type, _props| true,

            preload_instance: Box::new({
                let suspensey_thing_cache = Rc::clone(&suspensey_thing_cache);
                move |_instance, r#type, props| {
                    let (true, Kind::String(src)) =
                        (r#type == "suspensey-thing", js::classify(props.src))
                    else {
                        panic!("Attempted to preload unexpected instance: {type}");
                    };

                    // In addition to preloading an instance, this method asks whether the
                    // instance is ready to be committed. If it's not, React may yield to the
                    // main thread and ask again. It's possible a load event will fire in
                    // between, in which case we can avoid showing a fallback.
                    if suspensey_thing_cache.borrow().is_none() {
                        suspensey_thing_cache.replace(Some(js::map::new(None)));
                    }
                    let record = suspensey_thing_cache.borrow().unwrap().get(src);
                    match record {
                        None => {
                            let new_record = Rc::new(SuspenseyThingRecord {
                                status: Cell::new("pending"),
                                subscriptions: RefCell::new(None),
                            });
                            suspensey_thing_cache.borrow().unwrap().set(src, new_record);
                            let on_load_start = js::get(js::unknown(props), "onLoadStart");
                            if let Kind::Function(on_load_start) = js::classify(on_load_start) {
                                on_load_start();
                            }
                            false
                        }
                        Some(record) => record.status.get() == "fulfilled",
                    }
                }
            }),

            start_suspending_commit,
            suspend_instance: Box::new(suspend_instance),

            suspend_on_active_view_transition: |_state, _container| {
                // Not implemented
            },

            wait_for_commit_to_be_ready,

            get_suspended_commit_reason: |_state, _root_container| js::null(),

            not_pending_transition: None,

            reset_form_instance: |_form| {},

            bind_to_console: |method_name, args, _badge_name| {
                let Kind::Function(method) = js::classify(js::get(console, method_name)) else {
                    panic!("console.{method_name} is no function");
                };
                js::unknown_of(function::bind(method, Some(&console), args))
            },
        })),
    ));

    let mutation_host_config = MutationHostConfig {
        supports_mutation: true,

        clone_mutable_instance: || {
            // required for enableGestureTransition
            panic!("Not yet implemented.");
        },

        clone_mutable_text_instance: || {
            // required for enableGestureTransition
            panic!("Not yet implemented.");
        },

        commit_mount: |_instance, _type, _new_props| {
            // Noop
        },

        commit_update: Box::new({
            let host_update_counter = Rc::clone(&host_update_counter);
            move |instance, r#type, old_props, new_props| {
                if old_props.is_none() {
                    panic!("Should have old props");
                }
                host_update_counter.set(host_update_counter.get() + 1.0);
                instance.prop.set(new_props.prop);
                instance
                    .hidden
                    .set(new_props.hidden.is_some_and(|hidden| hidden));

                if r#type == "suspensey-thing"
                    && let Kind::String(src) = js::classify(new_props.src)
                {
                    instance.src.replace(Some(src.to_owned()));
                }

                if should_set_text_content(r#type, new_props) {
                    if __DEV__ {
                        check_prop_string_coercion(new_props.children, "children");
                    }
                    instance.text.replace(Some(compute_text(
                        &js::concat_text(new_props.children).unwrap(),
                        instance.context,
                    )));
                }
            }
        }),

        commit_text_update: Box::new({
            let host_update_counter = Rc::clone(&host_update_counter);
            move |text_instance, _old_text, new_text| {
                host_update_counter.set(host_update_counter.get() + 1.0);
                text_instance
                    .text
                    .replace(compute_text(new_text, text_instance.context));
            }
        }),

        append_child,
        append_child_to_container,
        insert_before,
        insert_in_container_before,
        remove_child,
        remove_child_from_container,
        clear_container,

        hide_instance: |instance| {
            instance.hidden.set(true);
        },

        hide_text_instance: |text_instance| {
            text_instance.hidden.set(true);
        },

        unhide_instance: |instance, props| {
            if !props.hidden.unwrap_or(false) {
                instance.hidden.set(false);
            }
        },

        unhide_text_instance: |text_instance, _text| {
            text_instance.hidden.set(false);
        },

        apply_view_transition_name: |_instance, _name, _class_name| {},

        restore_view_transition_name: |_instance, _props| {},

        cancel_view_transition_name: |_instance, _name, _props| {},

        cancel_root_view_transition_name: |_root_container| {},

        restore_root_view_transition_name: |_root_container| {},

        clone_root_view_transition_container: |_root_container| {
            panic!("Not yet implemented.");
        },

        remove_root_view_transition_clone: |_root_container, _clone| {
            panic!("Not implemented.");
        },

        measure_instance: |_instance| js::null(),

        measure_cloned_instance: |_instance| js::null(),

        was_instance_in_viewport: |_measurement| true,

        has_instance_changed: |_old_measurement, _new_measurement| false,

        has_instance_affected_parent: |_old_measurement, _new_measurement| false,

        start_view_transition: |_root_container,
                                _transition_types,
                                mutation_callback,
                                layout_callback,
                                _after_mutation_callback,
                                spawned_work_callback,
                                _passive_callback,
                                _error_callback,
                                _blocked_callback, // Profiling-only
                                _finished_animation| {
            mutation_callback();
            layout_callback();
            // Skip afterMutationCallback(). We don't need it since we're not animating.
            spawned_work_callback();
            // Skip passiveCallback(). Spawned work will schedule a task.
            js::null()
        },

        start_gesture_transition: |_root_container,
                                   _timeline,
                                   _range_start,
                                   _range_end,
                                   _transition_types,
                                   mutation_callback,
                                   animate_callback,
                                   _error_callback| {
            mutation_callback();
            animate_callback();
            js::null()
        },

        stop_view_transition: |_transition| {},

        add_view_transition_finished_listener: |_transition, callback| {
            callback();
        },

        create_view_transition_instance: |_name| js::null(),

        get_current_gesture_offset: |_provider| 0.0,

        reset_text_content: |instance| {
            instance.text.replace(None);
        },
    };

    let persistence_host_config = PersistenceHostConfig {
        supports_persistence: true,

        clone_instance: Rc::new(clone_instance.clone()),

        create_container_child_set: || Rc::new(RefCell::new(Vec::new())),

        append_child_to_container_child_set: |child_set, child| {
            child_set.borrow_mut().push(child);
        },

        finalize_container_children: |container, new_children| {
            container.pending_children.replace(Rc::clone(&new_children));
            if new_children.borrow().len() == 1
                && match &new_children.borrow()[0] {
                    Child::Instance(instance) => {
                        instance.text.borrow().as_deref() == Some("Error when completing root")
                    }
                    Child::Text(text_instance) => {
                        *text_instance.text.borrow() == "Error when completing root"
                    }
                }
            {
                // Trigger an error for testing purposes
                panic!("Error when completing root");
            }
        },

        replace_container_children: |container, new_children| {
            container.children.replace(new_children);
        },

        clone_hidden_instance: Box::new(move |instance, r#type, props| {
            let clone = clone_instance(instance, r#type, Some(props), props, true, js::null());
            clone.hidden.set(true);
            clone
        }),

        clone_hidden_text_instance: |instance, _text| {
            let clone = TextInstance {
                text: RefCell::new(instance.text.borrow().clone()),
                id: instance.id,
                parent: Cell::new(instance.parent.get()),
                hidden: Cell::new(true),
                context: instance.context,
            };
            // Hide from unit tests
            object::define_property(
                js::unknown(&clone),
                "id",
                &PropertyDescriptor {
                    value: Some(&clone.id),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            object::define_property(
                js::unknown(&clone),
                "parent",
                &PropertyDescriptor {
                    value: Some(&clone.parent.get()),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            object::define_property(
                js::unknown(&clone),
                "context",
                &PropertyDescriptor {
                    value: Some(clone.context),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            Rc::new(clone)
        },
    };

    let host_config = if use_mutation {
        object::spread((
            Some(js::unknown(shared_host_config)),
            Some(js::unknown(&mutation_host_config)),
        ))
    } else {
        object::spread((
            Some(js::unknown(shared_host_config)),
            Some(js::unknown(&persistence_host_config)),
        ))
    };

    #[cfg_attr(rust_js, rust_js::name = "NoopRenderer")]
    let noop_renderer = reconciler(host_config);

    let root_containers: Rc<&'static Map<str, Rc<Container>>> = Rc::new(js::map::new(None));
    let roots: Rc<&'static Map<str, Rc<FiberRoot>>> = Rc::new(js::map::new(None));
    const DEFAULT_ROOT_ID: &str = "<default>";

    #[cfg_attr(rust_js, rust_js::name = "createJSXElementForTestComparison")]
    fn create_jsx_element_for_test_comparison(
        r#type: Option<&'static Unknown>,
        props: &'static Unknown,
    ) -> &'static Unknown {
        if __DEV__ {
            let element = JsxElementInDev {
                r#type,
                typeof_: REACT_ELEMENT_TYPE,
                key: js::null(),
                props,
                owner: js::null(),
                store: Some(object::spread(None)),
            };
            object::define_property(
                js::unknown(&element),
                "ref",
                &PropertyDescriptor {
                    value: Some(&js::null::<&Unknown>()),
                    enumerable: Some(false),
                    ..Default::default()
                },
            );
            js::unknown_of(element)
        } else {
            js::unknown_of(JsxElement {
                typeof_: REACT_ELEMENT_TYPE,
                r#type,
                key: js::null(),
                r#ref: js::null(),
                props,
            })
        }
    }

    #[cfg_attr(rust_js, rust_js::name = "childToJSX")]
    fn child_to_jsx(child: Option<JsxChild>, text: Option<&str>) -> Option<&'static Unknown> {
        if let Some(text) = text {
            return Some(js::unknown_of(text.to_owned()));
        }
        let Some(child) = child else {
            return js::null();
        };
        match child {
            JsxChild::Children(child) => {
                let child = child.borrow().clone();
                if child.is_empty() {
                    return js::null();
                }
                if child.len() == 1 {
                    return child_to_jsx(Some(JsxChild::Child(child[0].clone())), js::null());
                }
                let children: Vec<Option<&'static Unknown>> = child
                    .iter()
                    .map(|c| child_to_jsx(Some(JsxChild::Child(c.clone())), js::null()))
                    .collect();
                if children.iter().all(|c| {
                    matches!(
                        js::classify(*c),
                        Kind::String(_) | Kind::Number(_) | Kind::BigInt(_)
                    )
                }) {
                    return Some(js::unknown_of(js::array::join(&children, "")));
                }
                Some(js::unknown_of(children))
            }
            JsxChild::Child(Child::Instance(instance)) => {
                // This is an instance.
                let children = child_to_jsx(
                    Some(JsxChild::Children(Rc::clone(&instance.children))),
                    { instance.text.borrow().clone() }.as_deref(),
                );
                let mut props = InstanceProps {
                    prop: instance.prop.get(),
                    hidden: None,
                    src: None,
                    children: None,
                };
                if instance.hidden.get() {
                    props.hidden = Some(true);
                }
                if instance
                    .src
                    .borrow()
                    .as_ref()
                    .is_some_and(|src| !src.is_empty())
                {
                    props.src = instance.src.borrow().clone();
                }
                if children.is_some() {
                    props.children = children;
                }
                Some(create_jsx_element_for_test_comparison(
                    Some(js::unknown_of(instance.r#type.clone())),
                    js::unknown_of(props),
                ))
            }
            JsxChild::Child(Child::Text(text_instance)) => {
                // This is a text instance
                if text_instance.hidden.get() {
                    return Some(js::unknown_of(String::new()));
                }
                Some(js::unknown_of(text_instance.text.borrow().clone()))
            }
        }
    }

    fn get_children(root: Option<&Container>) -> Option<Rc<RefCell<Vec<Child>>>> {
        if let Some(root) = root {
            Some(Rc::clone(&root.children.borrow()))
        } else {
            js::null()
        }
    }

    fn get_pending_children(root: Option<&Container>) -> Option<Rc<RefCell<Vec<Child>>>> {
        if let Some(root) = root {
            Some(Rc::clone(&root.children.borrow()))
        } else {
            js::null()
        }
    }

    #[cfg_attr(rust_js, rust_js::name = "getChildrenAsJSX")]
    fn get_children_as_jsx(root: Option<&Container>) -> Option<&'static Unknown> {
        let children = child_to_jsx(get_children(root).map(JsxChild::Children), js::null());
        let Some(children) = children else {
            return js::null();
        };
        if is_array(Some(children)) {
            return Some(create_jsx_element_for_test_comparison(
                Some(js::unknown(&REACT_FRAGMENT_TYPE)),
                js::unknown_of(FragmentProps { children }),
            ));
        }
        Some(children)
    }

    #[cfg_attr(rust_js, rust_js::name = "getPendingChildrenAsJSX")]
    fn get_pending_children_as_jsx(root: Option<&Container>) -> Option<&'static Unknown> {
        let children = child_to_jsx(get_children(root).map(JsxChild::Children), js::null());
        let Some(children) = children else {
            return js::null();
        };
        if is_array(Some(children)) {
            return Some(create_jsx_element_for_test_comparison(
                Some(js::unknown(&REACT_FRAGMENT_TYPE)),
                js::unknown_of(FragmentProps { children }),
            ));
        }
        Some(children)
    }

    let flush_sync = {
        let current_event_priority = Rc::clone(&current_event_priority);
        move |r#fn: Option<&dyn Fn() -> Option<&'static Unknown>>| -> Option<&'static Unknown> {
            if __DEV__ && noop_renderer.is_already_rendering() {
                console::error((concat!(
                    "flushSync was called from inside a lifecycle method. React cannot ",
                    "flush when React is already rendering. Consider moving this call to ",
                    "a scheduler task or micro task."
                ),));
            }
            if DISABLE_LEGACY_MODE {
                let previous_transition = t(REACT_SHARED_INTERNALS);
                let preivous_event_priority = current_event_priority.get();
                let _restore = RestoreFlushSync {
                    previous_transition,
                    current_event_priority: &current_event_priority,
                    preivous_event_priority,
                    noop_renderer,
                };
                set_t(REACT_SHARED_INTERNALS, None);
                current_event_priority.set(DISCRETE_EVENT_PRIORITY);
                if let Some(r#fn) = r#fn { r#fn() } else { None }
            } else {
                noop_renderer.flush_sync_from_reconciler(r#fn).flatten()
            }
        }
    };

    fn on_recoverable_error(error: Option<&'static Unknown>, _error_info: &ErrorInfo) {
        // eslint-disable-next-line react-internal/warning-args, react-internal/no-production-logging -- renderer is only used for testing.
        console::error((error,));
    }
    fn on_default_transition_indicator() -> Option<&'static dyn Fn()> {
        None
    }

    let id_counter = Rc::new(Cell::new(0.0));

    let get_or_create_root_container = {
        let roots = Rc::clone(&roots);
        let root_containers = Rc::clone(&root_containers);
        move |root_id: Option<&str>, tag: RootTag| -> &'static Container {
            let root_id = root_id.unwrap_or(DEFAULT_ROOT_ID);
            let mut root = roots.get(root_id);
            if root.is_none() {
                let container = Rc::new(Container {
                    root_id: root_id.to_owned(),
                    pending_children: RefCell::new(Rc::new(RefCell::new(Vec::new()))),
                    children: RefCell::new(Rc::new(RefCell::new(Vec::new()))),
                });
                root_containers.set(root_id, Rc::clone(&container));
                let new_root = noop_renderer.create_container(
                    js::unknown_of(Rc::clone(&container)),
                    tag,
                    None,
                    false,
                    Some(false),
                    String::new(),
                    Rc::new(move |error, error_info: &ErrorInfo| {
                        noop_renderer.default_on_uncaught_error(error, error_info)
                    }),
                    Rc::new(move |error, error_info: &CaughtErrorInfo| {
                        noop_renderer.default_on_caught_error(error, error_info)
                    }),
                    Rc::new(on_recoverable_error),
                    &on_default_transition_indicator,
                    None,
                );
                roots.set(root_id, Rc::clone(&new_root));
                root = Some(new_root);
            }
            // A root fiber's `stateNode` is its FiberRoot, whose `containerInfo` is the
            // container made above.
            unsafe {
                let root_node: Rc<FiberRoot> =
                    js::cast(root.unwrap().current.borrow().state_node.get());
                js::cast(Some(root_node.container_info))
            }
        }
    };

    #[cfg_attr(rust_js, rust_js::name = "renderToRootWithID")]
    let render_to_root_with_id = {
        let roots = Rc::clone(&roots);
        let get_or_create_root_container = get_or_create_root_container.clone();
        move |element: Option<&'static Unknown>,
              root_id: &str,
              callback: Option<&'static Unknown>| {
            let container = get_or_create_root_container(Some(root_id), CONCURRENT_ROOT);
            let root = roots.get(&container.root_id);
            noop_renderer.update_container(element, &root.unwrap(), None, callback);
        }
    };

    ReactNoop {
        scheduler: SCHEDULER,

        get_children: Box::new(|_root_id| {
            panic!(
                "No longer supported due to bad performance when used with `expect()`. Use `ReactNoop.getChildrenAsJSX()` instead or, if you really need to, `dangerouslyGetChildren` after you carefully considered the warning in its JSDOC."
            );
        }),

        get_pending_children: Box::new(|_root_id| {
            panic!(
                "No longer supported due to bad performance when used with `expect()`. Use `ReactNoop.getPendingChildrenAsJSX()` instead or, if you really need to, `dangerouslyGetPendingChildren` after you carefully considered the warning in its JSDOC."
            );
        }),

        dangerously_get_children: Box::new({
            let root_containers = Rc::clone(&root_containers);
            move |root_id| {
                let container = root_containers.get(root_id.unwrap_or(DEFAULT_ROOT_ID));
                get_children(container.as_deref())
            }
        }),

        dangerously_get_pending_children: Box::new({
            let root_containers = Rc::clone(&root_containers);
            move |root_id| {
                let container = root_containers.get(root_id.unwrap_or(DEFAULT_ROOT_ID));
                get_pending_children(container.as_deref())
            }
        }),

        get_or_create_root_container: Box::new(get_or_create_root_container.clone()),

        // TODO: Replace ReactNoop.render with createRoot + root.render
        create_root: Box::new({
            let id_counter = Rc::clone(&id_counter);
            move |options| {
                let id = id_counter.get();
                id_counter.set(id + 1.0);
                let container = Rc::new(Container {
                    root_id: js::concat_text(id),
                    pending_children: RefCell::new(Rc::new(RefCell::new(Vec::new()))),
                    children: RefCell::new(Rc::new(RefCell::new(Vec::new()))),
                });
                let fiber_root = noop_renderer.create_container(
                    js::unknown_of(Rc::clone(&container)),
                    CONCURRENT_ROOT,
                    None,
                    false,
                    Some(false),
                    String::new(),
                    options
                        .and_then(|options| options.on_uncaught_error)
                        .map(|on_error| Rc::new(on_error) as Rc<OnError>)
                        .unwrap_or_else(|| {
                            Rc::new(move |error, error_info: &ErrorInfo| {
                                noop_renderer.default_on_uncaught_error(error, error_info)
                            })
                        }),
                    options
                        .and_then(|options| options.on_caught_error)
                        .map(|on_error| Rc::new(on_error) as Rc<OnCaughtError>)
                        .unwrap_or_else(|| {
                            Rc::new(move |error, error_info: &CaughtErrorInfo| {
                                noop_renderer.default_on_caught_error(error, error_info)
                            })
                        }),
                    Rc::new(on_recoverable_error),
                    options
                        .and_then(|options| options.on_default_transition_indicator)
                        .unwrap_or(&on_default_transition_indicator),
                    options
                        .and_then(|options| options.unstable_transition_callbacks)
                        .or(js::null()),
                );
                Root {
                    scheduler: SCHEDULER,
                    render: Box::new(move |children| {
                        noop_renderer.update_container(children, &fiber_root, None, None);
                    }),
                    get_children: Box::new({
                        let container = Rc::clone(&container);
                        move || get_children(Some(&container))
                    }),
                    get_children_as_jsx: Box::new(move || get_children_as_jsx(Some(&container))),
                }
            }
        }),

        create_legacy_root: Box::new({
            let id_counter = Rc::clone(&id_counter);
            move || {
                if DISABLE_LEGACY_MODE {
                    panic!("createLegacyRoot: Unsupported Legacy Mode API.");
                }

                let id = id_counter.get();
                id_counter.set(id + 1.0);
                let container = Rc::new(Container {
                    root_id: js::concat_text(id),
                    pending_children: RefCell::new(Rc::new(RefCell::new(Vec::new()))),
                    children: RefCell::new(Rc::new(RefCell::new(Vec::new()))),
                });
                let fiber_root = noop_renderer.create_container(
                    js::unknown_of(Rc::clone(&container)),
                    LEGACY_ROOT,
                    None,
                    false,
                    js::null(),
                    String::new(),
                    Rc::new(move |error, error_info: &ErrorInfo| {
                        noop_renderer.default_on_uncaught_error(error, error_info)
                    }),
                    Rc::new(move |error, error_info: &CaughtErrorInfo| {
                        noop_renderer.default_on_caught_error(error, error_info)
                    }),
                    Rc::new(on_recoverable_error),
                    &on_default_transition_indicator,
                    None,
                );
                LegacyRoot {
                    scheduler: SCHEDULER,
                    render: Box::new(move |children| {
                        noop_renderer.update_container(children, &fiber_root, None, None);
                    }),
                    get_children: Box::new({
                        let container = Rc::clone(&container);
                        move || get_children(Some(&container))
                    }),
                    get_children_as_jsx: Box::new(move || get_children_as_jsx(Some(&container))),
                    legacy: true,
                }
            }
        }),

        get_children_as_jsx: Box::new({
            let root_containers = Rc::clone(&root_containers);
            move |root_id| {
                let container = root_containers.get(root_id.unwrap_or(DEFAULT_ROOT_ID));
                get_children_as_jsx(container.as_deref())
            }
        }),

        get_pending_children_as_jsx: Box::new({
            let root_containers = Rc::clone(&root_containers);
            move |root_id| {
                let container = root_containers.get(root_id.unwrap_or(DEFAULT_ROOT_ID));
                get_pending_children_as_jsx(container.as_deref())
            }
        }),

        get_suspensey_thing_status: Box::new({
            let suspensey_thing_cache = Rc::clone(&suspensey_thing_cache);
            move |src| match *suspensey_thing_cache.borrow() {
                None => js::null(),
                Some(suspensey_thing_cache) => {
                    let record = suspensey_thing_cache.get(src);
                    match record {
                        None => js::null(),
                        Some(record) => Some(record.status.get()),
                    }
                }
            }
        }),

        resolve_suspensey_thing: Box::new({
            let suspensey_thing_cache = Rc::clone(&suspensey_thing_cache);
            move |key| {
                if suspensey_thing_cache.borrow().is_none() {
                    suspensey_thing_cache.replace(Some(js::map::new(None)));
                }
                let record = suspensey_thing_cache.borrow().unwrap().get(key);
                match record {
                    None => {
                        let new_record = Rc::new(SuspenseyThingRecord {
                            status: Cell::new("fulfilled"),
                            subscriptions: RefCell::new(None),
                        });
                        suspensey_thing_cache.borrow().unwrap().set(key, new_record);
                    }
                    Some(record) => {
                        if record.status.get() == "pending" {
                            record.status.set("fulfilled");
                            let subscriptions = record.subscriptions.take();
                            if let Some(subscriptions) = subscriptions {
                                for subscription in subscriptions {
                                    subscription
                                        .pending_count
                                        .set(subscription.pending_count.get() - 1.0);
                                    if subscription.pending_count.get() == 0.0 {
                                        let commit = subscription.commit.get();
                                        subscription.commit.set(None);
                                        let Some(commit) = commit else {
                                            panic!(
                                                "Expected commit to be a function. This is a bug in React."
                                            );
                                        };
                                        commit();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }),

        reset_suspensey_thing_cache: Box::new({
            let suspensey_thing_cache = Rc::clone(&suspensey_thing_cache);
            move || {
                suspensey_thing_cache.replace(js::null());
            }
        }),

        create_portal: Box::new(move |children, container, key| {
            noop_renderer.create_portal(children, Some(js::unknown(container)), js::null(), key)
        }),

        // Shortcut for testing a single root
        render: Box::new({
            #[cfg_attr(rust_js, rust_js::name = "renderToRootWithID")]
            let render_to_root_with_id = render_to_root_with_id.clone();
            move |element, callback| {
                render_to_root_with_id(element, DEFAULT_ROOT_ID, callback);
            }
        }),

        render_legacy_sync_root: Box::new({
            let roots = Rc::clone(&roots);
            let get_or_create_root_container = get_or_create_root_container.clone();
            move |element, callback| {
                if DISABLE_LEGACY_MODE {
                    panic!("createLegacyRoot: Unsupported Legacy Mode API.");
                }
                let root_id = DEFAULT_ROOT_ID;
                let container = get_or_create_root_container(Some(root_id), LEGACY_ROOT);
                let root = roots.get(&container.root_id);
                noop_renderer.update_container(element, &root.unwrap(), None, callback);
            }
        }),

        render_to_root_with_id: Box::new(render_to_root_with_id.clone()),

        unmount_root_with_id: Box::new({
            let roots = Rc::clone(&roots);
            let root_containers = Rc::clone(&root_containers);
            move |root_id| {
                let root = roots.get(root_id);
                if let Some(root) = root {
                    let roots = Rc::clone(&roots);
                    let root_containers = Rc::clone(&root_containers);
                    let root_id = root_id.to_owned();
                    noop_renderer.update_container(
                        js::null(),
                        &root,
                        None,
                        Some(js::unknown_of(move || {
                            roots.delete(&root_id);
                            root_containers.delete(&root_id);
                        })),
                    );
                }
            }
        }),

        find_instance: Box::new(move |component_or_element| {
            let Some(component) = component_or_element else {
                return js::null();
            };
            // Unsound duck typing.
            if let Kind::Number(_) = js::classify(js::get(component, "id")) {
                return Some(component);
            }
            if __DEV__ {
                return noop_renderer.find_host_instance_with_warning(component, "findInstance");
            }
            noop_renderer.find_host_instance(component)
        }),

        flush_next_yield: Box::new(|| {
            unstable_flush_number_of_yields(1.0);
            unstable_clear_log()
        }),

        start_tracking_host_counters: Box::new({
            let host_update_counter = Rc::clone(&host_update_counter);
            let host_clone_counter = Rc::clone(&host_clone_counter);
            move || {
                host_update_counter.set(0.0);
                host_clone_counter.set(0.0);
            }
        }),

        stop_tracking_host_counters: Box::new(move || {
            let result = if use_mutation {
                HostCounters::Mutation(MutationCounters {
                    host_update_counter: host_update_counter.get(),
                })
            } else {
                HostCounters::Persistence(PersistenceCounters {
                    host_clone_counter: host_clone_counter.get(),
                })
            };
            host_update_counter.set(0.0);
            host_clone_counter.set(0.0);

            result
        }),

        expire: unstable_advance_time,

        flush_expired: Box::new(unstable_flush_expired),

        unstable_run_with_priority: Box::new(move |priority, r#fn| {
            let previous_priority = get_current_update_priority();
            let _restore = RestoreUpdatePriority {
                set_current_update_priority: &set_current_update_priority,
                previous_priority,
            };
            set_current_update_priority(priority);
            r#fn()
        }),

        batched_updates: Box::new(move |f, a| noop_renderer.batched_updates(f, a)),

        deferred_updates: Box::new(move |f| noop_renderer.deferred_updates(f)),

        discrete_updates: Box::new(move |f, a, b, c, d| {
            noop_renderer.discrete_updates(f, a, b, c, d)
        }),

        idle_updates: Box::new({
            let current_event_priority = Rc::clone(&current_event_priority);
            move |r#fn| {
                let prev_event_priority = current_event_priority.get();
                current_event_priority.set(IDLE_EVENT_PRIORITY);
                let _restore = RestorePriority {
                    priority: &current_event_priority,
                    previous: prev_event_priority,
                };
                r#fn();
            }
        }),

        flush_sync: Box::new(flush_sync),
        flush_passive_effects: Box::new(move || noop_renderer.flush_passive_effects()),

        // Logs the current state of the tree.
        dump_tree: Box::new({
            let roots = Rc::clone(&roots);
            let root_containers = Rc::clone(&root_containers);
            move |root_id| {
                let root_id = root_id.unwrap_or(DEFAULT_ROOT_ID);
                let root = roots.get(root_id);
                let root_container = root_containers.get(root_id);
                let (Some(root), Some(root_container)) = (root, root_container) else {
                    // eslint-disable-next-line react-internal/no-production-logging
                    console::log(("Nothing rendered yet.",));
                    return;
                };

                let mut buffered_log: Vec<String> = Vec::new();
                let mut log = |args: &dyn Rest<str>| {
                    buffered_log.extend(args.iter().map(|arg| arg.to_string()));
                    buffered_log.push("\n".to_owned());
                };

                fn log_host_instances(
                    log: &mut dyn FnMut(&dyn Rest<str>),
                    children: &[Child],
                    depth: usize,
                ) {
                    for child in children {
                        let indent = "  ".repeat(depth);
                        match child {
                            Child::Text(text_instance) => {
                                log(&(indent + "- " + &{ text_instance.text.borrow().clone() },));
                            }
                            Child::Instance(instance) => {
                                if let Some(text) = { instance.text.borrow().clone() } {
                                    log(&(indent + "- " + &text,));
                                } else {
                                    log(&(format!(
                                        "{indent}- {}#{}",
                                        instance.r#type, instance.id
                                    ),));

                                    log_host_instances(
                                        log,
                                        &{ instance.children.borrow().clone() },
                                        depth + 1,
                                    );
                                }
                            }
                        }
                    }
                }
                fn log_container(
                    log: &mut dyn FnMut(&dyn Rest<str>),
                    container: &Container,
                    depth: usize,
                ) {
                    log(&("  ".repeat(depth) + "- [root#" + &container.root_id + "]",));
                    log_host_instances(
                        log,
                        &{ container.children.borrow().borrow().clone() },
                        depth + 1,
                    );
                }

                fn log_update_queue(
                    log: &mut dyn FnMut(&dyn Rest<str>),
                    update_queue: &'static Unknown,
                    depth: usize,
                ) {
                    log(&("  ".repeat(depth + 1) + "QUEUED UPDATES",));
                    let first = js::get(update_queue, "firstBaseUpdate");
                    let update = first;
                    if let Some(update) = update {
                        loop {
                            log(&(
                                "  ".repeat(depth + 1) + "~",
                                "[".to_owned()
                                    + &js::string(js::get(update, "expirationTime"))
                                    + "]",
                            ));
                        }
                    }

                    let last_pending = js::get(js::get(update_queue, "shared").unwrap(), "pending");
                    if let Some(last_pending) = last_pending {
                        let first_pending = js::get(last_pending, "next");
                        let pending_update = first_pending;
                        if let Some(pending_update) = pending_update {
                            loop {
                                log(&(
                                    "  ".repeat(depth + 1) + "~",
                                    "[".to_owned()
                                        + &js::string(js::get(pending_update, "expirationTime"))
                                        + "]",
                                ));
                                if Some(pending_update) == first_pending {
                                    break;
                                }
                            }
                        }
                    }
                }

                fn log_fiber(
                    log: &mut dyn FnMut(&dyn Rest<str>),
                    fiber: &'static Unknown,
                    depth: usize,
                ) {
                    let r#type = js::get(fiber, "type");
                    log(&(
                        "  ".repeat(depth)
                            + "- "
                            // need to explicitly coerce Symbol to a string
                            + &if let Some(r#type) = r#type
                                && js::truthy(Some(r#type))
                            {
                                let name = js::get(r#type, "name");
                                if js::truthy(name) { js::string(name) } else { js::string(Some(r#type)) }
                            } else {
                                "[root]".to_owned()
                            },
                        "[".to_owned()
                            + &js::string(js::get(fiber, "childExpirationTime"))
                            + if js::truthy(js::get(fiber, "pendingProps")) {
                                "*"
                            } else {
                                ""
                            }
                            + "]",
                    ));
                    if let Some(update_queue) = js::get(fiber, "updateQueue")
                        && js::truthy(Some(update_queue))
                    {
                        log_update_queue(log, update_queue, depth);
                    }
                    // const childInProgress = fiber.progressedChild;
                    // if (childInProgress && childInProgress !== fiber.child) {
                    //   log(
                    //     '  '.repeat(depth + 1) + 'IN PROGRESS: ' + fiber.pendingWorkPriority,
                    //   );
                    //   logFiber(childInProgress, depth + 1);
                    //   if (fiber.child) {
                    //     log('  '.repeat(depth + 1) + 'CURRENT');
                    //   }
                    // } else if (fiber.child && fiber.updateQueue) {
                    //   log('  '.repeat(depth + 1) + 'CHILDREN');
                    // }
                    if let Some(child) = js::get(fiber, "child")
                        && js::truthy(Some(child))
                    {
                        log_fiber(log, child, depth + 1);
                    }
                    if let Some(sibling) = js::get(fiber, "sibling")
                        && js::truthy(Some(sibling))
                    {
                        log_fiber(log, sibling, depth);
                    }
                }

                log(&("HOST INSTANCES:",));
                log_container(&mut log, &root_container, 0);
                log(&("FIBERS:",));
                let current = root.current.borrow().clone();
                log_fiber(&mut log, js::unknown_of(current), 0);

                // eslint-disable-next-line react-internal/no-production-logging
                console::log(&buffered_log);
            }
        }),

        get_root: Box::new(move |root_id| roots.get(root_id.unwrap_or(DEFAULT_ROOT_ID))),
    }
}

js::export_default!(create_react_noop);

/// A child `childToJSX` renders: a host child, or a list of them,
/// upstream's `Instance | TextInstance | Array<Instance | TextInstance>`.
#[cfg_attr(rust_js, rust_js::untagged)]
enum JsxChild {
    Child(Child),
    Children(Rc<RefCell<Vec<Child>>>),
}

/// The props of an instance's element.
struct InstanceProps {
    prop: Option<&'static Unknown>,
    hidden: Option<bool>,
    src: Option<String>,
    children: Option<&'static Unknown>,
}

unsafe impl js::Defined for InstanceProps {}

struct FragmentProps {
    children: &'static Unknown,
}

unsafe impl js::Defined for FragmentProps {}

struct JsxElementInDev {
    r#type: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    typeof_: js::SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    key: Option<&'static Unknown>,
    props: &'static Unknown,
    #[cfg_attr(rust_js, rust_js::name = "_owner", rust_js::nullable)]
    owner: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "_store")]
    store: Option<&'static JsObject>,
}

unsafe impl js::Defined for JsxElementInDev {}

struct JsxElement {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    typeof_: js::SymbolFor<'static>,
    r#type: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    key: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    r#ref: Option<&'static Unknown>,
    props: &'static Unknown,
}

unsafe impl js::Defined for JsxElement {}

/// Puts back flushSync's Transition and event priority, and flushes, as
/// upstream's `finally` does.
struct RestoreFlushSync<'a> {
    previous_transition: Option<&'static react_isomorphic::react_start_transition::Transition>,
    current_event_priority: &'a Cell<EventPriority>,
    preivous_event_priority: EventPriority,
    noop_renderer: &'static ReactFiberReconciler,
}

impl Drop for RestoreFlushSync<'_> {
    fn drop(&mut self) {
        set_t(REACT_SHARED_INTERNALS, self.previous_transition);
        self.current_event_priority
            .set(self.preivous_event_priority);
        self.noop_renderer.flush_sync_work();
    }
}

/// Puts back `unstable_runWithPriority`'s update priority.
struct RestoreUpdatePriority<'a> {
    set_current_update_priority: &'a dyn Fn(EventPriority),
    previous_priority: EventPriority,
}

impl Drop for RestoreUpdatePriority<'_> {
    fn drop(&mut self) {
        (self.set_current_update_priority)(self.previous_priority);
    }
}
