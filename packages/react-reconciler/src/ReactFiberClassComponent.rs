// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Dict, Kind, Set, SymbolFor, Unknown};
use react_isomorphic::react_context::ReactContext;
use shared::assign::assign;
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::is_array::is_array;
use shared::react_feature_flags::{DISABLE_LEGACY_CONTEXT, ENABLE_SCHEDULING_PROFILER};
use shared::react_instance_map::{get as get_instance, set as set_instance};
use shared::react_symbols::{REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE};
use shared::shallow_equal::shallow_equal;
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_fiber_call_user_space::ClassInstance;
use crate::react_fiber_class_update_queue::{
    FORCE_UPDATE, REPLACE_STATE, UpdateQueue, check_has_force_update_after_processing,
    clone_update_queue, create_update, enqueue_update, entangle_transitions,
    initialize_update_queue, process_update_queue, reset_has_force_update_before_processing,
    suspend_if_update_read_from_entangled_async_action,
};
use crate::react_fiber_dev_tools_hook::{
    mark_force_update_scheduled, mark_state_update_scheduled, set_is_strict_mode_for_devtools,
};
use crate::react_fiber_flags::{LAYOUT_STATIC, MOUNT_LAYOUT_DEV, SNAPSHOT, UPDATE};
use crate::react_fiber_lane::{Lanes, NO_LANES};
use crate::react_fiber_legacy_context::{
    EMPTY_CONTEXT_OBJECT, cache_context, get_masked_context, get_unmasked_context,
    has_context_changed,
};
use crate::react_fiber_new_context::{check_if_context_changed, read_context};
use crate::react_fiber_work_loop::{request_update_lane, schedule_update_on_fiber};
use crate::react_internal_types::Fiber;
use crate::react_profiler_timer::start_update_timer_by_lane;
use crate::react_strict_mode_warnings::REACT_STRICT_MODE_WARNINGS;
use crate::react_type_of_mode::{NO_MODE, STRICT_EFFECTS_MODE, STRICT_LEGACY_MODE};

/// A class's static `getDerivedStateFromProps`: the state its props derive,
/// given its previous state.
pub type GetDerivedStateFromProps =
    dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>) -> Option<&'static Unknown>;

thread_local! {
    static FAKE_INTERNAL_INSTANCE: &'static Dict<()> = js::dict::new();

    static DID_WARN_ABOUT_STATE_ASSIGNMENT_FOR_COMPONENT: &'static Set<str> = js::set::new(None);
    static DID_WARN_ABOUT_UNINITIALIZED_STATE: &'static Set<str> = js::set::new(None);
    static DID_WARN_ABOUT_GET_SNAPSHOT_BEFORE_UPDATE_WITHOUT_DID_UPDATE: &'static Set<Unknown> =
        js::set::new(None);
    static DID_WARN_ABOUT_LEGACY_LIFECYCLES_AND_DERIVED_STATE: &'static Set<str> =
        js::set::new(None);
    static DID_WARN_ABOUT_UNDEFINED_DERIVED_STATE: &'static Set<str> = js::set::new(None);
    static DID_WARN_ABOUT_DIRECTLY_ASSIGNING_PROPS_TO_STATE: &'static Set<str> =
        js::set::new(None);
    static DID_WARN_ABOUT_CONTEXT_TYPE_AND_CONTEXT_TYPES: &'static Set<Unknown> =
        js::set::new(None);
    static DID_WARN_ABOUT_CONTEXT_TYPES: &'static Set<Unknown> = js::set::new(None);
    static DID_WARN_ABOUT_CHILD_CONTEXT_TYPES: &'static Set<Unknown> = js::set::new(None);
    static DID_WARN_ABOUT_INVALIDATE_CONTEXT_TYPE: &'static Set<Unknown> = js::set::new(None);
    static DID_WARN_ON_INVALID_CALLBACK: &'static Set<str> = js::set::new(None);
}

js::on_load! {
    if __DEV__ {
        FAKE_INTERNAL_INSTANCE
            .with(|fake_internal_instance| js::object::freeze(*fake_internal_instance));
    }
}

fn warn_on_invalid_callback(callback: Option<&'static Unknown>) {
    if __DEV__ {
        if matches!(js::classify(callback), Kind::Null) || js::type_of(callback) == "function" {
            return;
        }
        // eslint-disable-next-line react-internal/safe-string-coercion
        let key = js::string(callback);
        if !DID_WARN_ON_INVALID_CALLBACK.with(|did_warn| did_warn.has(&key)) {
            DID_WARN_ON_INVALID_CALLBACK.with(|did_warn| did_warn.add(&key));
            console::error((
                concat!(
                    "Expected the last optional `callback` argument to be a ",
                    "function. Instead received: %s."
                ),
                callback,
            ));
        }
    }
}

