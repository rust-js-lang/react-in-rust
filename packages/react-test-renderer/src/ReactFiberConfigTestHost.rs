// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use js::{JsObject, TimeoutId, Unknown, WeakMap, object};
use react_isomorphic::react_transition_type::TransitionTypes;
use shared::environment::__DEV__;
use shared::react_types::ReactContext;
use webapi::console;

use shared::is_array::is_array;
use shared::react_symbols::REACT_CONTEXT_TYPE;

use react_reconciler::react_event_priorities::{
    DEFAULT_EVENT_PRIORITY, EventPriority, NO_EVENT_PRIORITY,
};
use shared::react_feature_flags::ENABLE_PROFILER_TIMER;

// TODO: Consider exporting the react-native version.
#[cfg_attr(rust_js, rust_js::name = "rendererVersion")]
pub use shared::react_version::REACT_VERSION as RENDERER_VERSION;
#[cfg_attr(rust_js, rust_js::name = "rendererPackageName")]
pub const RENDERER_PACKAGE_NAME: &str = "react-test-renderer";
#[cfg_attr(rust_js, rust_js::name = "extraDevToolsConfig", rust_js::nullable)]
pub const EXTRA_DEV_TOOLS_CONFIG: Option<&Unknown> = None;

pub type Type = str;
pub type Props = Unknown;

/// What `createNodeMock` is given: an instance's type and props.
pub struct NodeMockElement {
    pub r#type: &'static str,
    pub props: &'static Props,
}

unsafe impl js::Defined for NodeMockElement {}

/// A container's `tag`, `'CONTAINER'`.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ContainerTag {
    #[cfg_attr(rust_js, rust_js::name = "CONTAINER")]
    Container,
}

/// An instance's `tag`, `'INSTANCE'`.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum InstanceTag {
    #[cfg_attr(rust_js, rust_js::name = "INSTANCE")]
    Instance,
}

/// A text instance's `tag`, `'TEXT'`.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum TextTag {
    #[cfg_attr(rust_js, rust_js::name = "TEXT")]
    Text,
}

pub struct Container {
    pub children: Rc<RefCell<Vec<Child>>>,
    pub create_node_mock: &'static dyn Fn(&NodeMockElement) -> Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::test)]
    pub tag: ContainerTag,
}

unsafe impl js::Defined for Container {}

pub struct Instance {
    pub r#type: Cell<&'static str>,
    pub props: Cell<&'static Props>,
    pub is_hidden: Cell<bool>,
    pub children: Rc<RefCell<Vec<Child>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub internal_instance_handle: Option<&'static Unknown>,
    pub root_container_instance: &'static Container,
    #[cfg_attr(rust_js, rust_js::test)]
    pub tag: InstanceTag,
}

unsafe impl js::Defined for Instance {}

pub struct TextInstance {
    pub text: Cell<&'static str>,
    pub is_hidden: Cell<bool>,
    #[cfg_attr(rust_js, rust_js::test)]
    pub tag: TextTag,
}

unsafe impl js::Defined for TextInstance {}

/// `Instance | TextInstance`, a host child.
#[derive(Clone)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Child {
    Instance(Rc<Instance>),
    Text(Rc<TextInstance>),
}

unsafe impl js::Defined for Child {}

/// `Instance | Container`, a parent of host children.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ContainerOrInstance<'a> {
    Container(&'a Container),
    Instance(&'a Instance),
}

impl ContainerOrInstance<'_> {
    fn children(&self) -> &Rc<RefCell<Vec<Child>>> {
        match self {
            ContainerOrInstance::Container(container) => &container.children,
            ContainerOrInstance::Instance(instance) => &instance.children,
        }
    }
}

pub type HydratableInstance = Child;
pub type PublicInstance = Child;
pub type HostContext = JsObject;
pub type UpdatePayload = JsObject;
pub type TimeoutHandle = TimeoutId;
pub type EventResponder = Unknown;

pub type TransitionStatus = Option<&'static Unknown>;

pub use react_reconciler::react_fiber_config_with_no_persistence::*;

pub use react_reconciler::react_fiber_config_with_no_hydration::*;
pub use react_reconciler::react_fiber_config_with_no_test_selectors::*;

