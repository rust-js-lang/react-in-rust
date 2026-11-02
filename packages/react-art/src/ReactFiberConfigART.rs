// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Weak;

use art::{Node, Surface, Transform, mode, transform};
use js::{JsObject, Kind, TimeoutId, Unknown, object};
use shared::environment::__DEV__;
use shared::react_types::ReactContext;

use react_reconciler::react_event_priorities::{
    DEFAULT_EVENT_PRIORITY, EventPriority, NO_EVENT_PRIORITY,
};

use crate::react_art_internals::{EVENT_TYPES, TYPES, children_as_string};
use shared::react_symbols::REACT_CONTEXT_TYPE;

#[cfg_attr(rust_js, rust_js::name = "rendererVersion")]
pub use shared::react_version::REACT_VERSION as RENDERER_VERSION;
#[cfg_attr(rust_js, rust_js::name = "rendererPackageName")]
pub const RENDERER_PACKAGE_NAME: &str = "react-art";
#[cfg_attr(rust_js, rust_js::name = "extraDevToolsConfig", rust_js::nullable)]
pub const EXTRA_DEV_TOOLS_CONFIG: Option<&Unknown> = None;

thread_local! {
    static POOLED_TRANSFORM: &'static Transform = transform::new();
    #[cfg_attr(rust_js, rust_js::name = "NO_CONTEXT")]
    static NO_CONTEXT: &'static HostContext = object::spread(None);
}

js::on_load! {
    if __DEV__ {
        object::freeze(NO_CONTEXT.with(|context| *context));
    }
}

pub type TransitionStatus = Option<&'static Unknown>;

pub type Instance = Node;
pub type TextInstance = str;
pub type Container = Surface;
pub type Props = Unknown;
pub type HostContext = JsObject;