fn warn_on_undefined_derived_state(r#type: &Unknown, partial_state: Option<&Unknown>) {
    if __DEV__ && matches!(js::classify(partial_state), Kind::Undefined) {
        let component_name = get_component_name_from_type(Some(r#type))
            .filter(|name| js::truthy(Some(name.as_str())))
            .unwrap_or_else(|| "Component".to_string());
        if !DID_WARN_ABOUT_UNDEFINED_DERIVED_STATE.with(|did_warn| did_warn.has(&component_name)) {
            DID_WARN_ABOUT_UNDEFINED_DERIVED_STATE.with(|did_warn| did_warn.add(&component_name));
            console::error((
                concat!(
                    "%s.getDerivedStateFromProps(): A valid state object (or null) must be returned. ",
                    "You have returned undefined."
                ),
                &component_name,
            ));
        }
    }
}

struct ExitStrictModeForDevtools;

impl Drop for ExitStrictModeForDevtools {
    fn drop(&mut self) {
        set_is_strict_mode_for_devtools(false);
    }
}

fn apply_derived_state_from_props(
    work_in_progress: &Fiber,
    ctor: &Unknown,
    get_derived_state_from_props: &GetDerivedStateFromProps,
    next_props: Option<&'static Unknown>,
) {
    let prev_state = work_in_progress.memoized_state.get();
    let mut partial_state = get_derived_state_from_props(next_props, prev_state);
    if __DEV__ {
        if (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
            set_is_strict_mode_for_devtools(true);
            let _exit = ExitStrictModeForDevtools;
            // Invoke the function an extra time to help detect side-effects.
            partial_state = get_derived_state_from_props(next_props, prev_state);
        }
        warn_on_undefined_derived_state(ctor, partial_state);
    }
    // Merge the partial state and the previous state.
    let memoized_state = match partial_state {
        None => prev_state,
        Some(partial_state) => Some(assign(
            js::unknown(js::dict::new::<()>()),
            (prev_state, partial_state),
        )),
    };
    work_in_progress.memoized_state.set(memoized_state);

    // Once the update queue is empty, persist the derived state onto the
    // base state.
    if work_in_progress.lanes.get() == NO_LANES {
        // Queue is always non-null for classes
        let update_queue: &UpdateQueue = unsafe { js::cast(work_in_progress.update_queue.get()) };
        update_queue.base_state.set(memoized_state);
    }
}

/// What a class instance's `setState`, `replaceState` and `forceUpdate`
/// schedule their updates with.
pub struct ClassComponentUpdater {
    pub enqueue_set_state: fn(&'static Unknown, Option<&'static Unknown>, Option<&'static Unknown>),
    pub enqueue_replace_state:
        fn(&'static Unknown, Option<&'static Unknown>, Option<&'static Unknown>),
    pub enqueue_force_update: fn(&'static Unknown, Option<&'static Unknown>),
}

#[cfg_attr(rust_js, rust_js::name = "classComponentUpdater")]
const CLASS_COMPONENT_UPDATER: ClassComponentUpdater = ClassComponentUpdater {
    enqueue_set_state: |inst, payload, callback| {
        let fiber: Rc<Fiber> = unsafe { js::cast(get_instance(inst)) };
        let lane = request_update_lane(&fiber);

        let update = create_update(lane);
        update.payload.set(payload);
        if let Some(callback) = callback {
            if __DEV__ {
                warn_on_invalid_callback(Some(callback));
            }
            update.callback.set(Some(callback));
        }

        let root = enqueue_update(&fiber, &update, lane);
        if let Some(root) = root {
            start_update_timer_by_lane(lane, "this.setState()", Some(&fiber));
            schedule_update_on_fiber(&root, &fiber, lane);
            entangle_transitions(&root, &fiber, lane);
        }

        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_state_update_scheduled(&fiber, lane);
        }
    },
    enqueue_replace_state: |inst, payload, callback| {
        let fiber: Rc<Fiber> = unsafe { js::cast(get_instance(inst)) };
        let lane = request_update_lane(&fiber);

        let update = create_update(lane);
        update.tag.set(REPLACE_STATE);
        update.payload.set(payload);

        if let Some(callback) = callback {
            if __DEV__ {
                warn_on_invalid_callback(Some(callback));
            }
            update.callback.set(Some(callback));
        }

        let root = enqueue_update(&fiber, &update, lane);
        if let Some(root) = root {
            start_update_timer_by_lane(lane, "this.replaceState()", Some(&fiber));
            schedule_update_on_fiber(&root, &fiber, lane);
            entangle_transitions(&root, &fiber, lane);
        }

        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_state_update_scheduled(&fiber, lane);
        }
    },
    enqueue_force_update: |inst, callback| {
        let fiber: Rc<Fiber> = unsafe { js::cast(get_instance(inst)) };
        let lane = request_update_lane(&fiber);

        let update = create_update(lane);
        update.tag.set(FORCE_UPDATE);

        if let Some(callback) = callback {
            if __DEV__ {
                warn_on_invalid_callback(Some(callback));
            }
            update.callback.set(Some(callback));
        }

        let root = enqueue_update(&fiber, &update, lane);
        if let Some(root) = root {
            start_update_timer_by_lane(lane, "this.forceUpdate()", Some(&fiber));
            schedule_update_on_fiber(&root, &fiber, lane);
            entangle_transitions(&root, &fiber, lane);
        }

        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_force_update_scheduled(&fiber, lane);
        }
    },
};

fn check_should_component_update(
    work_in_progress: &Fiber,
    ctor: &Unknown,
    old_props: &'static Unknown,
    new_props: Option<&'static Unknown>,
    old_state: Option<&'static Unknown>,
    new_state: Option<&'static Unknown>,
    next_context: Option<&'static Unknown>,
) -> bool {
    let instance: &ClassInstance = unsafe { js::cast(work_in_progress.state_node.get()) };
    if js::type_of(js::get(instance, "shouldComponentUpdate")) == "function" {
        let mut should_update =
            instance.should_component_update(new_props, new_state, next_context);
        if __DEV__ {
            if (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
                set_is_strict_mode_for_devtools(true);
                let _exit = ExitStrictModeForDevtools;
                // Invoke the function an extra time to help detect side-effects.
                should_update =
                    instance.should_component_update(new_props, new_state, next_context);
            }
            if matches!(js::classify(should_update), Kind::Undefined) {
                console::error((
                    concat!(
                        "%s.shouldComponentUpdate(): Returned undefined instead of a ",
                        "boolean value. Make sure to return true or false."
                    ),
                    get_component_name_from_type(Some(ctor))
                        .filter(|name| js::truthy(Some(name.as_str())))
                        .unwrap_or_else(|| "Component".to_string()),
                ));
            }
        }

        return js::truthy(should_update);
    }

    if js::truthy(js::get(ctor, "prototype"))
        && js::truthy(js::get(js::get(ctor, "prototype"), "isPureReactComponent"))
    {
        return !shallow_equal(Some(old_props), new_props) || !shallow_equal(old_state, new_state);
    }

    true
}

fn check_class_instance(
    work_in_progress: &Fiber,
    ctor: &'static Unknown,
    new_props: Option<&Unknown>,
) {
    let instance = work_in_progress.state_node.get();
    if __DEV__ {
        let name = get_component_name_from_type(Some(ctor))
            .filter(|name| js::truthy(Some(name.as_str())))
            .unwrap_or_else(|| "Component".to_string());
        let render_present = js::get(instance, "render");

        if !js::truthy(render_present) {
            if js::truthy(js::get(ctor, "prototype"))
                && js::type_of(js::get(js::get(ctor, "prototype"), "render")) == "function"
            {
                console::error((
                    concat!(
                        "No `render` method found on the %s ",
                        "instance: did you accidentally return an object from the constructor?"
                    ),
                    &name,
                ));
            } else {
                console::error((
                    concat!(
                        "No `render` method found on the %s ",
                        "instance: you may have forgotten to define `render`."
                    ),
                    &name,
                ));
            }
        }

        if js::truthy(js::get(instance, "getInitialState"))
            && !js::truthy(js::get(
                js::get(instance, "getInitialState"),
                "isReactClassApproved",
            ))
            && !js::truthy(js::get(instance, "state"))
        {
            console::error((
                concat!(
                    "getInitialState was defined on %s, a plain JavaScript class. ",
                    "This is only supported for classes created using React.createClass. ",
                    "Did you mean to define a state property instead?"
                ),
                &name,
            ));
        }
        if js::truthy(js::get(instance, "getDefaultProps"))
            && !js::truthy(js::get(
                js::get(instance, "getDefaultProps"),
                "isReactClassApproved",
            ))
        {
            console::error((
                concat!(
                    "getDefaultProps was defined on %s, a plain JavaScript class. ",
                    "This is only supported for classes created using React.createClass. ",
                    "Use a static property to define defaultProps instead."
                ),
                &name,
            ));
        }
        if js::truthy(js::get(instance, "contextType")) {
            console::error((
                concat!(
                    "contextType was defined as an instance property on %s. Use a static ",
                    "property to define contextType instead."
                ),
                &name,
            ));
        }

        if DISABLE_LEGACY_CONTEXT {
            if js::truthy(js::get(ctor, "childContextTypes"))
                && !DID_WARN_ABOUT_CHILD_CONTEXT_TYPES.with(|did_warn| did_warn.has(ctor))
            {
                DID_WARN_ABOUT_CHILD_CONTEXT_TYPES.with(|did_warn| did_warn.add(ctor));
                console::error((
                    concat!(
                        "%s uses the legacy childContextTypes API which was removed in React 19. ",
                        "Use React.createContext() instead. (https://react.dev/link/legacy-context)"
                    ),
                    &name,
                ));
            }
            if js::truthy(js::get(ctor, "contextTypes"))
                && !DID_WARN_ABOUT_CONTEXT_TYPES.with(|did_warn| did_warn.has(ctor))
            {
                DID_WARN_ABOUT_CONTEXT_TYPES.with(|did_warn| did_warn.add(ctor));
                console::error((
                    concat!(
                        "%s uses the legacy contextTypes API which was removed in React 19. ",
                        "Use React.createContext() with static contextType instead. ",
                        "(https://react.dev/link/legacy-context)"
                    ),
                    &name,
                ));
            }
        } else {
            if js::truthy(js::get(instance, "contextTypes")) {
                console::error((
                    concat!(
                        "contextTypes was defined as an instance property on %s. Use a static ",
                        "property to define contextTypes instead."
                    ),
                    &name,
                ));
            }

            if js::truthy(js::get(ctor, "contextType"))
                && js::truthy(js::get(ctor, "contextTypes"))
                && !DID_WARN_ABOUT_CONTEXT_TYPE_AND_CONTEXT_TYPES
                    .with(|did_warn| did_warn.has(ctor))
            {
                DID_WARN_ABOUT_CONTEXT_TYPE_AND_CONTEXT_TYPES.with(|did_warn| did_warn.add(ctor));
                console::error((
                    concat!(
                        "%s declares both contextTypes and contextType static properties. ",
                        "The legacy contextTypes property will be ignored."
                    ),
                    &name,
                ));
            }
            if js::truthy(js::get(ctor, "childContextTypes"))
                && !DID_WARN_ABOUT_CHILD_CONTEXT_TYPES.with(|did_warn| did_warn.has(ctor))
            {
                DID_WARN_ABOUT_CHILD_CONTEXT_TYPES.with(|did_warn| did_warn.add(ctor));
                console::error((
                    concat!(
                        "%s uses the legacy childContextTypes API which will soon be removed. ",
                        "Use React.createContext() instead. (https://react.dev/link/legacy-context)"
                    ),
                    &name,
                ));
            }
            if js::truthy(js::get(ctor, "contextTypes"))
                && !DID_WARN_ABOUT_CONTEXT_TYPES.with(|did_warn| did_warn.has(ctor))
            {
                DID_WARN_ABOUT_CONTEXT_TYPES.with(|did_warn| did_warn.add(ctor));
                console::error((
                    concat!(
                        "%s uses the legacy contextTypes API which will soon be removed. ",
                        "Use React.createContext() with static contextType instead. ",
                        "(https://react.dev/link/legacy-context)"
                    ),
                    &name,
                ));
            }
        }

        if js::type_of(js::get(instance, "componentShouldUpdate")) == "function" {
            console::error((
                concat!(
                    "%s has a method called ",
                    "componentShouldUpdate(). Did you mean shouldComponentUpdate()? ",
                    "The name is phrased as a question because the function is ",
                    "expected to return a value."
                ),
                &name,
            ));
        }
        if js::truthy(js::get(ctor, "prototype"))
            && js::truthy(js::get(js::get(ctor, "prototype"), "isPureReactComponent"))
            && js::type_of(js::get(instance, "shouldComponentUpdate")) != "undefined"
        {
            console::error((
                concat!(
                    "%s has a method called shouldComponentUpdate(). ",
                    "shouldComponentUpdate should not be used when extending React.PureComponent. ",
                    "Please extend React.Component if shouldComponentUpdate is used."
                ),
                get_component_name_from_type(Some(ctor))
                    .filter(|name| js::truthy(Some(name.as_str())))
                    .unwrap_or_else(|| "A pure component".to_string()),
            ));
        }
        if js::type_of(js::get(instance, "componentDidUnmount")) == "function" {
            console::error((
                concat!(
                    "%s has a method called ",
                    "componentDidUnmount(). But there is no such lifecycle method. ",
                    "Did you mean componentWillUnmount()?"
                ),
                &name,
            ));
        }
        if js::type_of(js::get(instance, "componentDidReceiveProps")) == "function" {
            console::error((
                concat!(
                    "%s has a method called ",
                    "componentDidReceiveProps(). But there is no such lifecycle method. ",
                    "If you meant to update the state in response to changing props, ",
                    "use componentWillReceiveProps(). If you meant to fetch data or ",
                    "run side-effects or mutations after React has updated the UI, use componentDidUpdate()."
                ),
                &name,
            ));
        }
        if js::type_of(js::get(instance, "componentWillRecieveProps")) == "function" {
            console::error((
                concat!(
                    "%s has a method called ",
                    "componentWillRecieveProps(). Did you mean componentWillReceiveProps()?"
                ),
                &name,
            ));
        }
        if js::type_of(js::get(instance, "UNSAFE_componentWillRecieveProps")) == "function" {
            console::error((
                concat!(
                    "%s has a method called ",
                    "UNSAFE_componentWillRecieveProps(). Did you mean UNSAFE_componentWillReceiveProps()?"
                ),
                &name,
            ));
        }
        let has_mutated_props = !js::strict_equals(js::get(instance, "props"), new_props);
        if !matches!(js::classify(js::get(instance, "props")), Kind::Undefined) && has_mutated_props
        {
            console::error((
                concat!(
                    "When calling super() in `%s`, make sure to pass ",
                    "up the same props that your component's constructor was passed."
                ),
                &name,
            ));
        }
        if js::truthy(js::get(instance, "defaultProps")) {
            console::error((
                concat!(
                    "Setting defaultProps as an instance property on %s is not supported and will be ignored.",
                    " Instead, define defaultProps as a static property on %s."
                ),
                &name,
                &name,
            ));
        }

        if js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) == "function"
            && js::type_of(js::get(instance, "componentDidUpdate")) != "function"
            && !DID_WARN_ABOUT_GET_SNAPSHOT_BEFORE_UPDATE_WITHOUT_DID_UPDATE
                .with(|did_warn| did_warn.has(ctor))
        {
            DID_WARN_ABOUT_GET_SNAPSHOT_BEFORE_UPDATE_WITHOUT_DID_UPDATE
                .with(|did_warn| did_warn.add(ctor));
            console::error((
                concat!(
                    "%s: getSnapshotBeforeUpdate() should be used with componentDidUpdate(). ",
                    "This component defines getSnapshotBeforeUpdate() only."
                ),
                get_component_name_from_type(Some(ctor)),
            ));
        }

        if js::type_of(js::get(instance, "getDerivedStateFromProps")) == "function" {
            console::error((
                concat!(
                    "%s: getDerivedStateFromProps() is defined as an instance method ",
                    "and will be ignored. Instead, declare it as a static method."
                ),
                &name,
            ));
        }
        if js::type_of(js::get(instance, "getDerivedStateFromError")) == "function" {
            console::error((
                concat!(
                    "%s: getDerivedStateFromError() is defined as an instance method ",
                    "and will be ignored. Instead, declare it as a static method."
                ),
                &name,
            ));
        }
        if js::type_of(js::get(ctor, "getSnapshotBeforeUpdate")) == "function" {
            console::error((
                concat!(
                    "%s: getSnapshotBeforeUpdate() is defined as a static method ",
                    "and will be ignored. Instead, declare it as an instance method."
                ),
                &name,
            ));
        }
        let state = js::get(instance, "state");
        if js::truthy(state) && (js::type_of(state) != "object" || is_array(state)) {
            console::error(("%s.state: must be set to an object or null", &name));
        }
        if js::type_of(js::get(instance, "getChildContext")) == "function"
            && js::type_of(js::get(ctor, "childContextTypes")) != "object"
        {
            console::error((
                concat!(
                    "%s.getChildContext(): childContextTypes must be defined in order to ",
                    "use getChildContext()."
                ),
                &name,
            ));
        }
    }
}

