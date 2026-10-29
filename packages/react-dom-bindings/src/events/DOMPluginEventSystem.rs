// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{JsError, Unknown, math, number, string};
use react_reconciler::react_current_fiber::run_with_fiber_in_dev;
use react_reconciler::react_fiber_tree_reflection::get_lowest_common_ancestor;
use react_reconciler::react_internal_types::Fiber;
use react_reconciler::react_work_tags::{
    HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL, HOST_ROOT, HOST_SINGLETON, HOST_TEXT,
    SCOPE_COMPONENT,
};
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    DISABLE_COMMENTS_AS_DOM_CONTAINERS, ENABLE_CREATE_EVENT_HANDLE_API, ENABLE_LEGACY_FB_SUPPORT,
    ENABLE_SCOPE_API, ENABLE_SCROLL_END_POLYFILL,
};
use shared::report_global_error::REPORT_GLOBAL_ERROR;
use webapi::{Element, Event, EventTarget, Node, console, node};

use crate::before_input_event_plugin;
use crate::change_event_plugin;
use crate::check_passive_events::PASSIVE_BROWSER_EVENTS_SUPPORTED;
use crate::current_replaying_event::is_replaying_event;
use crate::enter_leave_event_plugin;
use crate::event_listener::{
    add_event_bubble_listener, add_event_bubble_listener_with_passive_flag,
    add_event_capture_listener, add_event_capture_listener_with_passive_flag,
    remove_event_listener,
};
use crate::event_registry::ALL_NATIVE_EVENTS;
use crate::event_system_flags::{
    EventSystemFlags, IS_CAPTURE_PHASE, IS_EVENT_HANDLE_NON_MANAGED_NODE,
    IS_LEGACY_FB_SUPPORT_MODE, IS_NON_DELEGATED, SHOULD_NOT_DEFER_CLICK_FOR_FB_SUPPORT_MODE,
    SHOULD_NOT_PROCESS_POLYFILL_EVENT_PLUGINS,
};
use crate::form_action_event_plugin;
use crate::get_event_target::get_event_target;
use crate::get_listener::get_listener;
use crate::html_node_type::{COMMENT_NODE, DOCUMENT_NODE};
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::{
    get_closest_instance_from_node, get_event_handler_listeners, get_event_listener_set,
};
use crate::react_dom_event_listener::create_event_listener_wrapper_with_priority;
use crate::react_dom_update_batching::batched_updates;
use crate::react_synthetic_event_type::{KnownReactSyntheticEvent, ReactSyntheticEvent};
use crate::scroll_end_event_plugin;
use crate::select_event_plugin;
use crate::simple_event_plugin;

pub struct DispatchListener {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub instance: Option<Rc<Fiber>>,
    pub listener: &'static Unknown,
    pub current_target: &'static EventTarget,
}

pub struct DispatchEntry {
    pub event: &'static ReactSyntheticEvent,
    pub listeners: Vec<DispatchListener>,
}

pub type DispatchQueue = Vec<DispatchEntry>;

// TODO: remove top-level side effect.
js::on_load! {
    simple_event_plugin::register_events();
    enter_leave_event_plugin::register_events();
    change_event_plugin::register_events();
    select_event_plugin::register_events();
    before_input_event_plugin::register_events();
    if ENABLE_SCROLL_END_POLYFILL.with(|enable_scroll_end_polyfill| *enable_scroll_end_polyfill) {
        scroll_end_event_plugin::register_events();
    }
}

