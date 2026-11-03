// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::OnceCell;
use std::marker::PhantomData;
use std::rc::Rc;

use js::{JsObject, Kind, Unknown, function};
use shared::environment::__DEV__;
use shared::react_types::ReactNodeList;
use webapi::{EventTarget, console};

use react_reconciler::react_internal_types::{
    CaughtErrorInfo, ErrorInfo, FiberRoot, OnDefaultTransitionIndicator, OnError,
};

use crate::react_dom_root::{
    CreateRootOptions, HydrateRootOptions, ReactDOMHydrationRoot, ReactDOMRoot,
    create_root as create_root_impl, hydrate_root as hydrate_root_impl,
};

use react_dom_bindings::dom_plugin_event_system::listen_to_all_supported_events;
use react_dom_bindings::html_node_type::{COMMENT_NODE, DOCUMENT_NODE, ELEMENT_NODE};
use react_dom_bindings::react_dom_component_tree::{
    get_instance_from_node, is_container_marked_as_root, mark_container_as_root,
    unmark_container_as_root,
};
use react_dom_bindings::react_dom_container::is_valid_container;
use react_dom_bindings::react_fiber_config_dom::clear_container;
use shared::react_feature_flags::{DISABLE_COMMENTS_AS_DOM_CONTAINERS, DISABLE_LEGACY_MODE};

use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;
use react_reconciler::react_root_tags::LEGACY_ROOT;
use shared::get_component_name_from_type::get_component_name_from_type;

use react_reconciler::react_current_fiber::{CURRENT as CURRENT_OWNER, IS_RENDERING};

use shared::assign::assign;

use shared::noop::noop;

/// What www provides as `ReactFiberErrorDialog`.
struct ReactFiberErrorDialog(PhantomData<JsObject>);

/// What `showErrorDialog` is given.
struct CapturedError {
    error_boundary: Option<&'static Unknown>,
    error: Option<&'static Unknown>,
    component_stack: String,
}

unsafe impl js::Defined for CapturedError {}

impl ReactFiberErrorDialog {
    /// Whether React should log the error itself, `false` if not.
    #[cfg_attr(rust_js, rust_js::link_name = "showErrorDialog")]
    fn show_error_dialog(&self, captured_error: &CapturedError) -> Option<bool> {
        unreachable!()
    }
}

// Provided by www
#[cfg_attr(rust_js, rust_js::link_name = "get ReactFiberErrorDialog#*")]
#[cfg_attr(rust_js, rust_js::require)]
fn react_fiber_error_dialog() -> &'static ReactFiberErrorDialog {
    unreachable!()
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ReactFiberErrorDialogWWW")]
    static REACT_FIBER_ERROR_DIALOG_WWW: &'static ReactFiberErrorDialog = react_fiber_error_dialog();
}

js::on_load! {
    if !js::is_function((
        REACT_FIBER_ERROR_DIALOG_WWW.with(|&dialog| dialog),
        ReactFiberErrorDialog::show_error_dialog,
    )) {
        panic!("Expected ReactFiberErrorDialog.showErrorDialog to be a function.");
    }
}

fn www_on_uncaught_error(error: Option<&'static Unknown>, error_info: &ErrorInfo) {
    let component_stack = error_info.component_stack.clone().unwrap_or_default();
    let log_error = REACT_FIBER_ERROR_DIALOG_WWW.with(|dialog| {
        dialog.show_error_dialog(&CapturedError {
            error_boundary: js::null(),
            error,
            component_stack,
        })
    });

    // Allow injected showErrorDialog() to prevent default console.error logging.
    // This enables renderers like ReactNative to better manage redbox behavior.
    if log_error == Some(false) {
        return;
    }

    ReactFiberReconciler.default_on_uncaught_error(error, error_info);
}