pub fn construct_class_instance(
    work_in_progress: &Rc<Fiber>,
    ctor: &'static Unknown,
    props: Option<&'static Unknown>,
) -> &'static ClassInstance {
    let mut is_legacy_context_consumer = false;
    let mut unmasked_context =
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object);
    let mut context =
        Some(EMPTY_CONTEXT_OBJECT.with(|empty_context_object| js::unknown(*empty_context_object)));
    let context_type = js::get(ctor, "contextType");

    if __DEV__ && js::has(ctor, "contextType") {
        let is_valid =
            // Allow null for conditional declaration
            matches!(js::classify(context_type), Kind::Null)
                || (!matches!(js::classify(context_type), Kind::Undefined)
                    && SymbolFor::from_value(js::get(context_type, "$$typeof"))
                        == Some(REACT_CONTEXT_TYPE));

        if !is_valid && !DID_WARN_ABOUT_INVALIDATE_CONTEXT_TYPE.with(|did_warn| did_warn.has(ctor))
        {
            DID_WARN_ABOUT_INVALIDATE_CONTEXT_TYPE.with(|did_warn| did_warn.add(ctor));

            let addendum = if matches!(js::classify(context_type), Kind::Undefined) {
                concat!(
                    " However, it is set to undefined. ",
                    "This can be caused by a typo or by mixing up named and default imports. ",
                    "This can also happen due to a circular dependency, so ",
                    "try moving the createContext() call to a separate file."
                )
                .to_string()
            } else if js::type_of(context_type) != "object" {
                " However, it is set to a ".to_string() + &js::type_of(context_type) + "."
            } else if SymbolFor::from_value(js::get(context_type, "$$typeof"))
                == Some(REACT_CONSUMER_TYPE)
            {
                " Did you accidentally pass the Context.Consumer instead?".to_string()
            } else {
                " However, it is set to an object with keys {".to_string()
                    + &js::object::keys(unsafe { js::cast(context_type) }).join(", ")
                    + "}."
            };
            console::error((
                concat!(
                    "%s defines an invalid contextType. ",
                    "contextType should point to the Context object returned by React.createContext().%s"
                ),
                get_component_name_from_type(Some(ctor))
                    .filter(|name| js::truthy(Some(name.as_str())))
                    .unwrap_or_else(|| "Component".to_string()),
                &addendum,
            ));
        }
    }

    if js::type_of(context_type) == "object" && context_type.is_some() {
        let context_type: Rc<ReactContext<Option<&'static Unknown>>> =
            unsafe { js::cast(context_type) };
        context = read_context(&context_type);
    } else if !DISABLE_LEGACY_CONTEXT {
        unmasked_context = get_unmasked_context(work_in_progress, ctor, true);
        let context_types = js::get(ctor, "contextTypes");
        is_legacy_context_consumer = context_types.is_some();
        context = Some(js::unknown(if is_legacy_context_consumer {
            get_masked_context(work_in_progress, unmasked_context)
        } else {
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
        }));
    }

    let mut instance: &ClassInstance = unsafe { js::cast(Some(js::new(ctor, (props, context)))) };
    // Instantiate twice to help detect side-effects.
    if __DEV__ && (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
        set_is_strict_mode_for_devtools(true);
        let _exit = ExitStrictModeForDevtools;
        instance = unsafe { js::cast(Some(js::new(ctor, (props, context)))) };
    }

    let state = js::get(instance, "state").or(js::null());
    work_in_progress.memoized_state.set(state);
    js::set(instance, "updater", &CLASS_COMPONENT_UPDATER);
    work_in_progress.state_node.set(Some(js::unknown(instance)));
    // The instance needs access to the fiber so that it can schedule updates
    set_instance(js::unknown(instance), js::unknown(&**work_in_progress));
    if __DEV__ {
        FAKE_INTERNAL_INSTANCE.with(|fake_internal_instance| {
            js::set(instance, "_reactInternalInstance", *fake_internal_instance);
        });
    }

    if __DEV__ {
        if js::type_of(js::get(ctor, "getDerivedStateFromProps")) == "function" && state.is_none() {
            let component_name = get_component_name_from_type(Some(ctor))
                .filter(|name| js::truthy(Some(name.as_str())))
                .unwrap_or_else(|| "Component".to_string());
            if !DID_WARN_ABOUT_UNINITIALIZED_STATE.with(|did_warn| did_warn.has(&component_name)) {
                DID_WARN_ABOUT_UNINITIALIZED_STATE.with(|did_warn| did_warn.add(&component_name));
                console::error((
                    concat!(
                        "`%s` uses `getDerivedStateFromProps` but its initial state is ",
                        "%s. This is not recommended. Instead, define the initial state by ",
                        "assigning an object to `this.state` in the constructor of `%s`. ",
                        "This ensures that `getDerivedStateFromProps` arguments have a consistent shape."
                    ),
                    &component_name,
                    if matches!(js::classify(js::get(instance, "state")), Kind::Null) {
                        "null"
                    } else {
                        "undefined"
                    },
                    &component_name,
                ));
            }
        }

        // If new component APIs are defined, "unsafe" lifecycles won't be called.
        // Warn about these lifecycles if they are present.
        // Don't warn about react-lifecycles-compat polyfilled methods though.
        if js::type_of(js::get(ctor, "getDerivedStateFromProps")) == "function"
            || js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) == "function"
        {
            let mut found_will_mount_name = None;
            let mut found_will_receive_props_name = None;
            let mut found_will_update_name = None;
            if js::type_of(js::get(instance, "componentWillMount")) == "function"
                && !js::strict_equals(
                    js::get(
                        js::get(instance, "componentWillMount"),
                        "__suppressDeprecationWarning",
                    ),
                    true,
                )
            {
                found_will_mount_name = Some("componentWillMount");
            } else if js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function" {
                found_will_mount_name = Some("UNSAFE_componentWillMount");
            }
            if js::type_of(js::get(instance, "componentWillReceiveProps")) == "function"
                && !js::strict_equals(
                    js::get(
                        js::get(instance, "componentWillReceiveProps"),
                        "__suppressDeprecationWarning",
                    ),
                    true,
                )
            {
                found_will_receive_props_name = Some("componentWillReceiveProps");
            } else if js::type_of(js::get(instance, "UNSAFE_componentWillReceiveProps"))
                == "function"
            {
                found_will_receive_props_name = Some("UNSAFE_componentWillReceiveProps");
            }
            if js::type_of(js::get(instance, "componentWillUpdate")) == "function"
                && !js::strict_equals(
                    js::get(
                        js::get(instance, "componentWillUpdate"),
                        "__suppressDeprecationWarning",
                    ),
                    true,
                )
            {
                found_will_update_name = Some("componentWillUpdate");
            } else if js::type_of(js::get(instance, "UNSAFE_componentWillUpdate")) == "function" {
                found_will_update_name = Some("UNSAFE_componentWillUpdate");
            }
            if found_will_mount_name.is_some()
                || found_will_receive_props_name.is_some()
                || found_will_update_name.is_some()
            {
                let component_name = get_component_name_from_type(Some(ctor))
                    .filter(|name| js::truthy(Some(name.as_str())))
                    .unwrap_or_else(|| "Component".to_string());
                let new_api_name =
                    if js::type_of(js::get(ctor, "getDerivedStateFromProps")) == "function" {
                        "getDerivedStateFromProps()"
                    } else {
                        "getSnapshotBeforeUpdate()"
                    };
                if !DID_WARN_ABOUT_LEGACY_LIFECYCLES_AND_DERIVED_STATE
                    .with(|did_warn| did_warn.has(&component_name))
                {
                    DID_WARN_ABOUT_LEGACY_LIFECYCLES_AND_DERIVED_STATE
                        .with(|did_warn| did_warn.add(&component_name));
                    console::error((
                        concat!(
                            "Unsafe legacy lifecycles will not be called for components using new component APIs.\n\n",
                            "%s uses %s but also contains the following legacy lifecycles:%s%s%s\n\n",
                            "The above lifecycles should be removed. Learn more about this warning here:\n",
                            "https://react.dev/link/unsafe-component-lifecycles"
                        ),
                        &component_name,
                        new_api_name,
                        found_will_mount_name
                            .map(|name| format!("\n  {name}"))
                            .unwrap_or_default(),
                        found_will_receive_props_name
                            .map(|name| format!("\n  {name}"))
                            .unwrap_or_default(),
                        found_will_update_name
                            .map(|name| format!("\n  {name}"))
                            .unwrap_or_default(),
                    ));
                }
            }
        }
    }

    // Cache unmasked context so we can avoid recreating masked context unless necessary.
    // ReactFiberLegacyContext usually updates this cache but can't for newly-created instances.
    if is_legacy_context_consumer {
        cache_context(work_in_progress, unmasked_context, unsafe {
            js::cast(context)
        });
    }

    instance
}

