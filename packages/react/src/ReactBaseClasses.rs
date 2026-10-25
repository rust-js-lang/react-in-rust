// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{PropertyDescriptor, Unknown, object};
use shared::assign::assign;
use shared::environment::__DEV__;
use webapi::console;

use crate::react_noop_update_queue::{REACT_NOOP_UPDATE_QUEUE, Updater};

/// An object of no properties.
pub struct EmptyObject {}

unsafe impl js::Defined for EmptyObject {}

#[cfg_attr(rust_js, rust_js::name = "emptyObject")]
static EMPTY_OBJECT: EmptyObject = EmptyObject {};
js::on_load! {
    if __DEV__ {
        object::freeze(&EMPTY_OBJECT);
    }
}

/// Base class helpers for the updating state of a component.
#[cfg_attr(rust_js, rust_js::class)]
pub struct Component {
    pub props: Option<&'static Unknown>,
    pub context: Option<&'static Unknown>,
    pub refs: &'static EmptyObject,
    pub updater: &'static Updater,
}

unsafe impl js::Defined for Component {}

#[cfg_attr(rust_js, rust_js::class)]
impl Component {
    pub const IS_REACT_COMPONENT: EmptyObject = EmptyObject {};

    pub fn new(
        props: Option<&'static Unknown>,
        context: Option<&'static Unknown>,
        updater: Option<&'static Updater>,
    ) -> Self {
        Component {
            props,
            context,
            // If a component has string refs, we will assign a different object later.
            refs: &EMPTY_OBJECT,
            // We initialize the default updater but the real one gets injected by the
            // renderer.
            updater: updater.unwrap_or(&REACT_NOOP_UPDATE_QUEUE),
        }
    }

    /// Sets a subset of the state. Always use this to mutate
    /// state. You should treat `this.state` as immutable.
    ///
    /// There is no guarantee that `this.state` will be immediately updated, so
    /// accessing `this.state` after calling this method may return the old value.
    ///
    /// There is no guarantee that calls to `setState` will run synchronously,
    /// as they may eventually be batched together.  You can provide an optional
    /// callback that will be executed when the call to setState is actually
    /// completed.
    ///
    /// When a function is provided to setState, it will be called at some point in
    /// the future (not synchronously). It will be called with the up to date
    /// component arguments (state, props, context). These values can be different
    /// from this.* because your function may be called after receiveProps but before
    /// shouldComponentUpdate, and this new state, props, and context will not yet be
    /// assigned to this.
    ///
    /// @param {object|function} partialState Next partial state or function to
    ///        produce next partial state to be merged with current state.
    /// @param {?function} callback Called after state is updated.
    /// @final
    /// @protected
    pub fn set_state(
        &self,
        partial_state: Option<&'static Unknown>,
        callback: Option<&'static dyn Fn()>,
    ) {
        if js::type_of(partial_state) != "object"
            && js::type_of(partial_state) != "function"
            && partial_state.is_some()
        {
            panic!(concat!(
                "takes an object of state variables to update or a ",
                "function which returns an object of state variables."
            ));
        }

        (self.updater.enqueue_set_state)(
            js::unknown(self),
            partial_state,
            callback,
            Some("setState"),
        );
    }

    /// Forces an update. This should only be invoked when it is known with
    /// certainty that we are **not** in a DOM transaction.
    ///
    /// You may want to call this when you know that some deeper aspect of the
    /// component's state has changed but `setState` was not called.
    ///
    /// This will not invoke `shouldComponentUpdate`, but it will invoke
    /// `componentWillUpdate` and `componentDidUpdate`.
    ///
    /// @param {?function} callback Called after update is complete.
    /// @final
    /// @protected
    pub fn force_update(&self, callback: Option<&'static dyn Fn()>) {
        (self.updater.enqueue_force_update)(js::unknown(self), callback, Some("forceUpdate"));
    }
}

/// The deprecated APIs, each its name and what to do instead.
struct DeprecatedApis {
    is_mounted: (&'static str, &'static str),
    replace_state: (&'static str, &'static str),
}

unsafe impl js::Defined for DeprecatedApis {}

/*
 * Deprecated APIs. These APIs used to exist on classic React classes but since
 * we would like to deprecate them, we're not going to move them over to this
 * modern base class. Instead, we define a getter that warns if it's accessed.
 */
js::on_load! {
    if __DEV__ {
        let deprecated_apis = DeprecatedApis {
            is_mounted: (
                "isMounted",
                concat!(
                    "Instead, make sure to clean up subscriptions and pending requests in ",
                    "componentWillUnmount to prevent memory leaks."
                ),
            ),
            replace_state: (
                "replaceState",
                concat!(
                    "Refactor your code to use setState instead (see ",
                    "https://github.com/react/react/issues/3236)."
                ),
            ),
        };
        let define_deprecation_warning = |method_name: &str, info: &(&str, &str)| {
            object::define_property(
                js::class::<Component>().prototype(),
                method_name,
                &PropertyDescriptor {
                    get: Some(&|| {
                        console::warn((
                            "%s(...) is deprecated in plain JavaScript React classes. %s",
                            info.0,
                            info.1,
                        ));
                        None
                    }),
                    ..Default::default()
                },
            );
        };
        for fn_name in js::keys_in(js::unknown(&deprecated_apis)) {
            if object::has_own_property.call(js::unknown(&deprecated_apis), &fn_name) {
                define_deprecation_warning(&fn_name, unsafe {
                    js::cast(js::get(js::unknown(&deprecated_apis), &fn_name))
                });
            }
        }
    }
}

#[cfg_attr(rust_js, rust_js::class)]
struct ComponentDummy {}

unsafe impl js::Defined for ComponentDummy {}

#[cfg_attr(rust_js, rust_js::class)]
impl ComponentDummy {
    fn new() -> Self {
        ComponentDummy {}
    }
}
js::on_load! {
    js::class::<ComponentDummy>().set_prototype(js::class::<Component>().prototype());
}

/// Convenience component with default shallow equality check for sCU.
#[cfg_attr(rust_js, rust_js::class)]
pub struct PureComponent {
    pub props: Option<&'static Unknown>,
    pub context: Option<&'static Unknown>,
    pub refs: &'static EmptyObject,
    pub updater: &'static Updater,
}

#[cfg_attr(rust_js, rust_js::class)]
impl PureComponent {
    pub fn new(
        props: Option<&'static Unknown>,
        context: Option<&'static Unknown>,
        updater: Option<&'static Updater>,
    ) -> Self {
        PureComponent {
            props,
            context,
            // If a component has string refs, we will assign a different object later.
            refs: &EMPTY_OBJECT,
            updater: updater.unwrap_or(&REACT_NOOP_UPDATE_QUEUE),
        }
    }
}

js::on_load! {
    let pure_component_prototype = js::unknown_of(ComponentDummy::new());
    js::class::<PureComponent>().set_prototype(pure_component_prototype);
    js::set(pure_component_prototype, "constructor", js::class::<PureComponent>());
    // Avoid an extra prototype jump for these methods.
    assign(pure_component_prototype, (js::class::<Component>().prototype(),));
    js::set(pure_component_prototype, "isPureReactComponent", true);
}
