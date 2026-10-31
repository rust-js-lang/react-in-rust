// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use js::{Kind, SymbolFor, Unknown};
use react_dom_bindings::dom_plugin_event_system::listen_to_all_supported_events;
use react_dom_bindings::html_node_type::COMMENT_NODE;
use react_dom_bindings::react_dom_component_tree::{
    is_container_marked_as_root, mark_container_as_root, unmark_container_as_root,
};
use react_dom_bindings::react_dom_container::is_valid_container;
use react_dom_bindings::react_dom_event_replaying::queue_explicit_hydration_target;
use react_reconciler::react_fiber_error_logger::{
    default_on_caught_error, default_on_recoverable_error, default_on_uncaught_error,
};
use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;
use react_reconciler::react_internal_types::{
    FiberRoot, OnCaughtError, OnDefaultTransitionIndicator, OnError, TransitionTracingCallbacks,
};
use react_reconciler::react_root_tags::CONCURRENT_ROOT;
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    DISABLE_COMMENTS_AS_DOM_CONTAINERS, ENABLE_DEFAULT_TRANSITION_INDICATOR,
};
use shared::react_symbols::REACT_ELEMENT_TYPE;
use webapi::{EventTarget, Node, console};

use crate::react_dom_default_transition_indicator::default_on_default_transition_indicator;

/// What `createRoot` and `hydrateRoot` give: a root to render into and
/// unmount, the same two methods each.
pub trait RootType {
    /// `_internalRoot`, until it's unmounted.
    fn internal_root(&self) -> &RefCell<Option<Rc<FiberRoot>>>;

    fn render(
        &self,
        // shared/ReactTypes' `ReactNodeList`.
        children: Option<&'static Unknown>,
        #[cfg_attr(rust_js, rust_js::rest)] args: &[Option<&'static Unknown>],
    ) {
        let Some(root) = self.internal_root().borrow().clone() else {
            panic!("Cannot update an unmounted root.");
        };

        if __DEV__ {
            let second = args.first().copied().flatten();
            if matches!(js::classify(second), Kind::Function(_)) {
                console::error((concat!(
                    "does not support the second callback argument. ",
                    "To execute a side effect after rendering, declare it in a component body with useEffect()."
                ),));
            } else if is_valid_container(second) {
                console::error((concat!(
                    "You passed a container to the second argument of root.render(...). ",
                    "You don't need to pass it again since you already passed it to create the root."
                ),));
            } else if !matches!(js::classify(second), Kind::Undefined) {
                console::error((concat!(
                    "You passed a second argument to root.render(...) but it only accepts ",
                    "one argument."
                ),));
            }
        }
        ReactFiberReconciler.update_container(children, &root, None, None);
    }

    fn unmount(&self, #[cfg_attr(rust_js, rust_js::rest)] args: &[Option<&'static Unknown>]) {
        if __DEV__
            && matches!(
                js::classify(args.first().copied().flatten()),
                Kind::Function(_)
            )
        {
            console::error((concat!(
                "does not support a callback argument. ",
                "To execute a side effect after rendering, declare it in a component body with useEffect()."
            ),));
        }
        let root = self.internal_root().borrow().clone();
        if let Some(root) = root {
            self.internal_root().replace(None);
            let container = root.container_info;
            if __DEV__ && ReactFiberReconciler.is_already_rendering() {
                console::error((concat!(
                    "Attempted to synchronously unmount a root while React was already ",
                    "rendering. React cannot finish unmounting the root until the ",
                    "current render has completed, which may lead to a race condition."
                ),));
            }
            ReactFiberReconciler.update_container_sync(js::null(), &root, None, None);
            ReactFiberReconciler.flush_sync_work();
            unmark_container_as_root(container);
        }
    }
}

pub struct CreateRootOptions {
    #[cfg_attr(rust_js, rust_js::name = "unstable_strictMode")]
    pub unstable_strict_mode: Option<bool>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_transitionCallbacks")]
    pub unstable_transition_callbacks: Option<&'static TransitionTracingCallbacks>,
    pub identifier_prefix: Option<String>,
    pub on_uncaught_error: Option<&'static OnError>,
    pub on_caught_error: Option<&'static OnCaughtError>,
    pub on_recoverable_error: Option<&'static OnError>,
    pub on_default_transition_indicator: Option<&'static OnDefaultTransitionIndicator>,
}

unsafe impl js::Defined for CreateRootOptions {}