fn call_component_will_mount(work_in_progress: &Fiber, instance: &'static ClassInstance) {
    let old_state = js::get(instance, "state");

    if js::type_of(js::get(instance, "componentWillMount")) == "function" {
        instance.component_will_mount();
    }
    if js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function" {
        instance.unsafe_component_will_mount();
    }

    if !js::strict_equals(old_state, js::get(instance, "state")) {
        if __DEV__ {
            console::error((
                concat!(
                    "%s.componentWillMount(): Assigning directly to this.state is ",
                    "deprecated (except inside a component's ",
                    "constructor). Use setState instead."
                ),
                get_component_name_from_fiber(work_in_progress)
                    .filter(|name| js::truthy(Some(name.as_str())))
                    .unwrap_or_else(|| "Component".to_string()),
            ));
        }
        (CLASS_COMPONENT_UPDATER.enqueue_replace_state)(
            js::unknown(instance),
            js::get(instance, "state"),
            js::null(),
        );
    }
}

fn call_component_will_receive_props(
    work_in_progress: &Fiber,
    instance: &'static ClassInstance,
    new_props: Option<&'static Unknown>,
    next_context: Option<&'static Unknown>,
) {
    let old_state = js::get(instance, "state");
    if js::type_of(js::get(instance, "componentWillReceiveProps")) == "function" {
        instance.component_will_receive_props(new_props, next_context);
    }
    if js::type_of(js::get(instance, "UNSAFE_componentWillReceiveProps")) == "function" {
        instance.unsafe_component_will_receive_props(new_props, next_context);
    }

    if !js::strict_equals(js::get(instance, "state"), old_state) {
        if __DEV__ {
            let component_name = get_component_name_from_fiber(work_in_progress)
                .filter(|name| js::truthy(Some(name.as_str())))
                .unwrap_or_else(|| "Component".to_string());
            if !DID_WARN_ABOUT_STATE_ASSIGNMENT_FOR_COMPONENT
                .with(|did_warn| did_warn.has(&component_name))
            {
                DID_WARN_ABOUT_STATE_ASSIGNMENT_FOR_COMPONENT
                    .with(|did_warn| did_warn.add(&component_name));
                console::error((
                    concat!(
                        "%s.componentWillReceiveProps(): Assigning directly to ",
                        "this.state is deprecated (except inside a component's ",
                        "constructor). Use setState instead."
                    ),
                    &component_name,
                ));
            }
        }
        (CLASS_COMPONENT_UPDATER.enqueue_replace_state)(
            js::unknown(instance),
            js::get(instance, "state"),
            js::null(),
        );
    }
}