#[cfg_attr(rust_js, rust_js::nullable(target_inst, native_event_target))]
fn extract_events(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
    event_system_flags: EventSystemFlags,
    target_container: &'static EventTarget,
) {
    // TODO: we should remove the concept of a "SimpleEventPlugin".
    // This is the basic functionality of the event system. All
    // the other plugins are essentially polyfills. So the plugin
    // should probably be inlined somewhere and have its logic
    // be core the to event system. This would potentially allow
    // us to ship builds of React without the polyfilled plugins below.
    simple_event_plugin::extract_events(
        dispatch_queue,
        dom_event_name,
        target_inst.clone(),
        native_event,
        native_event_target,
        event_system_flags,
        target_container,
    );
    let should_process_polyfill_plugins =
        (event_system_flags & SHOULD_NOT_PROCESS_POLYFILL_EVENT_PLUGINS) == 0;
    // We don't process these events unless we are in the
    // event's native "bubble" phase, which means that we're
    // not in the capture phase. That's because we emulate
    // the capture phase here still. This is a trade-off,
    // because in an ideal world we would not emulate and use
    // the phases properly, like we do with the SimpleEvent
    // plugin. However, the plugins below either expect
    // emulation (EnterLeave) or use state localized to that
    // plugin (BeforeInput, Change, Select). The state in
    // these modules complicates things, as you'll essentially
    // get the case where the capture phase event might change
    // state, only for the following bubble event to come in
    // later and not trigger anything as the state now
    // invalidates the heuristics of the event plugin. We
    // could alter all these plugins to work in such ways, but
    // that might cause other unknown side-effects that we
    // can't foresee right now.
    if should_process_polyfill_plugins {
        enter_leave_event_plugin::extract_events(
            dispatch_queue,
            dom_event_name,
            target_inst.clone(),
            native_event,
            native_event_target,
            event_system_flags,
            target_container,
        );
        change_event_plugin::extract_events(
            dispatch_queue,
            dom_event_name,
            target_inst.clone(),
            native_event,
            native_event_target,
            event_system_flags,
            Some(target_container),
        );
        select_event_plugin::extract_events(
            dispatch_queue,
            dom_event_name,
            target_inst.clone(),
            native_event,
            native_event_target,
            event_system_flags,
            target_container,
        );
        before_input_event_plugin::extract_events(
            dispatch_queue,
            dom_event_name,
            target_inst.clone(),
            native_event,
            native_event_target,
            event_system_flags,
            target_container,
        );
        form_action_event_plugin::extract_events(
            dispatch_queue,
            dom_event_name,
            target_inst.clone(),
            native_event,
            native_event_target,
            event_system_flags,
            target_container,
        );
    }
    if ENABLE_SCROLL_END_POLYFILL.with(|enable_scroll_end_polyfill| *enable_scroll_end_polyfill) {
        scroll_end_event_plugin::extract_events(
            dispatch_queue,
            dom_event_name,
            target_inst,
            native_event,
            native_event_target,
            event_system_flags,
            Some(target_container),
        );
    }
}

thread_local! {
    // List of events that need to be individually attached to media elements.
    pub static MEDIA_EVENT_TYPES: Vec<&'static str> = vec![
        "abort",
        "canplay",
        "canplaythrough",
        "durationchange",
        "emptied",
        "encrypted",
        "ended",
        "error",
        "loadeddata",
        "loadedmetadata",
        "loadstart",
        "pause",
        "play",
        "playing",
        "progress",
        "ratechange",
        "resize",
        "seeked",
        "seeking",
        "stalled",
        "suspend",
        "timeupdate",
        "volumechange",
        "waiting",
    ];

    // We should not delegate these events to the container, but rather
    // set them on the actual target element itself. This is primarily
    // because these events do not consistently bubble in the DOM.
    pub static NON_DELEGATED_EVENTS: HashSet<&'static str> = MEDIA_EVENT_TYPES.with(|media_event_types| {
        [
            &[
                "beforetoggle",
                "cancel",
                "close",
                "invalid",
                "load",
                "scroll",
                "scrollend",
                "toggle",
            ][..],
            // In order to reduce bytes, we insert the above array of media events
            // into this Set. Note: the "error" event isn't an exclusive media event,
            // and can occur on other elements too. Rather than duplicate that event,
            // we just take it from the media events array.
            media_event_types,
        ]
        .concat()
        .into_iter()
        .collect()
    });
}

fn execute_dispatch(
    event: &ReactSyntheticEvent,
    listener: &Unknown,
    current_target: &'static EventTarget,
) {
    event.current_target.set(Some(current_target));
    let listener: &dyn Fn(&ReactSyntheticEvent) = unsafe { js::cast(Some(listener)) };
    if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| listener(event))) {
        REPORT_GLOBAL_ERROR.with(|report_global_error| {
            report_global_error(
                error
                    .downcast_ref::<&'static JsError>()
                    .and_then(|error| error.as_unknown()),
            );
        });
    }
    event.current_target.set(None);
}