pub struct HydrateRootOptions {
    // Hydration options
    pub on_hydrated: Option<&'static dyn Fn(&'static webapi::Comment)>,
    pub on_deleted: Option<&'static dyn Fn(&'static webapi::Comment)>,
    // Options for all roots
    #[cfg_attr(rust_js, rust_js::name = "unstable_strictMode")]
    pub unstable_strict_mode: Option<bool>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_transitionCallbacks")]
    pub unstable_transition_callbacks: Option<&'static TransitionTracingCallbacks>,
    pub identifier_prefix: Option<String>,
    pub on_uncaught_error: Option<&'static OnError>,
    pub on_caught_error: Option<&'static OnCaughtError>,
    pub on_recoverable_error: Option<&'static OnError>,
    pub on_default_transition_indicator: Option<&'static OnDefaultTransitionIndicator>,
    // shared/ReactTypes' `ReactFormState<any, any>`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub form_state: Option<&'static Unknown>,
}

unsafe impl js::Defined for HydrateRootOptions {}

#[cfg_attr(rust_js, rust_js::class)]
pub struct ReactDOMRoot {
    #[cfg_attr(rust_js, rust_js::name = "_internalRoot", rust_js::nullable)]
    pub internal_root: RefCell<Option<Rc<FiberRoot>>>,
}

unsafe impl js::Defined for ReactDOMRoot {}

#[cfg_attr(rust_js, rust_js::class)]
impl ReactDOMRoot {
    fn new(internal_root: Rc<FiberRoot>) -> Self {
        ReactDOMRoot {
            internal_root: RefCell::new(Some(internal_root)),
        }
    }
}

impl RootType for ReactDOMRoot {
    fn internal_root(&self) -> &RefCell<Option<Rc<FiberRoot>>> {
        &self.internal_root
    }
}