pub use react_reconciler::react_fiber_config_with_no_microtasks::*;
pub use react_reconciler::react_fiber_config_with_no_resources::*;
pub use react_reconciler::react_fiber_config_with_no_singletons::*;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "NO_CONTEXT")]
    static NO_CONTEXT: &'static HostContext = object::spread(None);
    static NODE_TO_INSTANCE_MAP: &'static WeakMap<JsObject, &'static Unknown> =
        js::weak_map::new();
}

js::on_load! {
    if __DEV__ {
        object::freeze(NO_CONTEXT.with(|context| *context));
    }
}

pub fn get_public_instance(inst: &'static Child) -> Option<&'static Unknown> {
    match inst {
        Child::Instance(inst) => {
            let create_node_mock = inst.root_container_instance.create_node_mock;
            let mock_node = create_node_mock(&NodeMockElement {
                r#type: inst.r#type.get(),
                props: inst.props.get(),
            });
            if let Some(mock_node) = js::classify(mock_node).object() {
                NODE_TO_INSTANCE_MAP.with(|map| map.set(mock_node, js::unknown(&**inst)));
            }
            mock_node
        }
        Child::Text(inst) => Some(js::unknown(&**inst)),
    }
}

pub fn append_child(parent_instance: ContainerOrInstance, child: Child) {
    if __DEV__ && !is_array(Some(js::unknown(&**parent_instance.children()))) {
        console::error((concat!(
            "An invalid container has been provided. ",
            "This may indicate that another renderer is being used in addition to the test renderer. ",
            "(For example, ReactDOM.createPortal inside of a ReactTestRenderer tree.) ",
            "This is not supported."
        ),));
    }
    let mut children = parent_instance.children().borrow_mut();
    if let Some(index) = children
        .iter()
        .position(|c| js::unknown(c) == js::unknown(&child))
    {
        children.remove(index);
    }
    children.push(child);
}

pub fn insert_before(parent_instance: ContainerOrInstance, child: Child, before_child: &Child) {
    let mut children = parent_instance.children().borrow_mut();
    if let Some(index) = children
        .iter()
        .position(|c| js::unknown(c) == js::unknown(&child))
    {
        children.remove(index);
    }
    // A `beforeChild` it doesn't hold is upstream's `splice(-1, 0, child)`.
    let before_index = children
        .iter()
        .position(|c| js::unknown(c) == js::unknown(before_child))
        .unwrap_or(children.len().saturating_sub(1));
    children.insert(before_index, child);
}

pub fn remove_child(parent_instance: ContainerOrInstance, child: &Child) {
    let mut children = parent_instance.children().borrow_mut();
    match children
        .iter()
        .position(|c| js::unknown(c) == js::unknown(child))
    {
        Some(index) => {
            children.remove(index);
        }
        // A `child` it doesn't hold is upstream's `splice(-1, 1)`, which
        // removes the last.
        None => {
            children.pop();
        }
    }
}

pub fn clear_container(container: &Container) {
    container.children.borrow_mut().clear();
}

pub fn get_root_host_context(_root_container_instance: &Container) -> &'static HostContext {
    NO_CONTEXT.with(|context| *context)
}

pub fn get_child_host_context(
    _parent_host_context: &HostContext,
    _type: &str,
) -> &'static HostContext {
    NO_CONTEXT.with(|context| *context)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn prepare_for_commit(_container_info: &Container) -> Option<&'static JsObject> {
    // noop
    None
}

pub fn reset_after_commit(_container_info: &Container) {
    // noop
}

pub fn create_instance(
    r#type: &'static str,
    props: &'static Props,
    root_container_instance: &'static Container,
    _host_context: &HostContext,
    internal_instance_handle: &'static Unknown,
) -> Rc<Instance> {
    Rc::new(Instance {
        r#type: Cell::new(r#type),
        props: Cell::new(props),
        is_hidden: Cell::new(false),
        children: Rc::default(),
        internal_instance_handle: Some(internal_instance_handle),
        root_container_instance,
        tag: InstanceTag::Instance,
    })
}

pub fn clone_mutable_instance(instance: &Instance, keep_children: bool) -> Rc<Instance> {
    Rc::new(Instance {
        r#type: Cell::new(instance.r#type.get()),
        props: Cell::new(instance.props.get()),
        is_hidden: Cell::new(instance.is_hidden.get()),
        children: if keep_children {
            Rc::clone(&instance.children)
        } else {
            Rc::default()
        },
        internal_instance_handle: None,
        root_container_instance: instance.root_container_instance,
        tag: InstanceTag::Instance,
    })
}