fn process_dispatch_queue_items_in_order(
    event: &ReactSyntheticEvent,
    dispatch_listeners: &[DispatchListener],
    in_capture_phase: bool,
) {
    let mut previous_instance = None;
    if in_capture_phase {
        for DispatchListener {
            instance,
            current_target,
            listener,
        } in dispatch_listeners.iter().rev()
        {
            if Some(instance) != previous_instance && (event.is_propagation_stopped.get())() {
                return;
            }
            if __DEV__ && instance.is_some() {
                run_with_fiber_in_dev(
                    instance.clone(),
                    |event, listener, current_target, (), ()| {
                        execute_dispatch(event, listener, current_target)
                    },
                    event,
                    *listener,
                    *current_target,
                    (),
                    (),
                );
            } else {
                execute_dispatch(event, listener, current_target);
            }
            previous_instance = Some(instance);
        }
    } else {
        for DispatchListener {
            instance,
            current_target,
            listener,
        } in dispatch_listeners
        {
            if Some(instance) != previous_instance && (event.is_propagation_stopped.get())() {
                return;
            }
            if __DEV__ && instance.is_some() {
                run_with_fiber_in_dev(
                    instance.clone(),
                    |event, listener, current_target, (), ()| {
                        execute_dispatch(event, listener, current_target)
                    },
                    event,
                    *listener,
                    *current_target,
                    (),
                    (),
                );
            } else {
                execute_dispatch(event, listener, current_target);
            }
            previous_instance = Some(instance);
        }
    }
}

pub fn process_dispatch_queue(
    dispatch_queue: &DispatchQueue,
    event_system_flags: EventSystemFlags,
) {
    let in_capture_phase = (event_system_flags & IS_CAPTURE_PHASE) != 0;
    for DispatchEntry { event, listeners } in dispatch_queue {
        process_dispatch_queue_items_in_order(event, listeners, in_capture_phase);
        //  event system doesn't use pooling.
    }
}

#[cfg_attr(rust_js, rust_js::nullable(target_inst))]
fn dispatch_events_for_plugins(
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    native_event: &'static AnyNativeEvent,
    target_inst: Option<Rc<Fiber>>,
    target_container: &'static EventTarget,
) {
    let native_event_target = get_event_target(native_event);
    let mut dispatch_queue: DispatchQueue = vec![];
    extract_events(
        &mut dispatch_queue,
        dom_event_name,
        target_inst,
        native_event,
        native_event_target,
        event_system_flags,
        target_container,
    );
    process_dispatch_queue(&dispatch_queue, event_system_flags);
}

pub fn listen_to_non_delegated_event(dom_event_name: &str, target_element: &'static Element) {
    if __DEV__
        && !NON_DELEGATED_EVENTS
            .with(|non_delegated_events| non_delegated_events.contains(dom_event_name))
    {
        console::error((
            concat!(
                "Did not expect a listenToNonDelegatedEvent() call for \"%s\". ",
                "This is a bug in React. Please file an issue."
            ),
            dom_event_name,
        ));
    }
    let is_capture_phase_listener = false;
    let listener_set = get_event_listener_set(target_element);
    let listener_set_key = get_listener_set_key(dom_event_name, is_capture_phase_listener);
    if !listener_set.has(&listener_set_key) {
        add_trapped_event_listener(
            target_element,
            dom_event_name,
            IS_NON_DELEGATED,
            is_capture_phase_listener,
            None,
        );
        listener_set.add(&listener_set_key);
    }
}

pub fn listen_to_native_event(
    dom_event_name: &str,
    is_capture_phase_listener: bool,
    target: &'static EventTarget,
) {
    if __DEV__
        && NON_DELEGATED_EVENTS
            .with(|non_delegated_events| non_delegated_events.contains(dom_event_name))
        && !is_capture_phase_listener
    {
        console::error((
            concat!(
                "Did not expect a listenToNativeEvent() call for \"%s\" in the bubble phase. ",
                "This is a bug in React. Please file an issue."
            ),
            dom_event_name,
        ));
    }

    let mut event_system_flags = 0;
    if is_capture_phase_listener {
        event_system_flags |= IS_CAPTURE_PHASE;
    }
    add_trapped_event_listener(
        target,
        dom_event_name,
        event_system_flags,
        is_capture_phase_listener,
        None,
    );
}