fn www_on_caught_error(error: Option<&'static Unknown>, error_info: &CaughtErrorInfo) {
    let error_boundary = error_info.error_boundary;
    let component_stack = error_info.component_stack.clone().unwrap_or_default();
    let log_error = REACT_FIBER_ERROR_DIALOG_WWW.with(|dialog| {
        dialog.show_error_dialog(&CapturedError {
            error_boundary,
            error,
            component_stack,
        })
    });

    // Allow injected showErrorDialog() to prevent default console.error logging.
    // This enables renderers like ReactNative to better manage redbox behavior.
    if log_error == Some(false) {
        return;
    }

    ReactFiberReconciler.default_on_caught_error(error, error_info);
}

#[cfg_attr(rust_js, rust_js::name = "noopOnDefaultTransitionIndicator")]
const NOOP_ON_DEFAULT_TRANSITION_INDICATOR: &OnDefaultTransitionIndicator = &|| None;

pub fn create_root(
    // An `Element`, a `Document` or a `DocumentFragment`.
    container: &'static Unknown,
    options: Option<&'static CreateRootOptions>,
) -> ReactDOMRoot {
    create_root_impl(
        container,
        Some(unsafe {
            js::cast(Some(assign(
                js::unknown(&CreateRootOptions {
                    on_uncaught_error: Some(&www_on_uncaught_error),
                    on_caught_error: Some(&www_on_caught_error),
                    on_default_transition_indicator: Some(NOOP_ON_DEFAULT_TRANSITION_INDICATOR),
                    unstable_strict_mode: None,
                    unstable_transition_callbacks: None,
                    identifier_prefix: None,
                    on_recoverable_error: None,
                }),
                (options,),
            )))
        }),
    )
}

pub fn hydrate_root(
    // A `Document` or an `Element`.
    container: &'static Unknown,
    initial_children: Option<&'static ReactNodeList>,
    options: Option<&'static HydrateRootOptions>,
) -> ReactDOMHydrationRoot {
    hydrate_root_impl(
        container,
        initial_children,
        Some(unsafe {
            js::cast(Some(assign(
                js::unknown(&HydrateRootOptions {
                    on_uncaught_error: Some(&www_on_uncaught_error),
                    on_caught_error: Some(&www_on_caught_error),
                    on_default_transition_indicator: Some(NOOP_ON_DEFAULT_TRANSITION_INDICATOR),
                    on_hydrated: None,
                    on_deleted: None,
                    unstable_strict_mode: None,
                    unstable_transition_callbacks: None,
                    identifier_prefix: None,
                    on_recoverable_error: None,
                    form_state: None,
                }),
                (options,),
            )))
        }),
    )
}