pub fn append_initial_child(parent_instance: &Instance, child: Child) {
    let mut children = parent_instance.children.borrow_mut();
    if let Some(index) = children
        .iter()
        .position(|c| js::unknown(c) == js::unknown(&child))
    {
        children.remove(index);
    }
    children.push(child);
}

pub fn finalize_initial_children(
    _test_element: &Instance,
    _type: &str,
    _props: &Props,
    _root_container_instance: &Container,
    _host_context: &HostContext,
) -> bool {
    false
}

pub fn should_set_text_content(_type: &str, _props: &Props) -> bool {
    false
}

pub fn create_text_instance(
    text: &'static str,
    _root_container_instance: &Container,
    _host_context: &HostContext,
    _internal_instance_handle: &Unknown,
) -> Rc<TextInstance> {
    Rc::new(TextInstance {
        text: Cell::new(text),
        is_hidden: Cell::new(false),
        tag: TextTag::Text,
    })
}

pub fn clone_mutable_text_instance(text_instance: &TextInstance) -> Rc<TextInstance> {
    Rc::new(TextInstance {
        text: Cell::new(text_instance.text.get()),
        is_hidden: Cell::new(text_instance.is_hidden.get()),
        tag: TextTag::Text,
    })
}

thread_local! {
    static CURRENT_UPDATE_PRIORITY: Cell<EventPriority> = const { Cell::new(NO_EVENT_PRIORITY) };
}

pub fn set_current_update_priority(new_priority: EventPriority) {
    CURRENT_UPDATE_PRIORITY.set(new_priority);
}

pub fn get_current_update_priority() -> EventPriority {
    CURRENT_UPDATE_PRIORITY.get()
}

pub fn resolve_update_priority() -> EventPriority {
    if CURRENT_UPDATE_PRIORITY.get() != NO_EVENT_PRIORITY {
        return CURRENT_UPDATE_PRIORITY.get();
    }
    DEFAULT_EVENT_PRIORITY
}

pub fn track_scheduler_event() {}
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn resolve_event_type() -> Option<String> {
    None
}
pub fn resolve_event_time_stamp() -> f64 {
    -1.1
}
pub fn should_attempt_eager_transition() -> bool {
    false
}

#[cfg_attr(rust_js, rust_js::name = "isPrimaryRenderer")]
pub const IS_PRIMARY_RENDERER: bool = false;
#[cfg_attr(rust_js, rust_js::name = "warnsIfNotActing")]
pub const WARNS_IF_NOT_ACTING: bool = true;

type ScheduleTimeout = fn(Box<dyn FnOnce()>, Option<f64>) -> &'static TimeoutHandle;
type CancelTimeout = fn(Option<&TimeoutHandle>);

#[cfg_attr(rust_js, rust_js::name = "scheduleTimeout")]
pub const SCHEDULE_TIMEOUT: ScheduleTimeout = js::set_timeout;
#[cfg_attr(rust_js, rust_js::name = "cancelTimeout")]
pub const CANCEL_TIMEOUT: CancelTimeout = js::clear_timeout;

#[cfg_attr(rust_js, rust_js::name = "noTimeout")]
pub const NO_TIMEOUT: i32 = -1;

// -------------------
//     Mutation
// -------------------

#[cfg_attr(rust_js, rust_js::name = "supportsMutation")]
pub const SUPPORTS_MUTATION: bool = true;

pub fn commit_update(
    instance: &Instance,
    r#type: &'static str,
    _old_props: &Props,
    new_props: &'static Props,
    _internal_instance_handle: &Unknown,
) {
    instance.r#type.set(r#type);
    instance.props.set(new_props);
}

pub fn commit_mount(
    _instance: &Instance,
    _type: &str,
    _new_props: &Props,
    _internal_instance_handle: &Unknown,
) {
    // noop
}

pub fn commit_text_update(text_instance: &TextInstance, _old_text: &str, new_text: &'static str) {
    text_instance.text.set(new_text);
}

pub fn reset_text_content(_test_element: &Instance) {
    // noop
}