// Invokes the mount life-cycles on a previously never rendered instance.
pub fn mount_class_instance(
    work_in_progress: &Rc<Fiber>,
    ctor: &'static Unknown,
    new_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) {
    if __DEV__ {
        check_class_instance(work_in_progress, ctor, new_props);
    }

    let instance: &ClassInstance = unsafe { js::cast(work_in_progress.state_node.get()) };
    js::set(instance, "props", new_props);
    js::set(instance, "state", work_in_progress.memoized_state.get());
    js::set(instance, "refs", js::dict::new::<()>());

    initialize_update_queue(work_in_progress);

    let context_type = js::get(ctor, "contextType");
    if js::type_of(context_type) == "object" && context_type.is_some() {
        let context_type: Rc<ReactContext<Option<&'static Unknown>>> =
            unsafe { js::cast(context_type) };
        js::set(instance, "context", read_context(&context_type));
    } else if DISABLE_LEGACY_CONTEXT {
        js::set(
            instance,
            "context",
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object),
        );
    } else {
        let unmasked_context = get_unmasked_context(work_in_progress, ctor, true);
        js::set(
            instance,
            "context",
            get_masked_context(work_in_progress, unmasked_context),
        );
    }

    if __DEV__ {
        if js::strict_equals(js::get(instance, "state"), new_props) {
            let component_name = get_component_name_from_type(Some(ctor))
                .filter(|name| js::truthy(Some(name.as_str())))
                .unwrap_or_else(|| "Component".to_string());
            if !DID_WARN_ABOUT_DIRECTLY_ASSIGNING_PROPS_TO_STATE
                .with(|did_warn| did_warn.has(&component_name))
            {
                DID_WARN_ABOUT_DIRECTLY_ASSIGNING_PROPS_TO_STATE
                    .with(|did_warn| did_warn.add(&component_name));
                console::error((
                    concat!(
                        "%s: It is not recommended to assign props directly to state ",
                        "because updates to props won't be reflected in state. ",
                        "In most cases, it is better to use props directly."
                    ),
                    &component_name,
                ));
            }
        }

        if (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
            REACT_STRICT_MODE_WARNINGS.with(|react_strict_mode_warnings| {
                (react_strict_mode_warnings
                    .record_legacy_context_warning
                    .get())(work_in_progress, Some(js::unknown(instance)));
            });
        }

        REACT_STRICT_MODE_WARNINGS.with(|react_strict_mode_warnings| {
            (react_strict_mode_warnings
                .record_unsafe_lifecycle_warnings
                .get())(work_in_progress, Some(js::unknown(instance)));
        });
    }

    js::set(instance, "state", work_in_progress.memoized_state.get());

    let get_derived_state_from_props = js::get(ctor, "getDerivedStateFromProps");
    if js::type_of(get_derived_state_from_props) == "function" {
        apply_derived_state_from_props(
            work_in_progress,
            ctor,
            unsafe { js::cast(get_derived_state_from_props) },
            new_props,
        );
        js::set(instance, "state", work_in_progress.memoized_state.get());
    }

    // In order to support react-lifecycles-compat polyfilled components,
    // Unsafe lifecycles should not be invoked for components using the new APIs.
    if js::type_of(js::get(ctor, "getDerivedStateFromProps")) != "function"
        && js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) != "function"
        && (js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function"
            || js::type_of(js::get(instance, "componentWillMount")) == "function")
    {
        call_component_will_mount(work_in_progress, instance);
        // If we had additional state updates during this life-cycle, let's
        // process them now.
        process_update_queue(
            work_in_progress,
            new_props,
            Some(js::unknown(instance)),
            render_lanes,
        );
        suspend_if_update_read_from_entangled_async_action();
        js::set(instance, "state", work_in_progress.memoized_state.get());
    }

    if js::type_of(js::get(instance, "componentDidMount")) == "function" {
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | UPDATE | LAYOUT_STATIC);
    }
    if __DEV__ && (work_in_progress.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | MOUNT_LAYOUT_DEV);
    }
}