// This is only used by createEventHandle when the
// target is not a DOM element. E.g. window.
pub fn listen_to_native_event_for_non_managed_event_target(
    dom_event_name: &str,
    is_capture_phase_listener: bool,
    target: &'static EventTarget,
) {
    let mut event_system_flags = IS_EVENT_HANDLE_NON_MANAGED_NODE;
    let listener_set = get_event_listener_set(target);
    let listener_set_key = get_listener_set_key(dom_event_name, is_capture_phase_listener);
    if !listener_set.has(&listener_set_key) {
        if is_capture_phase_listener {
            event_system_flags |= IS_CAPTURE_PHASE;
        }
        add_trapped_event_listener(
            target,
            dom_event_name,
            event_system_flags,
            is_capture_phase_listener,
            None,
        );
        listener_set.add(&listener_set_key);
    }
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "listeningMarker")]
    static LISTENING_MARKER: String = "_reactListening".to_owned()
        + &string::slice(&number::to_string(math::random(), Some(36)), 2.0, None);
}

pub fn listen_to_all_supported_events(root_container_element: &'static EventTarget) {
    LISTENING_MARKER.with(|listening_marker| {
        if !js::truthy(js::get(root_container_element, listening_marker)) {
            js::set(root_container_element, listening_marker, true);
            ALL_NATIVE_EVENTS.with(|all_native_events| {
                all_native_events.for_each(|dom_event_name| {
                    // We handle selectionchange separately because it
                    // doesn't bubble and needs to be on the document.
                    if dom_event_name != "selectionchange" {
                        if !NON_DELEGATED_EVENTS.with(|non_delegated_events| {
                            non_delegated_events.contains(dom_event_name)
                        }) {
                            listen_to_native_event(dom_event_name, false, root_container_element);
                        }
                        listen_to_native_event(dom_event_name, true, root_container_element);
                    }
                });
            });
            let owner_document: Option<&'static EventTarget> =
                if js::strict_equals(js::get(root_container_element, "nodeType"), DOCUMENT_NODE) {
                    Some(root_container_element)
                } else {
                    unsafe { js::cast(js::get(root_container_element, "ownerDocument")) }
                };
            // $FlowFixMe[invalid-compare]
            if let Some(owner_document) = owner_document {
                // The selectionchange event also needs deduplication
                // but it is attached to the document.
                if !js::truthy(js::get(owner_document, listening_marker)) {
                    js::set(owner_document, listening_marker, true);
                    listen_to_native_event("selectionchange", false, owner_document);
                }
            }
        }
    });
}

fn add_trapped_event_listener(
    mut target_container: &'static EventTarget,
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    is_capture_phase_listener: bool,
    #[cfg_attr(rust_js, rust_js::name = "isDeferredListenerForLegacyFBSupport")]
    is_deferred_listener_for_legacy_fb_support: Option<bool>,
) {
    let mut listener = create_event_listener_wrapper_with_priority(
        target_container,
        dom_event_name,
        event_system_flags,
    );
    // If passive option is not supported, then the event will be
    // active and not passive.
    let mut is_passive_listener = None;
    if PASSIVE_BROWSER_EVENTS_SUPPORTED.get() {
        // Browsers introduced an intervention, making these events
        // passive by default on document. React doesn't bind them
        // to document anymore, but changing this now would undo
        // the performance wins from the change. So we emulate
        // the existing behavior manually on the roots now.
        // https://github.com/react/react/issues/19651
        if dom_event_name == "touchstart"
            || dom_event_name == "touchmove"
            || dom_event_name == "wheel"
        {
            is_passive_listener = Some(true);
        }
    }

    target_container =
        if ENABLE_LEGACY_FB_SUPPORT && is_deferred_listener_for_legacy_fb_support == Some(true) {
            // A Document container's ownerDocument is null, so it must be used
            // as the deferral target itself.
            if js::strict_equals(js::get(target_container, "nodeType"), DOCUMENT_NODE) {
                target_container
            } else {
                unsafe { js::cast(js::get(target_container, "ownerDocument")) }
            }
        } else {
            target_container
        };

    let unsubscribe_listener = Rc::new(Cell::new(None));
    // When legacyFBSupport is enabled, it's for when we
    // want to add a one time event listener to a container.
    // This should only be used with enableLegacyFBSupport
    // due to requirement to provide compatibility with
    // internal FB www event tooling. This works by removing
    // the event listener as soon as it is invoked. We could
    // also attempt to use the {once: true} param on
    // addEventListener, but that requires support and some
    // browsers do not support this today, and given this is
    // to support legacy code patterns, it's likely they'll
    // need support for such browsers.
    if ENABLE_LEGACY_FB_SUPPORT && is_deferred_listener_for_legacy_fb_support == Some(true) {
        let original_listener = listener;
        let dom_event_name = dom_event_name.to_owned();
        let unsubscribe_listener = Rc::clone(&unsubscribe_listener);
        listener = webapi::listener(move |event: &Event| {
            if let Some(unsubscribe_listener) = unsubscribe_listener.get() {
                remove_event_listener(
                    target_container,
                    &dom_event_name,
                    unsubscribe_listener,
                    is_capture_phase_listener,
                );
            }
            original_listener(event);
        });
    }
    // TODO: There are too many combinations here. Consolidate them.
    if is_capture_phase_listener {
        if let Some(is_passive_listener) = is_passive_listener {
            unsubscribe_listener.set(Some(add_event_capture_listener_with_passive_flag(
                target_container,
                dom_event_name,
                listener,
                is_passive_listener,
            )));
        } else {
            unsubscribe_listener.set(Some(add_event_capture_listener(
                target_container,
                dom_event_name,
                listener,
            )));
        }
    } else if let Some(is_passive_listener) = is_passive_listener {
        unsubscribe_listener.set(Some(add_event_bubble_listener_with_passive_flag(
            target_container,
            dom_event_name,
            listener,
            is_passive_listener,
        )));
    } else {
        unsubscribe_listener.set(Some(add_event_bubble_listener(
            target_container,
            dom_event_name,
            listener,
        )));
    }
}