#[cfg_attr(rust_js, rust_js::name = "appendChildToContainer")]
pub const APPEND_CHILD_TO_CONTAINER: fn(ContainerOrInstance, Child) = append_child;
#[cfg_attr(rust_js, rust_js::name = "insertInContainerBefore")]
pub const INSERT_IN_CONTAINER_BEFORE: fn(ContainerOrInstance, Child, &Child) = insert_before;
#[cfg_attr(rust_js, rust_js::name = "removeChildFromContainer")]
pub const REMOVE_CHILD_FROM_CONTAINER: fn(ContainerOrInstance, &Child) = remove_child;

pub fn hide_instance(instance: &Instance) {
    instance.is_hidden.set(true);
}

pub fn hide_text_instance(text_instance: &TextInstance) {
    text_instance.is_hidden.set(true);
}

pub fn unhide_instance(instance: &Instance, _props: &Props) {
    instance.is_hidden.set(false);
}

pub fn unhide_text_instance(text_instance: &TextInstance, _text: &str) {
    text_instance.is_hidden.set(false);
}

pub fn apply_view_transition_name(_instance: &Instance, _name: &str, _class_name: Option<&str>) {
    // Noop
}

pub fn restore_view_transition_name(_instance: &Instance, _props: &Props) {
    // Noop
}

pub fn cancel_view_transition_name(_instance: &Instance, _name: &str, _props: &Props) {
    // Noop
}

pub fn cancel_root_view_transition_name(_root_container: &Container) {
    // Noop
}

pub fn restore_root_view_transition_name(_root_container: &Container) {
    // Noop
}

pub fn clone_root_view_transition_container(root_container: &'static Container) -> Rc<Instance> {
    Rc::new(Instance {
        r#type: Cell::new("ROOT"),
        props: Cell::new(object::spread(None)),
        is_hidden: Cell::new(false),
        children: Rc::default(),
        internal_instance_handle: None,
        root_container_instance: root_container,
        tag: InstanceTag::Instance,
    })
}

pub fn remove_root_view_transition_clone(_root_container: &Container, _clone: &Instance) {
    // Noop since it was never inserted anywhere.
}

pub type InstanceMeasurement = Option<&'static Unknown>;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn measure_instance(_instance: &Instance) -> InstanceMeasurement {
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn measure_cloned_instance(_instance: &Instance) -> InstanceMeasurement {
    None
}

pub fn was_instance_in_viewport(_measurement: InstanceMeasurement) -> bool {
    true
}

pub fn has_instance_changed(
    _old_measurement: InstanceMeasurement,
    _new_measurement: InstanceMeasurement,
) -> bool {
    false
}

pub fn has_instance_affected_parent(
    _old_measurement: InstanceMeasurement,
    _new_measurement: InstanceMeasurement,
) -> bool {
    false
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn start_view_transition(
    _suspended_state: Option<&SuspendedState>,
    _root_container: &Container,
    _transition_types: Option<Rc<RefCell<TransitionTypes>>>,
    mutation_callback: &dyn Fn(),
    layout_callback: &dyn Fn(),
    _after_mutation_callback: &dyn Fn(),
    spawned_work_callback: &dyn Fn(),
    _passive_callback: &dyn Fn() -> Option<&'static Unknown>,
    _error_callback: &dyn Fn(Option<&'static Unknown>),
    _blocked_callback: &dyn Fn(&str), // Profiling-only
    _finished_animation: &dyn Fn(),   // Profiling-only
) -> Option<&'static RunningViewTransition> {
    mutation_callback();
    layout_callback();
    // Skip afterMutationCallback(). We don't need it since we're not animating.
    spawned_work_callback();
    // Skip passiveCallback(). Spawned work will schedule a task.
    None
}

pub type RunningViewTransition = Unknown;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn start_gesture_transition(
    _suspended_state: Option<&SuspendedState>,
    _root_container: &Container,
    _timeline: Option<&GestureTimeline>,
    _range_start: f64,
    _range_end: f64,
    _transition_types: Option<Rc<RefCell<TransitionTypes>>>,
    mutation_callback: &dyn Fn(),
    animate_callback: &dyn Fn(),
    _error_callback: &dyn Fn(Option<&'static Unknown>),
    finished_animation: &dyn Fn(), // Profiling-only
) -> Option<&'static RunningViewTransition> {
    mutation_callback();
    animate_callback();
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        finished_animation();
    }
    None
}

pub fn stop_view_transition(_transition: Option<&RunningViewTransition>) {}

pub fn add_view_transition_finished_listener(
    _transition: Option<&RunningViewTransition>,
    callback: &dyn Fn(),
) {
    callback();
}

