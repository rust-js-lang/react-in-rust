// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Dict, Kind, Set, SymbolFor, Unknown};
use shared::environment::__DEV__;
use webapi::console;

use crate::react_fizz_legacy_context::EMPTY_CONTEXT_OBJECT;
use crate::react_fizz_new_context::{Context, read_context};

use shared::react_feature_flags::DISABLE_LEGACY_CONTEXT;
use shared::react_instance_map::{get as get_instance, set as set_instance};

use shared::get_component_name_from_type::get_component_name_from_type;
use shared::react_symbols::{REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE};

use shared::assign::assign;
use shared::is_array::is_array;

/// A class's static `getDerivedStateFromProps`: the state its props derive,
/// given its previous state.
pub type GetDerivedStateFromProps =
    dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>) -> Option<&'static Unknown>;

thread_local! {
    static DID_WARN_ABOUT_NOOP_UPDATE_FOR_COMPONENT: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_DEPRECATED_WILL_MOUNT: &'static Dict<bool> = js::dict::new();

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

fn warn_noop(public_instance: &'static Unknown, caller_name: &str) {
    if __DEV__ {
        let constructor = js::get(public_instance, "constructor");
        let component_name = (if js::truthy(constructor) {
            get_component_name_from_type(constructor)
        } else {
            None
        })
        .filter(|name| js::truthy(Some(name.as_str())))
        .unwrap_or_else(|| "ReactClass".to_string());
        let warning_key = component_name.clone() + "." + caller_name;
        if DID_WARN_ABOUT_NOOP_UPDATE_FOR_COMPONENT
            .with(|did_warn| js::dict::get(did_warn, &warning_key).is_some_and(|&warned| warned))
        {
            return;
        }

        console::error((
            concat!(
                "Can only update a mounting component. ",
                "This usually means you called %s() outside componentWillMount() on the server. ",
                "This is a no-op.\n\nPlease check the code for the %s component."
            ),
            caller_name,
            &component_name,
        ));
        DID_WARN_ABOUT_NOOP_UPDATE_FOR_COMPONENT
            .with(|did_warn| js::dict::set(did_warn, &warning_key, true));
    }
}

pub struct InternalInstance {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub queue: RefCell<Option<Vec<Option<&'static Unknown>>>>,
    pub replace: Cell<bool>,
}

unsafe impl js::Defined for InternalInstance {}

pub struct ClassComponentUpdater {
    pub enqueue_set_state: fn(&'static Unknown, Option<&'static Unknown>, Option<&'static Unknown>),
    pub enqueue_replace_state:
        fn(&'static Unknown, Option<&'static Unknown>, Option<&'static Unknown>),
    pub enqueue_force_update: fn(&'static Unknown, Option<&'static Unknown>),
}

#[cfg_attr(rust_js, rust_js::name = "classComponentUpdater")]
const CLASS_COMPONENT_UPDATER: ClassComponentUpdater = ClassComponentUpdater {
    enqueue_set_state: |inst, payload, callback| {
        let internals: &InternalInstance = unsafe { js::cast(get_instance(inst)) };
        if internals.queue.borrow().is_none() {
            warn_noop(inst, "setState");
        } else {
            internals.queue.borrow_mut().as_mut().unwrap().push(payload);
            if __DEV__ && callback.is_some() {
                warn_on_invalid_callback(callback);
            }
        }
    },
    enqueue_replace_state: |inst, payload, callback| {
        let internals: &InternalInstance = unsafe { js::cast(get_instance(inst)) };
        internals.replace.set(true);
        *internals.queue.borrow_mut() = Some(vec![payload]);
        if __DEV__ && callback.is_some() {
            warn_on_invalid_callback(callback);
        }
    },
    enqueue_force_update: |inst, callback| {
        let internals: &InternalInstance = unsafe { js::cast(get_instance(inst)) };
        if internals.queue.borrow().is_none() {
            warn_noop(inst, "forceUpdate");
        } else if __DEV__ && callback.is_some() {
            warn_on_invalid_callback(callback);
        }
    },
};

fn apply_derived_state_from_props(
    _instance: &Unknown,
    ctor: &Unknown,
    get_derived_state_from_props: &GetDerivedStateFromProps,
    prev_state: Option<&'static Unknown>,
    next_props: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let partial_state = get_derived_state_from_props(next_props, prev_state);

    if __DEV__ {
        warn_on_undefined_derived_state(ctor, partial_state);
    }
    // Merge the partial state and the previous state.
    match partial_state {
        None => prev_state,
        Some(partial_state) => Some(assign(
            js::unknown(js::dict::new::<()>()),
            (prev_state, Some(partial_state)),
        )),
    }
}

pub fn construct_class_instance(
    ctor: &'static Unknown,
    props: Option<&'static Unknown>,
    masked_legacy_context: Option<&'static Unknown>,
) -> &'static Unknown {
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
        let context_type: &'static Context = unsafe { js::cast(context_type) };
        context = read_context(context_type);
    } else if !DISABLE_LEGACY_CONTEXT {
        context = masked_legacy_context;
    }

    let instance: &'static Unknown = js::new(ctor, (props, context));

    if __DEV__ {
        if js::type_of(js::get(ctor, "getDerivedStateFromProps")) == "function"
            && js::get(instance, "state").is_none()
        {
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

    instance
}

fn check_class_instance(
    instance: Option<&'static Unknown>,
    ctor: &'static Unknown,
    new_props: Option<&Unknown>,
) {
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
                        "property to define contextTypes instead. (https://react.dev/link/legacy-context)"
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

fn call_component_will_mount(r#type: &'static Unknown, instance: &'static Unknown) {
    let old_state = js::get(instance, "state");

    if js::type_of(js::get(instance, "componentWillMount")) == "function" {
        if __DEV__
            && !js::strict_equals(
                js::get(
                    js::get(instance, "componentWillMount"),
                    "__suppressDeprecationWarning",
                ),
                true,
            )
        {
            let component_name = get_component_name_from_type(Some(r#type))
                .filter(|name| js::truthy(Some(name.as_str())))
                .unwrap_or_else(|| "Unknown".to_string());

            if !DID_WARN_ABOUT_DEPRECATED_WILL_MOUNT.with(|did_warn| {
                js::dict::get(did_warn, &component_name).is_some_and(|&warned| warned)
            }) {
                console::warn((
                    // keep this warning in sync with ReactStrictModeWarning.js
                    concat!(
                        "componentWillMount has been renamed, and is not recommended for use. ",
                        "See https://react.dev/link/unsafe-component-lifecycles for details.\n\n",
                        "* Move code from componentWillMount to componentDidMount (preferred in most cases) ",
                        "or the constructor.\n",
                        "\nPlease update the following components: %s"
                    ),
                    &component_name,
                ));
                DID_WARN_ABOUT_DEPRECATED_WILL_MOUNT
                    .with(|did_warn| js::dict::set(did_warn, &component_name, true));
            }
        }

        component_will_mount(instance);
    }
    if js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function" {
        unsafe_component_will_mount(instance);
    }

    if !js::strict_equals(old_state, js::get(instance, "state")) {
        if __DEV__ {
            console::error((
                concat!(
                    "%s.componentWillMount(): Assigning directly to this.state is ",
                    "deprecated (except inside a component's ",
                    "constructor). Use setState instead."
                ),
                get_component_name_from_type(Some(r#type))
                    .filter(|name| js::truthy(Some(name.as_str())))
                    .unwrap_or_else(|| "Component".to_string()),
            ));
        }
        (CLASS_COMPONENT_UPDATER.enqueue_replace_state)(
            instance,
            js::get(instance, "state"),
            js::null(),
        );
    }
}

#[cfg_attr(rust_js, rust_js::link_name = "componentWillMount")]
fn component_will_mount(this: &Unknown) {
    let _ = this;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "UNSAFE_componentWillMount")]
fn unsafe_component_will_mount(this: &Unknown) {
    let _ = this;
    unreachable!()
}

fn process_update_queue(
    internal_instance: &InternalInstance,
    inst: &'static Unknown,
    props: Option<&'static Unknown>,
    masked_legacy_context: Option<&'static Unknown>,
) {
    // Taken whether it's empty or not: its `null` either way.
    let queue = internal_instance.queue.take();
    if let Some(old_queue) = queue.filter(|queue| !queue.is_empty()) {
        let old_replace = internal_instance.replace.get();
        internal_instance.replace.set(false);

        if old_replace && old_queue.len() == 1 {
            js::set(inst, "state", old_queue[0]);
        } else {
            let mut next_state = if old_replace {
                old_queue[0]
            } else {
                js::get(inst, "state")
            };
            let mut dont_mutate = true;
            for &partial in &old_queue[if old_replace { 1 } else { 0 }..] {
                let partial_state = if js::type_of(partial) == "function" {
                    let partial: &dyn Fn(
                        Option<&'static Unknown>,
                        Option<&'static Unknown>,
                        Option<&'static Unknown>,
                    ) -> Option<&'static Unknown> = unsafe { js::cast(partial) };
                    js::function::call(
                        partial,
                        Some(inst),
                        (next_state, props, masked_legacy_context),
                    )
                } else {
                    partial
                };
                if let Some(partial_state) = partial_state {
                    if dont_mutate {
                        dont_mutate = false;
                        next_state = Some(assign(
                            js::unknown(js::dict::new::<()>()),
                            (next_state, Some(partial_state)),
                        ));
                    } else {
                        assign(next_state.unwrap(), (Some(partial_state),));
                    }
                }
            }
            js::set(inst, "state", next_state);
        }
    }
}

// Invokes the mount life-cycles on a previously never rendered instance.
pub fn mount_class_instance(
    instance: &'static Unknown,
    ctor: &'static Unknown,
    new_props: Option<&'static Unknown>,
    masked_legacy_context: Option<&'static Unknown>,
) {
    if __DEV__ {
        check_class_instance(Some(instance), ctor, new_props);
    }

    let initial_state = match js::classify(js::get(instance, "state")) {
        Kind::Undefined => js::null(),
        _ => js::get(instance, "state"),
    };

    js::set(instance, "updater", &CLASS_COMPONENT_UPDATER);
    js::set(instance, "props", new_props);
    js::set(instance, "state", initial_state);
    // We don't bother initializing the refs object on the server, since we're not going to resolve them anyway.

    // The internal instance will be used to manage updates that happen during this mount.
    let internal_instance = Rc::new(InternalInstance {
        queue: RefCell::new(Some(Vec::new())),
        replace: Cell::new(false),
    });
    set_instance(instance, js::unknown(&*internal_instance));

    let context_type = js::get(ctor, "contextType");
    if js::type_of(context_type) == "object" && context_type.is_some() {
        let context_type: &'static Context = unsafe { js::cast(context_type) };
        js::set(instance, "context", read_context(context_type));
    } else if DISABLE_LEGACY_CONTEXT {
        js::set(
            instance,
            "context",
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object),
        );
    } else {
        js::set(instance, "context", masked_legacy_context);
    }

    if __DEV__ && js::strict_equals(js::get(instance, "state"), new_props) {
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

    let get_derived_state_from_props = js::get(ctor, "getDerivedStateFromProps");
    if js::type_of(get_derived_state_from_props) == "function" {
        let get_derived_state_from_props: &GetDerivedStateFromProps =
            unsafe { js::cast(get_derived_state_from_props) };
        js::set(
            instance,
            "state",
            apply_derived_state_from_props(
                instance,
                ctor,
                get_derived_state_from_props,
                initial_state,
                new_props,
            ),
        );
    }

    // In order to support react-lifecycles-compat polyfilled components,
    // Unsafe lifecycles should not be invoked for components using the new APIs.
    if js::type_of(js::get(ctor, "getDerivedStateFromProps")) != "function"
        && js::type_of(js::get(instance, "getSnapshotBeforeUpdate")) != "function"
        && (js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function"
            || js::type_of(js::get(instance, "componentWillMount")) == "function")
    {
        call_component_will_mount(ctor, instance);
        // If we had additional state updates during this life-cycle, let's
        // process them now.
        process_update_queue(
            &internal_instance,
            instance,
            new_props,
            masked_legacy_context,
        );
    }
}