pub fn create_root(
    // An `Element`, a `Document` or a `DocumentFragment`.
    container: &'static Unknown,
    options: Option<&'static CreateRootOptions>,
) -> ReactDOMRoot {
    if !is_valid_container(Some(container)) {
        panic!("Target container is not a DOM element.");
    }

    warn_if_react_dom_container_in_dev(container);

    let concurrent_updates_by_default_override = false;
    let mut is_strict_mode = false;
    let mut identifier_prefix = String::new();
    let mut on_uncaught_error: Rc<OnError> = Rc::new(default_on_uncaught_error);
    let mut on_caught_error: Rc<OnCaughtError> = Rc::new(default_on_caught_error);
    let mut on_recoverable_error: Rc<OnError> = Rc::new(default_on_recoverable_error);
    let mut on_default_transition_indicator: &'static OnDefaultTransitionIndicator =
        &default_on_default_transition_indicator;
    let mut transition_callbacks = js::null();

    if let Some(options) = options {
        if __DEV__ {
            if js::truthy(js::get(options, "hydrate")) {
                console::warn((
                    "hydrate through createRoot is deprecated. Use ReactDOMClient.hydrateRoot(container, <App />) instead.",
                ));
            } else if js::classify(Some(js::unknown(options))).object().is_some()
                && SymbolFor::from_value(js::get(options, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
            {
                console::error((concat!(
                    "You passed a JSX element to createRoot. You probably meant to ",
                    "call root.render instead. ",
                    "Example usage:\n\n",
                    "  let root = createRoot(domContainer);\n",
                    "  root.render(<App />);"
                ),));
            }
        }
        if js::strict_equals(options.unstable_strict_mode, true) {
            is_strict_mode = true;
        }
        if let Some(prefix) = &options.identifier_prefix {
            identifier_prefix = prefix.clone();
        }
        if let Some(on_error) = options.on_uncaught_error {
            on_uncaught_error = Rc::new(on_error);
        }
        if let Some(on_error) = options.on_caught_error {
            on_caught_error = Rc::new(on_error);
        }
        if let Some(on_error) = options.on_recoverable_error {
            on_recoverable_error = Rc::new(on_error);
        }
        if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled)
            && let Some(indicator) = options.on_default_transition_indicator
        {
            on_default_transition_indicator = indicator;
        }
        if options.unstable_transition_callbacks.is_some() {
            transition_callbacks = options.unstable_transition_callbacks;
        }
    }

    let root = ReactFiberReconciler.create_container(
        container,
        CONCURRENT_ROOT,
        None,
        is_strict_mode,
        Some(concurrent_updates_by_default_override),
        identifier_prefix,
        on_uncaught_error,
        on_caught_error,
        on_recoverable_error,
        on_default_transition_indicator,
        transition_callbacks,
    );
    let current = root.current.borrow().clone();
    mark_container_as_root(&current, container);

    let root_container_element: &'static EventTarget = if !DISABLE_COMMENTS_AS_DOM_CONTAINERS
        && js::strict_equals(js::get(container, "nodeType"), COMMENT_NODE)
    {
        unsafe { js::cast(js::get(container, "parentNode")) }
    } else {
        unsafe { js::cast(Some(container)) }
    };
    listen_to_all_supported_events(root_container_element);

    ReactDOMRoot::new(root)
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct ReactDOMHydrationRoot {
    #[cfg_attr(rust_js, rust_js::name = "_internalRoot", rust_js::nullable)]
    pub internal_root: RefCell<Option<Rc<FiberRoot>>>,
}

unsafe impl js::Defined for ReactDOMHydrationRoot {}

#[cfg_attr(rust_js, rust_js::class)]
impl ReactDOMHydrationRoot {
    fn new(internal_root: Rc<FiberRoot>) -> Self {
        ReactDOMHydrationRoot {
            internal_root: RefCell::new(Some(internal_root)),
        }
    }

    #[cfg_attr(rust_js, rust_js::name = "unstable_scheduleHydration")]
    pub fn unstable_schedule_hydration(&self, target: Option<&'static Node>) {
        schedule_hydration(target);
    }
}

impl RootType for ReactDOMHydrationRoot {
    fn internal_root(&self) -> &RefCell<Option<Rc<FiberRoot>>> {
        &self.internal_root
    }
}

fn schedule_hydration(target: Option<&'static Node>) {
    if let Some(target) = target
        && js::truthy(Some(target))
    {
        queue_explicit_hydration_target(target);
    }
}

pub fn hydrate_root(
    // A `Document` or an `Element`.
    container: &'static Unknown,
    // shared/ReactTypes' `ReactNodeList`.
    initial_children: Option<&'static Unknown>,
    options: Option<&'static HydrateRootOptions>,
) -> ReactDOMHydrationRoot {
    if !is_valid_container(Some(container)) {
        panic!("Target container is not a DOM element.");
    }

    warn_if_react_dom_container_in_dev(container);

    if __DEV__ && matches!(js::classify(initial_children), Kind::Undefined) {
        console::error((concat!(
            "Must provide initial children as second argument to hydrateRoot. ",
            "Example usage: hydrateRoot(domContainer, <App />)"
        ),));
    }

    // For now we reuse the whole bag of options since they contain
    // the hydration callbacks.
    let hydration_callbacks = match options {
        Some(options) => unsafe { js::cast(Some(js::unknown(options))) },
        None => js::null(),
    };

    let concurrent_updates_by_default_override = false;
    let mut is_strict_mode = false;
    let mut identifier_prefix = String::new();
    let mut on_uncaught_error: Rc<OnError> = Rc::new(default_on_uncaught_error);
    let mut on_caught_error: Rc<OnCaughtError> = Rc::new(default_on_caught_error);
    let mut on_recoverable_error: Rc<OnError> = Rc::new(default_on_recoverable_error);
    let mut on_default_transition_indicator: &'static OnDefaultTransitionIndicator =
        &default_on_default_transition_indicator;
    let mut transition_callbacks = js::null();
    let mut form_state = js::null();
    if let Some(options) = options {
        if js::strict_equals(options.unstable_strict_mode, true) {
            is_strict_mode = true;
        }
        if let Some(prefix) = &options.identifier_prefix {
            identifier_prefix = prefix.clone();
        }
        if let Some(on_error) = options.on_uncaught_error {
            on_uncaught_error = Rc::new(on_error);
        }
        if let Some(on_error) = options.on_caught_error {
            on_caught_error = Rc::new(on_error);
        }
        if let Some(on_error) = options.on_recoverable_error {
            on_recoverable_error = Rc::new(on_error);
        }
        if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled)
            && let Some(indicator) = options.on_default_transition_indicator
        {
            on_default_transition_indicator = indicator;
        }
        if options.unstable_transition_callbacks.is_some() {
            transition_callbacks = options.unstable_transition_callbacks;
        }
        if options.form_state.is_some() {
            form_state = options.form_state;
        }
    }

    let root = ReactFiberReconciler.create_hydration_container(
        initial_children,
        js::null(),
        container,
        CONCURRENT_ROOT,
        hydration_callbacks,
        is_strict_mode,
        Some(concurrent_updates_by_default_override),
        identifier_prefix,
        on_uncaught_error,
        on_caught_error,
        on_recoverable_error,
        on_default_transition_indicator,
        transition_callbacks,
        form_state,
    );
    let current = root.current.borrow().clone();
    mark_container_as_root(&current, container);
    // This can't be a comment node since hydration doesn't work on comment nodes anyway.
    listen_to_all_supported_events(unsafe { js::cast(Some(container)) });

    ReactDOMHydrationRoot::new(root)
}

#[cfg_attr(rust_js, rust_js::name = "warnIfReactDOMContainerInDEV")]
fn warn_if_react_dom_container_in_dev(container: &Unknown) {
    if __DEV__ && is_container_marked_as_root(container) {
        if js::truthy(js::get(container, "_reactRootContainer")) {
            console::error((concat!(
                "You are calling ReactDOMClient.createRoot() on a container that was previously ",
                "passed to ReactDOM.render(). This is not supported."
            ),));
        } else {
            console::error((concat!(
                "You are calling ReactDOMClient.createRoot() on a container that ",
                "has already been passed to createRoot() before. Instead, call ",
                "root.render() on the existing root instead if you want to update it."
            ),));
        }
    }
}