pub type ViewTransitionInstance = Unknown;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn create_view_transition_instance(_name: &str) -> Option<&'static ViewTransitionInstance> {
    None
}

pub type FragmentInstanceType = Unknown;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn create_fragment_instance(
    _fragment_fiber: &Unknown,
) -> Option<&'static FragmentInstanceType> {
    None
}

pub fn update_fragment_instance_fiber(
    _fragment_fiber: &Unknown,
    _instance: Option<&FragmentInstanceType>,
) {
    // Noop
}

pub fn commit_new_child_to_fragment_instance(
    _child: &Instance,
    _fragment_instance: Option<&FragmentInstanceType>,
) {
    // noop
}

pub fn delete_child_from_fragment_instance(
    _child: &Instance,
    _fragment_instance: Option<&FragmentInstanceType>,
) {
    // Noop
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_instance_from_node(mock_node: &JsObject) -> Option<&'static Unknown> {
    let instance = NODE_TO_INSTANCE_MAP.with(|map| map.get(mock_node));
    if let Some(instance) = instance {
        let instance: &Instance = unsafe { js::cast(Some(instance)) };
        return instance.internal_instance_handle;
    }
    None
}

pub type GestureTimeline = Unknown;

pub fn get_current_gesture_offset(_provider: Option<&GestureTimeline>) -> f64 {
    0.0
}

pub fn before_active_instance_blur(_internal_instance_handle: &Unknown) {
    // noop
}

pub fn after_active_instance_blur() {
    // noop
}

pub fn prepare_portal_mount(_portal_instance: &Container) {
    // noop
}

pub fn prepare_scope_update(scope_instance: &JsObject, inst: &'static Unknown) {
    NODE_TO_INSTANCE_MAP.with(|map| map.set(scope_instance, inst));
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_instance_from_scope(scope_instance: &JsObject) -> Option<&'static Unknown> {
    NODE_TO_INSTANCE_MAP.with(|map| map.get(scope_instance))
}

pub fn detach_deleted_instance(_node: &Instance) {
    // noop
}

pub fn log_recoverable_error(_error: Option<&Unknown>) {
    // noop
}

pub fn request_post_paint_callback(_callback: &dyn Fn(f64)) {
    // noop
}

pub fn may_suspend_commit(_type: &Type, _props: &Props) -> bool {
    false
}

pub fn may_suspend_commit_on_update(_type: &Type, _old_props: &Props, _new_props: &Props) -> bool {
    false
}

pub fn may_suspend_commit_in_sync_render(_type: &Type, _props: &Props) -> bool {
    false
}

pub fn preload_instance(_instance: &Instance, _type: &Type, _props: &Props) -> bool {
    // Return true to indicate it's already loaded
    true
}

pub type SuspendedState = Unknown;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn start_suspending_commit() -> Option<&'static SuspendedState> {
    None
}

pub fn suspend_instance(
    _state: Option<&SuspendedState>,
    _instance: &Instance,
    _type: &Type,
    _props: &Props,
) {
}

pub fn suspend_on_active_view_transition(_state: Option<&SuspendedState>, _container: &Container) {}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn wait_for_commit_to_be_ready(
    _state: Option<&SuspendedState>,
    _timeout_offset: f64,
) -> Option<&'static Unknown> {
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_suspended_commit_reason(
    _state: Option<&SuspendedState>,
    _root_container: &Container,
) -> Option<String> {
    None
}

#[cfg_attr(rust_js, rust_js::name = "NotPendingTransition", rust_js::nullable)]
pub const NOT_PENDING_TRANSITION: TransitionStatus = None;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "HostTransitionContext")]
    pub static HOST_TRANSITION_CONTEXT: ReactContext<TransitionStatus> = const {
        ReactContext {
            r#typeof: REACT_CONTEXT_TYPE,
            provider: Cell::new(Weak::new()),
            consumer: Cell::new(js::null()),
            current_value: RefCell::new(NOT_PENDING_TRANSITION),
            current_value2: RefCell::new(NOT_PENDING_TRANSITION),
            thread_count: Cell::new(0.0),
            current_renderer: Cell::new(None),
            current_renderer2: Cell::new(None),
        }
    };
}

pub type FormInstance = Instance;
pub fn reset_form_instance(_form: &Instance) {}