pub fn resume_mount_class_instance(
    work_in_progress: &Rc<Fiber>,
    ctor: &'static Unknown,
    new_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> bool {
    let instance: &ClassInstance = unsafe { js::cast(work_in_progress.state_node.get()) };

    let unresolved_old_props = work_in_progress.memoized_props.get();
    let old_props =
        resolve_class_component_props(Some(ctor), unsafe { js::cast(unresolved_old_props) });
    js::set(instance, "props", old_props);

    let old_context = js::get(instance, "context");
    let context_type = js::get(ctor, "contextType");
    let mut next_context =
        Some(EMPTY_CONTEXT_OBJECT.with(|empty_context_object| js::unknown(*empty_context_object)));
    if js::type_of(context_type) == "object" && context_type.is_some() {
        let context_type: Rc<ReactContext<Option<&'static Unknown>>> =
            unsafe { js::cast(context_type) };
        next_context = read_context(&context_type);
    } else if !DISABLE_LEGACY_CONTEXT {
        let next_legacy_unmasked_context = get_unmasked_context(work_in_progress, ctor, true);
        next_context = Some(js::unknown(get_masked_context(
            work_in_progress,
            next_legacy_unmasked_context,
        )));
    }

    let get_derived_state_from_props = js::get(ctor, "getDerivedStateFromProps");
    let has_new_lifecycles = js::type_of(get_derived_state_from_props) == "function"
        || js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) == "function";

    // When comparing whether props changed, we should compare using the
    // unresolved props object that is stored on the fiber, rather than the
    // one that gets assigned to the instance, because that object may have been
    // cloned to resolve default props and/or remove `ref`.
    let unresolved_new_props = work_in_progress.pending_props.get();
    let did_receive_new_props = !js::strict_equals(unresolved_new_props, unresolved_old_props);

    // Note: During these life-cycles, instance.props/instance.state are what
    // ever the previously attempted to render - not the "current". However,
    // during componentDidUpdate we pass the "current" props.

    // In order to support react-lifecycles-compat polyfilled components,
    // Unsafe lifecycles should not be invoked for components using the new APIs.
    if !has_new_lifecycles
        && (js::type_of(js::get(instance, "UNSAFE_componentWillReceiveProps")) == "function"
            || js::type_of(js::get(instance, "componentWillReceiveProps")) == "function")
        && (did_receive_new_props || !js::strict_equals(old_context, next_context))
    {
        call_component_will_receive_props(work_in_progress, instance, new_props, next_context);
    }

    reset_has_force_update_before_processing();

    let old_state = work_in_progress.memoized_state.get();
    js::set(instance, "state", old_state);
    process_update_queue(
        work_in_progress,
        new_props,
        Some(js::unknown(instance)),
        render_lanes,
    );
    suspend_if_update_read_from_entangled_async_action();
    let mut new_state = work_in_progress.memoized_state.get();
    if !did_receive_new_props
        && js::strict_equals(old_state, new_state)
        && !has_context_changed()
        && !check_has_force_update_after_processing()
    {
        // If an update was already in progress, we should schedule an Update
        // effect even though we're bailing out, so that cWU/cDU are called.
        if js::type_of(js::get(instance, "componentDidMount")) == "function" {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE | LAYOUT_STATIC);
        }
        if __DEV__ && (work_in_progress.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | MOUNT_LAYOUT_DEV);
        }
        return false;
    }

    if js::type_of(get_derived_state_from_props) == "function" {
        apply_derived_state_from_props(
            work_in_progress,
            ctor,
            unsafe { js::cast(get_derived_state_from_props) },
            new_props,
        );
        new_state = work_in_progress.memoized_state.get();
    }

    let should_update = check_has_force_update_after_processing()
        || check_should_component_update(
            work_in_progress,
            ctor,
            old_props,
            new_props,
            old_state,
            new_state,
            next_context,
        );

    if should_update {
        // In order to support react-lifecycles-compat polyfilled components,
        // Unsafe lifecycles should not be invoked for components using the new APIs.
        if !has_new_lifecycles
            && (js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function"
                || js::type_of(js::get(instance, "componentWillMount")) == "function")
        {
            if js::type_of(js::get(instance, "componentWillMount")) == "function" {
                instance.component_will_mount();
            }
            if js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function" {
                instance.unsafe_component_will_mount();
            }
        }
        if js::type_of(js::get(instance, "componentDidMount")) == "function" {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE | LAYOUT_STATIC);
        }
        if __DEV__ && (work_in_progress.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | MOUNT_LAYOUT_DEV);
        }
    } else {
        // If an update was already in progress, we should schedule an Update
        // effect even though we're bailing out, so that cWU/cDU are called.
        if js::type_of(js::get(instance, "componentDidMount")) == "function" {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE | LAYOUT_STATIC);
        }
        if __DEV__ && (work_in_progress.mode.get() & STRICT_EFFECTS_MODE) != NO_MODE {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | MOUNT_LAYOUT_DEV);
        }

        // If shouldComponentUpdate returned false, we should still update the
        // memoized state to indicate that this work can be reused.
        work_in_progress.memoized_props.set(new_props);
        work_in_progress.memoized_state.set(new_state);
    }

    // Update the existing instance's state, props, and context pointers even
    // if shouldComponentUpdate returns false.
    js::set(instance, "props", new_props);
    js::set(instance, "state", new_state);
    js::set(instance, "context", next_context);

    should_update
}