/// What a node's `_applyProps` is: one of the functions below that apply
/// a type's props, which createInstance sets.
///
/// The expandos react-art keeps on art's nodes are read as upstream reads
/// them, by key. Each one called is cast to the function it is:
/// `_applyProps` an `ApplyProps`, a subscription the function `subscribe`
/// gave that unsubscribes, and a fill's `applyFill` and a listener's
/// `handleEvent`, called only where they're truthy, as their methods,
/// which throws JS's `TypeError` where one isn't a function, as upstream's
/// call does.
type ApplyProps = fn(&'static Instance, &Props, Option<&Props>);

/// `Instance | TextInstance`, a host child: a node, or the text a text
/// node's children were flattened to.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Child {
    Text(&'static TextInstance),
    Instance(&'static Instance),
}

// Helper Methods

fn add_event_listeners(
    instance: &'static Instance,
    r#type: Option<&Unknown>,
    listener: Option<&Unknown>,
) {
    // We need to explicitly unregister before unmount.
    // For this reason we need to track subscriptions.
    if !js::truthy(js::get(instance, "_listeners")) {
        js::set(instance, "_listeners", object::spread(None));
        js::set(instance, "_subscriptions", object::spread(None));
    }

    js::set(js::get(instance, "_listeners"), &r#type, listener);

    if js::truthy(listener) {
        if !js::truthy(js::get(js::get(instance, "_subscriptions"), &r#type)) {
            js::set(
                js::get(instance, "_subscriptions"),
                &r#type,
                instance.subscribe(r#type, create_event_handler(instance), instance),
            );
        }
    } else if js::truthy(js::get(js::get(instance, "_subscriptions"), &r#type)) {
        unsafe { js::cast::<&dyn Fn()>(js::get(js::get(instance, "_subscriptions"), &r#type))() };
        js::delete(js::get(instance, "_subscriptions"), &r#type);
    }
}

fn create_event_handler(instance: &'static Instance) -> &'static dyn Fn(&'static Unknown) {
    js::function::with_this(move |_, (event,): (&'static Unknown,)| {
        let listener = js::get(js::get(instance, "_listeners"), &js::get(event, "type"));

        if !js::truthy(listener) {
            // Noop
        } else if let Kind::Function(listener) = js::classify(listener) {
            js::function::call(listener, Some(instance), (event,));
        } else if js::truthy(js::get(listener, "handleEvent")) {
            js::function::call(
                unsafe { js::cast::<&dyn Fn(&Unknown)>(js::get(listener, "handleEvent")) },
                Some(&listener),
                (event,),
            );
        }
    })
}

fn destroy_event_listeners(instance: &Instance) {
    if js::truthy(js::get(instance, "_subscriptions")) {
        for r#type in js::keys_in(js::get(instance, "_subscriptions")) {
            unsafe {
                js::cast::<&dyn Fn()>(js::get(js::get(instance, "_subscriptions"), &r#type))()
            };
        }
    }

    js::set(instance, "_subscriptions", js::null::<&Unknown>());
    js::set(instance, "_listeners", js::null::<&Unknown>());
}

fn get_scale_x(props: &Props) -> &'static Unknown {
    if let Some(scale_x) = js::get(props, "scaleX") {
        scale_x
    } else if let Some(scale) = js::get(props, "scale") {
        scale
    } else {
        js::unknown_of(1.0)
    }
}

fn get_scale_y(props: &Props) -> &'static Unknown {
    if let Some(scale_y) = js::get(props, "scaleY") {
        scale_y
    } else if let Some(scale) = js::get(props, "scale") {
        scale
    } else {
        js::unknown_of(1.0)
    }
}

fn is_same_font(old_font: Option<&Unknown>, new_font: Option<&Unknown>) -> bool {
    if js::strict_equals(old_font, new_font) {
        true
    } else if matches!(js::classify(new_font), Kind::String(_))
        || matches!(js::classify(old_font), Kind::String(_))
    {
        false
    } else {
        js::strict_equals(js::get(new_font, "fontSize"), js::get(old_font, "fontSize"))
            && js::strict_equals(
                js::get(new_font, "fontStyle"),
                js::get(old_font, "fontStyle"),
            )
            && js::strict_equals(
                js::get(new_font, "fontVariant"),
                js::get(old_font, "fontVariant"),
            )
            && js::strict_equals(
                js::get(new_font, "fontWeight"),
                js::get(old_font, "fontWeight"),
            )
            && js::strict_equals(
                js::get(new_font, "fontFamily"),
                js::get(old_font, "fontFamily"),
            )
    }
}

// Render Methods

fn apply_clipping_rectangle_props(
    instance: &'static Instance,
    props: &Props,
    prev_props: Option<&Props>,
) {
    let prev_props = prev_props.unwrap_or(object::spread(None));
    apply_node_props(instance, props, Some(prev_props));

    instance.set_width(js::get(props, "width"));
    instance.set_height(js::get(props, "height"));
}

fn apply_group_props(instance: &'static Instance, props: &Props, prev_props: Option<&Props>) {
    let prev_props = prev_props.unwrap_or(object::spread(None));
    apply_node_props(instance, props, Some(prev_props));

    instance.set_width(js::get(props, "width"));
    instance.set_height(js::get(props, "height"));
}

fn apply_node_props(instance: &'static Instance, props: &Props, prev_props: Option<&Props>) {
    let prev_props = prev_props.unwrap_or(object::spread(None));
    let scale_x = get_scale_x(props);
    let scale_y = get_scale_y(props);

    POOLED_TRANSFORM.with(|pooled_transform| {
        pooled_transform
            .transform_to(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            .r#move(
                js::or(js::get(props, "x"), Some(js::unknown_of(0.0))),
                js::or(js::get(props, "y"), Some(js::unknown_of(0.0))),
            )
            .rotate(
                js::or(js::get(props, "rotation"), Some(js::unknown_of(0.0))),
                js::get(props, "originX"),
                js::get(props, "originY"),
            )
            .scale(Some(scale_x), Some(scale_y));

        if js::get(props, "transform").is_some() {
            pooled_transform.transform(js::get(props, "transform"));
        }

        if instance.xx() != pooled_transform.xx()
            || instance.yx() != pooled_transform.yx()
            || instance.xy() != pooled_transform.xy()
            || instance.yy() != pooled_transform.yy()
            || instance.x() != pooled_transform.x()
            || instance.y() != pooled_transform.y()
        {
            instance.transform_to_transform(pooled_transform);
        }
    });

    if !js::strict_equals(js::get(props, "cursor"), js::get(prev_props, "cursor"))
        || !js::strict_equals(js::get(props, "title"), js::get(prev_props, "title"))
    {
        instance.indicate(js::get(props, "cursor"), js::get(props, "title"));
    }

    if js::is_function((instance, Node::blend))
        && !js::strict_equals(js::get(props, "opacity"), js::get(prev_props, "opacity"))
    {
        instance.blend(Some(
            js::get(props, "opacity").unwrap_or(js::unknown_of(1.0)),
        ));
    }

    if !js::strict_equals(js::get(props, "visible"), js::get(prev_props, "visible")) {
        if js::get(props, "visible").is_none() || js::truthy(js::get(props, "visible")) {
            instance.show();
        } else {
            instance.hide();
        }
    }

    for r#type in js::keys_in(&EVENT_TYPES) {
        add_event_listeners(
            instance,
            js::get(&EVENT_TYPES, &r#type),
            js::get(props, &r#type),
        );
    }
}

fn apply_renderable_node_props(
    instance: &'static Instance,
    props: &Props,
    prev_props: Option<&Props>,
) {
    let prev_props = prev_props.unwrap_or(object::spread(None));
    apply_node_props(instance, props, Some(prev_props));

    if !js::strict_equals(js::get(prev_props, "fill"), js::get(props, "fill")) {
        if js::truthy(js::get(props, "fill"))
            && js::truthy(js::get(js::get(props, "fill"), "applyFill"))
        {
            js::function::call(
                unsafe {
                    js::cast::<&dyn Fn(&Instance)>(js::get(js::get(props, "fill"), "applyFill"))
                },
                Some(&js::get(props, "fill")),
                (instance,),
            );
        } else {
            instance.fill(js::get(props, "fill"));
        }
    }
    if !js::strict_equals(js::get(prev_props, "stroke"), js::get(props, "stroke"))
        || !js::strict_equals(
            js::get(prev_props, "strokeWidth"),
            js::get(props, "strokeWidth"),
        )
        || !js::strict_equals(js::get(prev_props, "strokeCap"), js::get(props, "strokeCap"))
        || !js::strict_equals(js::get(prev_props, "strokeJoin"), js::get(props, "strokeJoin"))
        // TODO: Consider deep check of stokeDash; may benefit VML in IE.
        || !js::strict_equals(js::get(prev_props, "strokeDash"), js::get(props, "strokeDash"))
    {
        instance.stroke(
            js::get(props, "stroke"),
            js::get(props, "strokeWidth"),
            js::get(props, "strokeCap"),
            js::get(props, "strokeJoin"),
            js::get(props, "strokeDash"),
        );
    }
}

fn apply_shape_props(instance: &'static Instance, props: &Props, prev_props: Option<&Props>) {
    let prev_props = prev_props.unwrap_or(object::spread(None));
    apply_renderable_node_props(instance, props, Some(prev_props));

    let path = if js::truthy(js::get(props, "d")) {
        js::get(props, "d")
    } else {
        Some(js::unknown_of(children_as_string(js::get(
            props, "children",
        ))))
    };

    let prev_delta = js::get(instance, "_prevDelta");
    let prev_path = js::get(instance, "_prevPath");

    if !js::strict_equals(path, prev_path)
        || !js::strict_equals(js::get(path, "delta"), prev_delta)
        || !js::strict_equals(js::get(prev_props, "height"), js::get(props, "height"))
        || !js::strict_equals(js::get(prev_props, "width"), js::get(props, "width"))
    {
        instance.draw_shape(path, js::get(props, "width"), js::get(props, "height"));

        js::set(instance, "_prevDelta", js::get(path, "delta"));
        js::set(instance, "_prevPath", path);
    }
}

fn apply_text_props(instance: &'static Instance, props: &Props, prev_props: Option<&Props>) {
    let prev_props = prev_props.unwrap_or(object::spread(None));
    apply_renderable_node_props(instance, props, Some(prev_props));

    let string = js::get(props, "children");

    if !js::strict_equals(js::get(instance, "_currentString"), string)
        || !is_same_font(js::get(props, "font"), js::get(prev_props, "font"))
        || !js::strict_equals(
            js::get(props, "alignment"),
            js::get(prev_props, "alignment"),
        )
        || !js::strict_equals(js::get(props, "path"), js::get(prev_props, "path"))
    {
        instance.draw_text(
            string,
            js::get(props, "font"),
            js::get(props, "alignment"),
            js::get(props, "path"),
        );

        js::set(instance, "_currentString", string);
    }
}

pub use react_reconciler::react_fiber_config_with_no_hydration::*;
pub use react_reconciler::react_fiber_config_with_no_microtasks::*;
pub use react_reconciler::react_fiber_config_with_no_persistence::*;
pub use react_reconciler::react_fiber_config_with_no_resources::*;
pub use react_reconciler::react_fiber_config_with_no_scopes::*;
pub use react_reconciler::react_fiber_config_with_no_singletons::*;
pub use react_reconciler::react_fiber_config_with_no_test_selectors::*;

pub fn append_initial_child(parent_instance: &Instance, child: Child) {
    match child {
        Child::Text(_) => {
            // Noop for string children of Text (eg <Text>{'foo'}{'bar'}</Text>)
            panic!("Text children should already be flattened.");
        }
        Child::Instance(child) => {
            child.inject(parent_instance);
        }
    }
}

pub fn create_instance(
    r#type: &str,
    props: &Props,
    _internal_instance_handle: &Unknown,
) -> &'static Instance {
    let instance = if r#type == TYPES.clipping_rectangle {
        let instance = mode::clipping_rectangle();
        js::set(
            instance,
            "_applyProps",
            apply_clipping_rectangle_props as ApplyProps,
        );
        Some(instance)
    } else if r#type == TYPES.group {
        let instance = mode::group();
        js::set(instance, "_applyProps", apply_group_props as ApplyProps);
        Some(instance)
    } else if r#type == TYPES.shape {
        let instance = mode::shape();
        js::set(instance, "_applyProps", apply_shape_props as ApplyProps);
        Some(instance)
    } else if r#type == TYPES.text {
        let instance = mode::text(
            js::get(props, "children"),
            js::get(props, "font"),
            js::get(props, "alignment"),
            js::get(props, "path"),
        );
        js::set(instance, "_applyProps", apply_text_props as ApplyProps);
        Some(instance)
    } else {
        None
    };

    let Some(instance) = instance else {
        panic!("ReactART does not support the type \"{type}\"");
    };

    unsafe { js::cast::<ApplyProps>(js::get(instance, "_applyProps"))(instance, props, None) };

    instance
}

pub fn clone_mutable_instance(
    instance: &'static Instance,
    _keep_children: bool,
) -> &'static Instance {
    instance
}

pub fn create_text_instance(
    text: &'static TextInstance,
    _root_container_instance: &Container,
    _internal_instance_handle: &Unknown,
) -> &'static TextInstance {
    text
}

pub fn clone_mutable_text_instance(text_instance: &'static TextInstance) -> &'static TextInstance {
    text_instance
}

pub type FragmentInstanceType = Unknown;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn create_fragment_instance(_fiber: &Unknown) -> Option<&'static FragmentInstanceType> {
    None
}

pub fn update_fragment_instance_fiber(_fiber: &Unknown, _instance: Option<&FragmentInstanceType>) {
    // Noop
}

pub fn commit_new_child_to_fragment_instance(
    _child: &Instance,
    _fragment_instance: Option<&FragmentInstanceType>,
) {
    // Noop
}

pub fn delete_child_from_fragment_instance(
    _child: &Instance,
    _fragment_instance: Option<&FragmentInstanceType>,
) {
    // Noop
}

pub fn finalize_initial_children(_dom_element: &Instance, _type: &str, _props: &Props) -> bool {
    false
}

pub fn get_public_instance(instance: &'static Instance) -> &'static Instance {
    instance
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn prepare_for_commit() -> Option<&'static JsObject> {
    // Noop
    None
}

pub fn reset_after_commit() {
    // Noop
}

pub fn reset_text_content(_dom_element: &Instance) {
    // Noop
}

pub fn get_root_host_context() -> &'static HostContext {
    NO_CONTEXT.with(|context| *context)
}

pub fn get_child_host_context() -> &'static HostContext {
    NO_CONTEXT.with(|context| *context)
}

type ScheduleTimeout = fn(Box<dyn FnOnce()>, Option<f64>) -> &'static TimeoutId;
type CancelTimeout = fn(Option<&TimeoutId>);

#[cfg_attr(rust_js, rust_js::name = "scheduleTimeout")]
pub const SCHEDULE_TIMEOUT: ScheduleTimeout = js::set_timeout;
#[cfg_attr(rust_js, rust_js::name = "cancelTimeout")]
pub const CANCEL_TIMEOUT: CancelTimeout = js::clear_timeout;
#[cfg_attr(rust_js, rust_js::name = "noTimeout")]
pub const NO_TIMEOUT: i32 = -1;

pub fn should_set_text_content(_type: &str, props: &Props) -> bool {
    matches!(js::classify(js::get(props, "children")), Kind::String(_))
        || matches!(js::classify(js::get(props, "children")), Kind::Number(_))
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
    js::or(CURRENT_UPDATE_PRIORITY.get(), DEFAULT_EVENT_PRIORITY)
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

// The ART renderer is secondary to the React DOM renderer.
#[cfg_attr(rust_js, rust_js::name = "isPrimaryRenderer")]
pub const IS_PRIMARY_RENDERER: bool = false;

// The ART renderer shouldn't trigger missing act() warnings
#[cfg_attr(rust_js, rust_js::name = "warnsIfNotActing")]
pub const WARNS_IF_NOT_ACTING: bool = false;

#[cfg_attr(rust_js, rust_js::name = "supportsMutation")]
pub const SUPPORTS_MUTATION: bool = true;

pub fn append_child(parent_instance: &'static Instance, child: &Instance) {
    if js::strict_equals(child.parent_node(), parent_instance) {
        child.eject();
    }
    child.inject(parent_instance);
}

pub fn append_child_to_container(parent_instance: &'static Container, child: &Instance) {
    if js::strict_equals(child.parent_node(), parent_instance) {
        child.eject();
    }
    child.inject(parent_instance);
}

pub fn insert_before(_parent_instance: &Instance, child: &Instance, before_child: &Instance) {
    if child == before_child {
        panic!("ReactART: Can not insert node before itself");
    }

    child.inject_before(before_child);
}

pub fn insert_in_container_before(
    _parent_instance: &Container,
    child: &Instance,
    before_child: &Instance,
) {
    if child == before_child {
        panic!("ReactART: Can not insert node before itself");
    }

    child.inject_before(before_child);
}

pub fn remove_child(_parent_instance: &Instance, child: &Instance) {
    destroy_event_listeners(child);
    child.eject();
}

pub fn remove_child_from_container(_parent_instance: &Container, child: &Instance) {
    destroy_event_listeners(child);
    child.eject();
}

pub fn commit_text_update(_text_instance: &TextInstance, _old_text: &str, _new_text: &str) {
    // Noop
}

pub fn commit_mount(_instance: &Instance, _type: &str, _new_props: &Props) {
    // Noop
}

pub fn commit_update(
    instance: &'static Instance,
    _type: &str,
    old_props: &Props,
    new_props: &Props,
) {
    unsafe {
        js::cast::<ApplyProps>(js::get(instance, "_applyProps"))(
            instance,
            new_props,
            Some(old_props),
        )
    };
}

pub fn hide_instance(instance: &Instance) {
    instance.hide();
}

pub fn hide_text_instance(_text_instance: &TextInstance) {
    // Noop
}

pub fn unhide_instance(instance: &Instance, props: &Props) {
    if js::get(props, "visible").is_none() || js::truthy(js::get(props, "visible")) {
        instance.show();
    }
}

pub fn unhide_text_instance(_text_instance: &TextInstance, _text: &str) {
    // Noop
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

pub fn clone_root_view_transition_container(_root_container: &Container) -> &'static Instance {
    panic!("Not implemented.");
}

pub fn remove_root_view_transition_clone(_root_container: &Container, _clone: &Instance) {
    panic!("Not implemented.");
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

pub type RunningViewTransition = Unknown;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn start_view_transition() -> Option<&'static RunningViewTransition> {
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn start_gesture_transition() -> Option<&'static RunningViewTransition> {
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

pub type GestureTimeline = Unknown;

pub fn get_current_gesture_offset(_provider: Option<&GestureTimeline>) -> f64 {
    panic!("startGestureTransition is not yet supported in react-art.");
}

pub fn clear_container(_container: &Container) {
    // TODO Implement this
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_instance_from_node(_node: &Unknown) -> Option<&'static Unknown> {
    None
}

pub fn before_active_instance_blur(_internal_instance_handle: &Unknown) {
    // noop
}

pub fn after_active_instance_blur() {
    // noop
}

pub fn prepare_portal_mount(_portal_instance: &Unknown) {
    // noop
}

pub fn detach_deleted_instance(_node: &Instance) {
    // noop
}

pub fn request_post_paint_callback(_callback: &dyn Fn(f64)) {
    // noop
}

pub fn may_suspend_commit(_type: &str, _props: &Props) -> bool {
    false
}

pub fn may_suspend_commit_on_update(_type: &str, _old_props: &Props, _new_props: &Props) -> bool {
    false
}

pub fn may_suspend_commit_in_sync_render(_type: &str, _props: &Props) -> bool {
    false
}

pub fn preload_instance(_type: &str, _props: &Props) -> bool {
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
    _type: &str,
    _props: &Props,
) {
}

pub fn suspend_on_active_view_transition(_state: Option<&SuspendedState>, _container: &Container) {}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn wait_for_commit_to_be_ready(_timeout_offset: f64) -> Option<&'static Unknown> {
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

pub fn reset_form_instance() {}