#[cfg_attr(rust_js, rust_js::name = "deferClickToDocumentForLegacyFBSupport")]
fn defer_click_to_document_for_legacy_fb_support(
    dom_event_name: &str,
    target_container: &'static EventTarget,
) {
    // We defer all click events with legacy FB support mode on.
    // This means we add a one time event listener to trigger
    // after the FB delegated listeners fire.
    #[cfg_attr(rust_js, rust_js::name = "isDeferredListenerForLegacyFBSupport")]
    let is_deferred_listener_for_legacy_fb_support = true;
    add_trapped_event_listener(
        target_container,
        dom_event_name,
        IS_LEGACY_FB_SUPPORT_MODE,
        false,
        Some(is_deferred_listener_for_legacy_fb_support),
    );
}

fn is_matching_root_container(grand_container: &Node, target_container: &Node) -> bool {
    grand_container == target_container
        || (!DISABLE_COMMENTS_AS_DOM_CONTAINERS
            && grand_container.node_type() == COMMENT_NODE
            && grand_container.parent_node() == Some(target_container))
}

#[cfg_attr(rust_js, rust_js::nullable(target_inst))]
pub fn dispatch_event_for_plugin_event_system(
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    native_event: &'static AnyNativeEvent,
    target_inst: Option<Rc<Fiber>>,
    target_container: &'static EventTarget,
) {
    let mut ancestor_inst = target_inst.clone();
    if (event_system_flags & IS_EVENT_HANDLE_NON_MANAGED_NODE) == 0
        && (event_system_flags & IS_NON_DELEGATED) == 0
    {
        let target_container_node = node::unchecked_from(target_container);

        // If we are using the legacy FB support flag, we
        // defer the event to the null with a one
        // time event listener so we can defer the event.
        if ENABLE_LEGACY_FB_SUPPORT
            // If our event flags match the required flags for entering
            // FB legacy mode and we are processing the "click" event,
            // then we can defer the event to the "document", to allow
            // for legacy FB support, where the expected behavior was to
            // match React < 16 behavior of delegated clicks to the doc.
            && dom_event_name == "click"
            && (event_system_flags & SHOULD_NOT_DEFER_CLICK_FOR_FB_SUPPORT_MODE) == 0
            && !is_replaying_event(native_event)
        {
            defer_click_to_document_for_legacy_fb_support(dom_event_name, target_container);
            return;
        }
        if target_inst.is_some() {
            // The below logic attempts to work out if we need to change
            // the target fiber to a different ancestor. We had similar logic
            // in the legacy event system, except the big difference between
            // systems is that the modern event system now has an event listener
            // attached to each React Root and React Portal Root. Together,
            // the DOM nodes representing these roots are the "rootContainer".
            // To figure out which ancestor instance we should use, we traverse
            // up the fiber tree from the target instance and attempt to find
            // root boundaries that match that of our current "rootContainer".
            // If we find that "rootContainer", we find the parent fiber
            // sub-tree for that root and make that our ancestor instance.
            let mut node = target_inst;

            'main_loop: loop {
                let Some(current) = node else {
                    return;
                };
                let node_tag = current.tag.get();
                if node_tag == HOST_ROOT || node_tag == HOST_PORTAL {
                    let mut container: Option<&'static Node> =
                        unsafe { js::cast(js::get(current.state_node.get(), "containerInfo")) };
                    if is_matching_root_container(
                        unsafe { js::cast(container.map(js::unknown)) },
                        target_container_node,
                    ) {
                        break;
                    }
                    if node_tag == HOST_PORTAL {
                        // The target is a portal, but it's not the rootContainer we're looking for.
                        // Normally portals handle their own events all the way down to the root.
                        // So we should be able to stop now. However, we don't know if this portal
                        // was part of *our* root.
                        let mut grand_node = current.r#return.borrow().clone();
                        while let Some(grand) = grand_node {
                            let grand_tag = grand.tag.get();
                            if grand_tag == HOST_ROOT || grand_tag == HOST_PORTAL {
                                let grand_container: &Node = unsafe {
                                    js::cast(js::get(grand.state_node.get(), "containerInfo"))
                                };
                                if is_matching_root_container(
                                    grand_container,
                                    target_container_node,
                                ) {
                                    // This is the rootContainer we're looking for and we found it as
                                    // a parent of the Portal. That means we can ignore it because the
                                    // Portal will bubble through to us.
                                    return;
                                }
                            }
                            grand_node = grand.r#return.borrow().clone();
                        }
                    }
                    // Now we need to find it's corresponding host fiber in the other
                    // tree. To do this we can use getClosestInstanceFromNode, but we
                    // need to validate that the fiber is a host instance, otherwise
                    // we need to traverse up through the DOM till we find the correct
                    // node that is from the other tree.
                    while let Some(container_node) = container {
                        let Some(parent_node) = get_closest_instance_from_node(container_node)
                        else {
                            return;
                        };
                        let parent_tag = parent_node.tag.get();
                        if parent_tag == HOST_COMPONENT
                            || parent_tag == HOST_TEXT
                            || parent_tag == HOST_HOISTABLE
                            || parent_tag == HOST_SINGLETON
                        {
                            ancestor_inst = Some(parent_node);
                            node = ancestor_inst.clone();
                            continue 'main_loop;
                        }
                        container = container_node.parent_node();
                    }
                }
                node = current.r#return.borrow().clone();
            }
        }
    }

    batched_updates(
        |(), _: Option<()>| {
            dispatch_events_for_plugins(
                dom_event_name,
                event_system_flags,
                native_event,
                ancestor_inst,
                target_container,
            )
        },
        (),
        None,
    );
}