// Invokes the update life-cycles and returns false if it shouldn't rerender.
pub fn update_class_instance(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    ctor: &'static Unknown,
    new_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) -> bool {
    let instance: &ClassInstance = unsafe { js::cast(work_in_progress.state_node.get()) };

    clone_update_queue(current, work_in_progress);

    let unresolved_old_props = work_in_progress.memoized_props.get();
    let old_props =
        resolve_class_component_props(Some(ctor), unsafe { js::cast(unresolved_old_props) });
    js::set(instance, "props", old_props);
    let unresolved_new_props = work_in_progress.pending_props.get();

    let old_context = js::get(instance, "context");
    let context_type = js::get(ctor, "contextType");
    let mut next_context =
        Some(EMPTY_CONTEXT_OBJECT.with(|empty_context_object| js::unknown(*empty_context_object)));
    if js::type_of(context_type) == "object" && context_type.is_some() {
        let context_type: Rc<ReactContext<Option<&'static Unknown>>> =
            unsafe { js::cast(context_type) };
        next_context = read_context(&context_type);
    } else if !DISABLE_LEGACY_CONTEXT {
        let next_unmasked_context = get_unmasked_context(work_in_progress, ctor, true);
        next_context = Some(js::unknown(get_masked_context(
            work_in_progress,
            next_unmasked_context,
        )));
    }

    let get_derived_state_from_props = js::get(ctor, "getDerivedStateFromProps");
    let has_new_lifecycles = js::type_of(get_derived_state_from_props) == "function"
        || js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) == "function";

    // Note: During these life-cycles, instance.props/instance.state are what
    // ever the previously attempted to render - not the "current". However,
    // during componentDidUpdate we pass the "current" props.

    // In order to support react-lifecycles-compat polyfilled components,
    // Unsafe lifecycles should not be invoked for components using the new APIs.
    if !has_new_lifecycles
        && (js::type_of(js::get(instance, "UNSAFE_componentWillReceiveProps")) == "function"
            || js::type_of(js::get(instance, "componentWillReceiveProps")) == "function")
        && (!js::strict_equals(unresolved_old_props, unresolved_new_props)
            || !js::strict_equals(old_context, next_context))
    {
        call_component_will_receive_props(work_in_progress, instance, new_props, next_context);
    }

    reset_has_force_update_before_processing();

    let old_state = work_in_progress.memoized_state.get();
    js::set(instance, "state", old_state);
    process_update_queue(
        work_in_progress,
        new_props,
        Some(js::unknown(instance)),
        render_lanes,
    );
    suspend_if_update_read_from_entangled_async_action();
    let mut new_state = work_in_progress.memoized_state.get();

    let current_dependencies = current.dependencies.borrow().clone();
    if js::strict_equals(unresolved_old_props, unresolved_new_props)
        && js::strict_equals(old_state, new_state)
        && !has_context_changed()
        && !check_has_force_update_after_processing()
        && !current_dependencies
            .as_deref()
            .is_some_and(check_if_context_changed)
    {
        // If an update was already in progress, we should schedule an Update
        // effect even though we're bailing out, so that cWU/cDU are called.
        if js::type_of(js::get(instance, "componentDidUpdate")) == "function"
            && (!js::strict_equals(unresolved_old_props, current.memoized_props.get())
                || !js::strict_equals(old_state, current.memoized_state.get()))
        {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE);
        }
        if js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) == "function"
            && (!js::strict_equals(unresolved_old_props, current.memoized_props.get())
                || !js::strict_equals(old_state, current.memoized_state.get()))
        {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | SNAPSHOT);
        }
        return false;
    }

    if js::type_of(get_derived_state_from_props) == "function" {
        apply_derived_state_from_props(
            work_in_progress,
            ctor,
            unsafe { js::cast(get_derived_state_from_props) },
            new_props,
        );
        new_state = work_in_progress.memoized_state.get();
    }

    let should_update = check_has_force_update_after_processing()
        || check_should_component_update(
            work_in_progress,
            ctor,
            old_props,
            new_props,
            old_state,
            new_state,
            next_context,
        )
        // TODO: In some cases, we'll end up checking if context has changed twice,
        // both before and after `shouldComponentUpdate` has been called. Not ideal,
        // but I'm loath to refactor this function. This only happens for memoized
        // components so it's not that common.
        || current_dependencies
            .as_deref()
            .is_some_and(check_if_context_changed);

    if should_update {
        // In order to support react-lifecycles-compat polyfilled components,
        // Unsafe lifecycles should not be invoked for components using the new APIs.
        if !has_new_lifecycles
            && (js::type_of(js::get(instance, "UNSAFE_componentWillUpdate")) == "function"
                || js::type_of(js::get(instance, "componentWillUpdate")) == "function")
        {
            if js::type_of(js::get(instance, "componentWillUpdate")) == "function" {
                instance.component_will_update(new_props, new_state, next_context);
            }
            if js::type_of(js::get(instance, "UNSAFE_componentWillUpdate")) == "function" {
                instance.unsafe_component_will_update(new_props, new_state, next_context);
            }
        }
        if js::type_of(js::get(instance, "componentDidUpdate")) == "function" {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE);
        }
        if js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) == "function" {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | SNAPSHOT);
        }
    } else {
        // If an update was already in progress, we should schedule an Update
        // effect even though we're bailing out, so that cWU/cDU are called.
        if js::type_of(js::get(instance, "componentDidUpdate")) == "function"
            && (!js::strict_equals(unresolved_old_props, current.memoized_props.get())
                || !js::strict_equals(old_state, current.memoized_state.get()))
        {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE);
        }
        if js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) == "function"
            && (!js::strict_equals(unresolved_old_props, current.memoized_props.get())
                || !js::strict_equals(old_state, current.memoized_state.get()))
        {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | SNAPSHOT);
        }

        // If shouldComponentUpdate returned false, we should still update the
        // memoized props/state to indicate that this work can be reused.
        work_in_progress.memoized_props.set(new_props);
        work_in_progress.memoized_state.set(new_state);
    }

    // Update the existing instance's state, props, and context pointers even
    // if shouldComponentUpdate returns false.
    js::set(instance, "props", new_props);
    js::set(instance, "state", new_state);
    js::set(instance, "context", next_context);

    should_update
}

pub fn resolve_class_component_props(
    component: Option<&'static Unknown>,
    base_props: &'static Unknown,
) -> &'static Unknown {
    let mut new_props = base_props;

    // Remove ref from the props object, if it exists.
    if js::has(base_props, "ref") {
        new_props = js::unknown(js::dict::new::<()>());
        for prop_name in js::keys_in(base_props) {
            if prop_name != "ref" {
                js::set(new_props, &prop_name, js::get(base_props, &prop_name));
            }
        }
    }

    // Resolve default props.
    let default_props = js::get(component, "defaultProps");
    if let Some(default_props) =
        default_props.filter(|default_props| js::truthy(Some(*default_props)))
    {
        // We may have already copied the props object above to remove ref. If so,
        // we can modify that. Otherwise, copy the props object with Object.assign.
        if js::strict_equals(new_props, base_props) {
            new_props = assign(js::unknown(js::dict::new::<()>()), (new_props,));
        }
        // Taken from old JSX runtime, where this used to live.
        for prop_name in js::keys_in(default_props) {
            if matches!(
                js::classify(js::get(new_props, &prop_name)),
                Kind::Undefined
            ) {
                js::set(new_props, &prop_name, js::get(default_props, &prop_name));
            }
        }
    }

    new_props
}