fn top_level_update_warnings(
    // ReactFiberConfigDOM's `Container`.
    container: &'static Unknown,
) {
    if js::truthy(js::get(container, "_reactRootContainer"))
        && !js::strict_equals(js::get(container, "nodeType"), COMMENT_NODE)
    {
        let root: Rc<FiberRoot> = unsafe { js::cast(js::get(container, "_reactRootContainer")) };
        let current = root.current.borrow().clone();
        let host_instance = ReactFiberReconciler.find_host_instance_with_no_portals(&current);
        if js::truthy(host_instance)
            && !js::strict_equals(js::get(host_instance, "parentNode"), container)
        {
            console::error((concat!(
                "It looks like the React-rendered content of this ",
                "container was removed without using React. This is not ",
                "supported and will cause errors. Instead, call ",
                "ReactDOM.unmountComponentAtNode to empty a container."
            ),));
        }
    }

    let is_root_rendered_by_some_react = js::truthy(js::get(container, "_reactRootContainer"));
    let root_el = get_react_root_element_in_container(container);
    let has_non_root_react_child = root_el.is_some_and(|root_el| {
        get_instance_from_node(unsafe { js::cast(Some(root_el)) }).is_some()
    });

    if has_non_root_react_child && !is_root_rendered_by_some_react {
        console::error((concat!(
            "Replacing React-rendered children with a new root ",
            "component. If you intended to update the children of this node, ",
            "you should instead have the existing children update their state ",
            "and render the new components instead of calling ReactDOM.render."
        ),));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_react_root_element_in_container(container: &Unknown) -> Option<&'static Unknown> {
    if !js::truthy(Some(container)) {
        return None;
    }

    if js::strict_equals(js::get(container, "nodeType"), DOCUMENT_NODE) {
        js::get(container, "documentElement")
    } else {
        js::get(container, "firstChild")
    }
}

// This isn't reachable because onRecoverableError isn't called in the
// legacy API.
#[cfg_attr(rust_js, rust_js::name = "noopOnRecoverableError")]
const NOOP_ON_RECOVERABLE_ERROR: &OnError = &|_, _| noop();

#[cfg_attr(rust_js, rust_js::name = "legacyCreateRootFromDOMContainer")]
fn legacy_create_root_from_dom_container(
    // ReactFiberConfigDOM's `Container`.
    container: &'static Unknown,
    initial_children: Option<&'static ReactNodeList>,
    // A class component, if any.
    parent_component: Option<&'static Unknown>,
    mut callback: Option<&'static Unknown>,
    is_hydration_container: bool,
) -> Rc<FiberRoot> {
    if is_hydration_container {
        let created: Rc<OnceCell<Rc<FiberRoot>>> = Rc::default();
        if let Kind::Function(original_callback) = js::classify(callback) {
            let created = Rc::clone(&created);
            callback = Some(js::unknown_of(move || {
                let instance =
                    ReactFiberReconciler.get_public_root_instance(created.get().unwrap());
                function::call(original_callback, Some(&instance), ());
            }));
        }

        let root = ReactFiberReconciler.create_hydration_container(
            initial_children,
            callback,
            container,
            LEGACY_ROOT,
            None,          // hydrationCallbacks
            false,         // isStrictMode
            Some(false),   // concurrentUpdatesByDefaultOverride,
            String::new(), // identifierPrefix
            Rc::new(www_on_uncaught_error),
            Rc::new(www_on_caught_error),
            Rc::new(NOOP_ON_RECOVERABLE_ERROR),
            NOOP_ON_DEFAULT_TRANSITION_INDICATOR,
            // TODO(luna) Support hydration later
            js::null(),
            js::null(),
        );
        let _ = created.set(Rc::clone(&root));
        js::set(container, "_reactRootContainer", &root);
        let current = root.current.borrow().clone();
        mark_container_as_root(&current, container);

        let root_container_element: &'static EventTarget = if !DISABLE_COMMENTS_AS_DOM_CONTAINERS
            && js::strict_equals(js::get(container, "nodeType"), COMMENT_NODE)
        {
            unsafe { js::cast(js::get(container, "parentNode")) }
        } else {
            unsafe { js::cast(Some(container)) }
        };
        // $FlowFixMe[incompatible-type]
        listen_to_all_supported_events(root_container_element);

        ReactFiberReconciler.flush_sync_work();
        root
    } else {
        // First clear any existing content.
        clear_container(unsafe { js::cast(Some(container)) });

        let created: Rc<OnceCell<Rc<FiberRoot>>> = Rc::default();
        if let Kind::Function(original_callback) = js::classify(callback) {
            let created = Rc::clone(&created);
            callback = Some(js::unknown_of(move || {
                let instance =
                    ReactFiberReconciler.get_public_root_instance(created.get().unwrap());
                function::call(original_callback, Some(&instance), ());
            }));
        }

        let root = ReactFiberReconciler.create_container(
            container,
            LEGACY_ROOT,
            None,          // hydrationCallbacks
            false,         // isStrictMode
            Some(false),   // concurrentUpdatesByDefaultOverride,
            String::new(), // identifierPrefix
            Rc::new(www_on_uncaught_error),
            Rc::new(www_on_caught_error),
            Rc::new(NOOP_ON_RECOVERABLE_ERROR),
            NOOP_ON_DEFAULT_TRANSITION_INDICATOR,
            js::null(), // transitionCallbacks
        );
        let _ = created.set(Rc::clone(&root));
        js::set(container, "_reactRootContainer", &root);
        let current = root.current.borrow().clone();
        mark_container_as_root(&current, container);

        let root_container_element: &'static EventTarget = if !DISABLE_COMMENTS_AS_DOM_CONTAINERS
            && js::strict_equals(js::get(container, "nodeType"), COMMENT_NODE)
        {
            unsafe { js::cast(js::get(container, "parentNode")) }
        } else {
            unsafe { js::cast(Some(container)) }
        };
        // $FlowFixMe[incompatible-type]
        listen_to_all_supported_events(root_container_element);

        // Initial mount should not be batched.
        ReactFiberReconciler.update_container_sync(
            initial_children,
            &root,
            parent_component,
            callback,
        );
        ReactFiberReconciler.flush_sync_work();

        root
    }
}

fn warn_on_invalid_callback(callback: Option<&'static Unknown>) {
    if __DEV__
        && !matches!(js::classify(callback), Kind::Null)
        && !matches!(js::classify(callback), Kind::Function(_))
    {
        console::error((
            concat!(
                "Expected the last optional `callback` argument to be a ",
                "function. Instead received: %s."
            ),
            callback,
        ));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn legacy_render_subtree_into_container(
    // A class component, if any.
    parent_component: Option<&'static Unknown>,
    children: Option<&'static ReactNodeList>,
    // ReactFiberConfigDOM's `Container`.
    container: &'static Unknown,
    force_hydrate: bool,
    mut callback: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if __DEV__ {
        top_level_update_warnings(container);
        warn_on_invalid_callback(if matches!(js::classify(callback), Kind::Undefined) {
            js::null()
        } else {
            callback
        });
    }

    let maybe_root = js::get(container, "_reactRootContainer");
    let root: Rc<FiberRoot>;
    if !js::truthy(maybe_root) {
        // Initial mount
        root = legacy_create_root_from_dom_container(
            container,
            children,
            parent_component,
            callback,
            force_hydrate,
        );
    } else {
        root = unsafe { js::cast(maybe_root) };
        if let Kind::Function(original_callback) = js::classify(callback) {
            let root = Rc::clone(&root);
            callback = Some(js::unknown_of(move || {
                let instance = ReactFiberReconciler.get_public_root_instance(&root);
                function::call(original_callback, Some(&instance), ());
            }));
        }
        // Update
        ReactFiberReconciler.update_container(children, &root, parent_component, callback);
    }
    ReactFiberReconciler.get_public_root_instance(&root)
}

#[cfg_attr(rust_js, rust_js::name = "findDOMNode")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_dom_node(
    // An `Element` or a class component, if any.
    component_or_element: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if __DEV__ {
        let owner = CURRENT_OWNER.with_borrow(Clone::clone);
        if let Some(owner) = owner
            && IS_RENDERING.get()
            && owner.state_node.get().is_some()
        {
            let warned_about_refs_in_render =
                js::get(owner.state_node.get(), "_warnedAboutRefsInRender");
            if !js::truthy(warned_about_refs_in_render) {
                console::error((
                    concat!(
                        "%s is accessing findDOMNode inside its render(). ",
                        "render() should be a pure function of props and state. It should ",
                        "never access something that requires stale data from the previous ",
                        "render, such as refs. Move this logic to componentDidMount and ",
                        "componentDidUpdate instead."
                    ),
                    get_component_name_from_type(owner.r#type.get())
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "A component".to_string()),
                ));
            }
            js::set(owner.state_node.get(), "_warnedAboutRefsInRender", true);
        }
    }
    let component_or_element = component_or_element?;
    if js::strict_equals(js::get(component_or_element, "nodeType"), ELEMENT_NODE) {
        return Some(component_or_element);
    }
    if __DEV__ {
        return ReactFiberReconciler
            .find_host_instance_with_warning(component_or_element, "findDOMNode");
    }
    ReactFiberReconciler.find_host_instance(component_or_element)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn render(
    // A React element.
    element: Option<&'static ReactNodeList>,
    // ReactFiberConfigDOM's `Container`.
    container: &'static Unknown,
    callback: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if DISABLE_LEGACY_MODE {
        if __DEV__ {
            console::error(("ReactDOM.render was removed in React 19. Use createRoot instead.",));
        }
        panic!("ReactDOM: Unsupported Legacy Mode API.");
    }
    if __DEV__ {
        console::error((concat!(
            "ReactDOM.render has not been supported since React 18. Use createRoot ",
            "instead. Until you switch to the new API, your app will behave as ",
            "if it's running React 17. Learn ",
            "more: https://react.dev/link/switch-to-createroot"
        ),));
    }

    if !is_valid_container(Some(container)) {
        panic!("Target container is not a DOM element.");
    }

    if __DEV__ {
        let is_modern_root = is_container_marked_as_root(container)
            && matches!(
                js::classify(js::get(container, "_reactRootContainer")),
                Kind::Undefined
            );
        if is_modern_root {
            console::error((concat!(
                "You are calling ReactDOM.render() on a container that was previously ",
                "passed to ReactDOMClient.createRoot(). This is not supported. ",
                "Did you mean to call root.render(element)?"
            ),));
        }
    }
    legacy_render_subtree_into_container(
        js::null(),
        // $FlowFixMe[incompatible-type]
        element,
        container,
        false,
        callback,
    )
}

pub fn unmount_component_at_node(
    // ReactFiberConfigDOM's `Container`.
    container: &'static Unknown,
) -> bool {
    if DISABLE_LEGACY_MODE {
        if __DEV__ {
            console::error((
                "unmountComponentAtNode was removed in React 19. Use root.unmount() instead.",
            ));
        }
        panic!("ReactDOM: Unsupported Legacy Mode API.");
    }
    if !is_valid_container(Some(container)) {
        panic!("Target container is not a DOM element.");
    }

    if __DEV__ {
        let is_modern_root = is_container_marked_as_root(container)
            && matches!(
                js::classify(js::get(container, "_reactRootContainer")),
                Kind::Undefined
            );
        if is_modern_root {
            console::error((concat!(
                "You are calling ReactDOM.unmountComponentAtNode() on a container that was previously ",
                "passed to ReactDOMClient.createRoot(). This is not supported. Did you mean to call root.unmount()?"
            ),));
        }
    }

    if js::truthy(js::get(container, "_reactRootContainer")) {
        let root: Rc<FiberRoot> = unsafe { js::cast(js::get(container, "_reactRootContainer")) };

        if __DEV__ {
            let root_el = get_react_root_element_in_container(container);
            let rendered_by_different_react = root_el.is_some_and(|root_el| {
                get_instance_from_node(unsafe { js::cast(Some(root_el)) }).is_none()
            });
            if rendered_by_different_react {
                console::error((concat!(
                    "unmountComponentAtNode(): The node you're attempting to unmount ",
                    "was rendered by another copy of React."
                ),));
            }
        }

        ReactFiberReconciler.update_container_sync(js::null(), &root, js::null(), js::null());
        ReactFiberReconciler.flush_sync_work();
        // $FlowFixMe[incompatible-type] This should probably use `delete container._reactRootContainer`
        js::set(container, "_reactRootContainer", js::null::<&Unknown>());
        unmark_container_as_root(container);
        true
    } else {
        if __DEV__ {
            let root_el = get_react_root_element_in_container(container);
            let has_non_root_react_child = root_el.is_some_and(|root_el| {
                get_instance_from_node(unsafe { js::cast(Some(root_el)) }).is_some()
            });

            // Check if the container itself is a React root node.
            let is_container_react_root = js::strict_equals(js::get(container, "nodeType"), ELEMENT_NODE)
                    && is_valid_container(js::get(container, "parentNode"))
                    // $FlowFixMe[prop-missing]
                    // $FlowFixMe[incompatible-use]
                    && js::truthy(js::get(
                        js::get(container, "parentNode"),
                        "_reactRootContainer",
                    ));

            if has_non_root_react_child {
                console::error((
                    concat!(
                        "unmountComponentAtNode(): The node you're attempting to unmount ",
                        "was rendered by React and is not a top-level container. %s"
                    ),
                    if is_container_react_root {
                        concat!(
                            "You may have accidentally passed in a React root node instead ",
                            "of its container."
                        )
                    } else {
                        concat!(
                            "Instead, have the parent component update its state and ",
                            "rerender in order to remove this component."
                        )
                    },
                ));
            }
        }

        false
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_batchedUpdates")]
pub use react_reconciler::react_fiber_work_loop::batched_updates as unstable_batched_updates;