#[cfg_attr(rust_js, rust_js::nullable(instance))]
fn create_dispatch_listener(
    instance: Option<Rc<Fiber>>,
    listener: &'static Unknown,
    current_target: &'static EventTarget,
) -> DispatchListener {
    DispatchListener {
        instance,
        listener,
        current_target,
    }
}

#[cfg_attr(rust_js, rust_js::nullable(target_fiber, react_name))]
pub fn accumulate_single_phase_listeners(
    target_fiber: Option<Rc<Fiber>>,
    react_name: Option<&str>,
    native_event_type: &str,
    in_capture_phase: bool,
    accumulate_target_only: bool,
    native_event: &AnyNativeEvent,
) -> Vec<DispatchListener> {
    let capture_name = react_name.map(|react_name| react_name.to_owned() + "Capture");
    let react_event_name = if in_capture_phase {
        capture_name.as_deref()
    } else {
        react_name
    };
    let mut listeners: Vec<DispatchListener> = vec![];

    let mut instance = target_fiber;
    let mut last_host_component: Option<&'static Unknown> = js::null();

    // Accumulate all instances and listeners via the target -> root path.
    while let Some(current) = instance {
        let state_node = current.state_node.get();
        let tag = current.tag.get();
        // Handle listeners that are on HostComponents (i.e. <div>)
        if (tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON)
            && let Some(state_node) = state_node
        {
            last_host_component = Some(state_node);

            // createEventHandle listeners
            if ENABLE_CREATE_EVENT_HANDLE_API {
                let event_handler_listeners = get_event_handler_listeners(state_node);
                if let Some(event_handler_listeners) = event_handler_listeners {
                    event_handler_listeners.for_each(|entry| {
                        if entry.r#type == native_event_type && entry.capture == in_capture_phase {
                            listeners.push(create_dispatch_listener(
                                Some(Rc::clone(&current)),
                                js::unknown(entry.callback),
                                unsafe { js::cast(last_host_component) },
                            ));
                        }
                    });
                }
            }

            // Standard React on* listeners, i.e. onClick or onClickCapture
            if let Some(react_event_name) = react_event_name {
                let listener = get_listener(&current, react_event_name);
                if let Some(listener) = listener {
                    listeners.push(create_dispatch_listener(
                        Some(Rc::clone(&current)),
                        listener,
                        unsafe { js::cast(last_host_component) },
                    ));
                }
            }
        } else if ENABLE_CREATE_EVENT_HANDLE_API
            && ENABLE_SCOPE_API
            && tag == SCOPE_COMPONENT
            && last_host_component.is_some()
            && let Some(state_node) = state_node
        {
            // Scopes
            let react_scope_instance = state_node;
            let event_handler_listeners = get_event_handler_listeners(react_scope_instance);
            if let Some(event_handler_listeners) = event_handler_listeners {
                event_handler_listeners.for_each(|entry| {
                    if entry.r#type == native_event_type && entry.capture == in_capture_phase {
                        listeners.push(create_dispatch_listener(
                            Some(Rc::clone(&current)),
                            js::unknown(entry.callback),
                            unsafe { js::cast(last_host_component) },
                        ));
                    }
                });
            }
        }
        // If we are only accumulating events for the target, then we don't
        // continue to propagate through the React fiber tree to find other
        // listeners.
        if accumulate_target_only {
            break;
        }
        // If we are processing the onBeforeBlur event, then we need to take
        // into consideration that part of the React tree might have been hidden
        // or deleted (as we're invoking this event during commit). We can find
        // this out by checking if intercept fiber set on the event matches the
        // current instance fiber. In which case, we should clear all existing
        // listeners.
        if ENABLE_CREATE_EVENT_HANDLE_API && native_event.r#type() == "beforeblur" {
            // $FlowFixMe[prop-missing] internal field
            let detached_intercept_fiber: Option<Rc<Fiber>> =
                unsafe { js::cast(js::get(native_event, "_detachedInterceptFiber")) };
            if let Some(detached_intercept_fiber) = detached_intercept_fiber
                && (Rc::ptr_eq(&detached_intercept_fiber, &current)
                    || *current.alternate.borrow() == Some(detached_intercept_fiber))
            {
                listeners = vec![];
            }
        }
        instance = current.r#return.borrow().clone();
    }
    listeners
}

// We should only use this function for:
// - BeforeInputEventPlugin
// - ChangeEventPlugin
// - SelectEventPlugin
// - ScrollEndEventPlugin
// This is because we only process these plugins
// in the bubble phase, so we need to accumulate two
// phase event listeners (via emulation).
#[cfg_attr(rust_js, rust_js::nullable(target_fiber))]
pub fn accumulate_two_phase_listeners(
    target_fiber: Option<Rc<Fiber>>,
    react_name: &str,
) -> Vec<DispatchListener> {
    let capture_name = react_name.to_owned() + "Capture";
    let mut listeners: Vec<DispatchListener> = vec![];
    let mut instance = target_fiber;

    // Accumulate all instances and listeners via the target -> root path.
    while let Some(current) = instance {
        let state_node = current.state_node.get();
        let tag = current.tag.get();
        // Handle listeners that are on HostComponents (i.e. <div>)
        if (tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON)
            && let Some(state_node) = state_node
        {
            let current_target: &EventTarget = unsafe { js::cast(Some(state_node)) };
            let capture_listener = get_listener(&current, &capture_name);
            if let Some(capture_listener) = capture_listener {
                listeners.insert(
                    0,
                    create_dispatch_listener(
                        Some(Rc::clone(&current)),
                        capture_listener,
                        current_target,
                    ),
                );
            }
            let bubble_listener = get_listener(&current, react_name);
            if let Some(bubble_listener) = bubble_listener {
                listeners.push(create_dispatch_listener(
                    Some(Rc::clone(&current)),
                    bubble_listener,
                    current_target,
                ));
            }
        }
        if current.tag.get() == HOST_ROOT {
            return listeners;
        }
        instance = current.r#return.borrow().clone();
    }
    // If we didn't reach the root it means we're unmounted and shouldn't
    // dispatch any events on the target.
    vec![]
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_parent(mut inst: Option<Rc<Fiber>>) -> Option<Rc<Fiber>> {
    inst.as_ref()?;
    loop {
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        inst = inst.and_then(|inst| inst.r#return.borrow().clone());
        // TODO: If this is a HostRoot we might want to bail out.
        // That is depending on if we want nested subtrees (layers) to bubble
        // events to their parent. We could also go through parentNode on the
        // host node but that wouldn't work for React Native and doesn't let us
        // do the portal feature.
        if !inst.as_ref().is_some_and(|inst| {
            inst.tag.get() != HOST_COMPONENT && inst.tag.get() != HOST_SINGLETON
        }) {
            break;
        }
    }
    inst
}

#[cfg_attr(rust_js, rust_js::nullable(common))]
fn accumulate_enter_leave_listeners_for_event(
    dispatch_queue: &mut DispatchQueue,
    event: &'static KnownReactSyntheticEvent,
    target: Rc<Fiber>,
    common: Option<&Rc<Fiber>>,
    in_capture_phase: bool,
) {
    let registration_name: &str =
        unsafe { js::cast(event.react_name.borrow().as_deref().map(js::unknown)) };
    let mut listeners: Vec<DispatchListener> = vec![];

    let mut instance = Some(target);
    while let Some(current) = instance {
        if Some(&current) == common {
            break;
        }
        let alternate = current.alternate.borrow().clone();
        let state_node = current.state_node.get();
        let tag = current.tag.get();
        if alternate.is_some() && alternate.as_ref() == common {
            break;
        }
        if (tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON)
            && let Some(state_node) = state_node
        {
            let current_target: &EventTarget = unsafe { js::cast(Some(state_node)) };
            if in_capture_phase {
                let capture_listener = get_listener(&current, registration_name);
                if let Some(capture_listener) = capture_listener {
                    listeners.insert(
                        0,
                        create_dispatch_listener(
                            Some(Rc::clone(&current)),
                            capture_listener,
                            current_target,
                        ),
                    );
                }
                // $FlowFixMe[constant-condition]
            } else if !in_capture_phase {
                let bubble_listener = get_listener(&current, registration_name);
                if let Some(bubble_listener) = bubble_listener {
                    listeners.push(create_dispatch_listener(
                        Some(Rc::clone(&current)),
                        bubble_listener,
                        current_target,
                    ));
                }
            }
        }
        instance = current.r#return.borrow().clone();
    }
    if !listeners.is_empty() {
        dispatch_queue.push(DispatchEntry { event, listeners });
    }
}

// We should only use this function for:
// - EnterLeaveEventPlugin
// This is because we only process this plugin
// in the bubble phase, so we need to accumulate two
// phase event listeners.
#[cfg_attr(rust_js, rust_js::nullable(enter_event, from, to))]
pub fn accumulate_enter_leave_two_phase_listeners(
    dispatch_queue: &mut DispatchQueue,
    leave_event: &'static KnownReactSyntheticEvent,
    enter_event: Option<&'static KnownReactSyntheticEvent>,
    from: Option<Rc<Fiber>>,
    to: Option<Rc<Fiber>>,
) {
    let common = if let (Some(from), Some(to)) = (&from, &to) {
        get_lowest_common_ancestor(from, to, &get_parent)
    } else {
        js::null()
    };

    if let Some(from) = from {
        accumulate_enter_leave_listeners_for_event(
            dispatch_queue,
            leave_event,
            from,
            common.as_ref(),
            false,
        );
    }
    if let Some(to) = to
        && let Some(enter_event) = enter_event
    {
        accumulate_enter_leave_listeners_for_event(
            dispatch_queue,
            enter_event,
            to,
            common.as_ref(),
            true,
        );
    }
}

pub fn accumulate_event_handle_non_managed_node_listeners(
    react_event_type: &str,
    current_target: &'static EventTarget,
    in_capture_phase: bool,
) -> Vec<DispatchListener> {
    let mut listeners: Vec<DispatchListener> = vec![];

    let event_listeners = get_event_handler_listeners(js::unknown(current_target));
    if let Some(event_listeners) = event_listeners {
        event_listeners.for_each(|entry| {
            if entry.r#type == react_event_type && entry.capture == in_capture_phase {
                listeners.push(create_dispatch_listener(
                    js::null(),
                    js::unknown(entry.callback),
                    current_target,
                ));
            }
        });
    }
    listeners
}

pub fn get_listener_set_key(dom_event_name: &str, capture: bool) -> String {
    format!(
        "{dom_event_name}__{}",
        if capture { "capture" } else { "bubble" }
    )
}
