// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use js::{Kind, Unknown};
use shared::react_feature_flags::{ENABLE_CREATE_EVENT_HANDLE_API, ENABLE_SCOPE_API};
use webapi::EventTarget;

use crate::dom_event_names::DOMEventName;
use crate::event_registry::ALL_NATIVE_EVENTS;
use crate::html_node_type::ELEMENT_NODE;
use crate::react_dom_component_tree::{
    add_event_handle_to_target, does_target_have_event_handle, get_event_handler_listeners,
    set_event_handler_listeners,
};
use crate::react_dom_event_handle_types::{ReactDOMEventHandle, ReactDOMEventHandleListener};
use crate::react_synthetic_event_type::ReactSyntheticEvent;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/events/DOMPluginEventSystem.js#listenToNativeEventForNonManagedEventTarget"
)]
fn listen_to_native_event_for_non_managed_event_target(
    dom_event_name: &str,
    is_capture_phase_listener: bool,
    target: &EventTarget,
) {
    let _ = (dom_event_name, is_capture_phase_listener, target);
    unreachable!()
}

pub struct EventHandleOptions {
    pub capture: Option<&'static Unknown>,
}

unsafe impl js::Defined for EventHandleOptions {}

fn is_valid_event_target(target: &Unknown) -> bool {
    js::type_of(js::get(target, "addEventListener")) == "function"
}

fn is_react_scope(target: &Unknown) -> bool {
    js::type_of(js::get(target, "getChildContextValues")) == "function"
}

fn create_event_handle_listener(
    r#type: DOMEventName,
    is_capture_phase_listener: bool,
    callback: &'static dyn Fn(&ReactSyntheticEvent),
) -> ReactDOMEventHandleListener {
    ReactDOMEventHandleListener {
        callback,
        capture: is_capture_phase_listener,
        r#type,
    }
}

#[cfg_attr(rust_js, rust_js::name = "registerReactDOMEvent")]
fn register_react_dom_event(
    target: &'static Unknown,
    dom_event_name: &str,
    is_capture_phase_listener: bool,
) {
    if js::strict_equals(js::get(target, "nodeType"), ELEMENT_NODE)
        || (ENABLE_SCOPE_API && is_react_scope(target))
    {
        // Do nothing. We already attached all root listeners.
    } else if is_valid_event_target(target) {
        let event_target: &EventTarget = unsafe { js::cast(Some(target)) };
        // These are valid event targets, but they are also
        // non-managed React nodes.
        listen_to_native_event_for_non_managed_event_target(
            dom_event_name,
            is_capture_phase_listener,
            event_target,
        );
    } else {
        panic!(concat!(
            "ReactDOM.createEventHandle: setter called on an invalid ",
            "target. Provide a valid EventTarget or an element managed by React."
        ));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn create_event_handle(
    r#type: &str,
    options: Option<&EventHandleOptions>,
) -> Option<Rc<ReactDOMEventHandle>> {
    if ENABLE_CREATE_EVENT_HANDLE_API {
        let dom_event_name: DOMEventName = r#type.to_owned();

        // We cannot support arbitrary native events with eager root listeners
        // because the eager strategy relies on knowing the whole list ahead of time.
        // If we wanted to support this, we'd have to add code to keep track
        // (or search) for all portal and root containers, and lazily add listeners
        // to them whenever we see a previously unknown event. This seems like a lot
        // of complexity for something we don't even have a particular use case for.
        // Unfortunately, the downside of this invariant is that *removing* a native
        // event from the list of known events has now become a breaking change for
        // any code relying on the createEventHandle API.
        if !ALL_NATIVE_EVENTS.with(|all_native_events| all_native_events.has(&dom_event_name)) {
            panic!(
                "Cannot call unstable_createEventHandle with \"{dom_event_name}\", as it is not an event known to React."
            );
        }

        let mut is_capture_phase_listener = false;
        if let Some(options) = options {
            let options_capture = options.capture;
            if let Kind::Bool(options_capture) = js::classify(options_capture) {
                is_capture_phase_listener = options_capture;
            }
        }

        let this_handle: Rc<RefCell<Option<Weak<ReactDOMEventHandle>>>> =
            Rc::new(RefCell::new(None));
        let event_handle: Rc<ReactDOMEventHandle> = Rc::new({
            let this_handle = Rc::clone(&this_handle);
            move |target: &'static Unknown, callback: &'static Unknown| -> Box<dyn Fn()> {
                if js::type_of(Some(callback)) != "function" {
                    panic!(concat!(
                        "ReactDOM.createEventHandle: setter called with an invalid ",
                        "callback. The callback must be a function."
                    ));
                }
                let callback: &'static dyn Fn(&ReactSyntheticEvent) =
                    unsafe { js::cast(Some(callback)) };

                let event_handle = this_handle
                    .borrow()
                    .as_ref()
                    .and_then(Weak::upgrade)
                    .unwrap();
                if !does_target_have_event_handle(target, &*event_handle) {
                    add_event_handle_to_target(target, &*event_handle);
                    register_react_dom_event(target, &dom_event_name, is_capture_phase_listener);
                }
                let listener = create_event_handle_listener(
                    dom_event_name.clone(),
                    is_capture_phase_listener,
                    callback,
                );
                let target_listeners = match get_event_handler_listeners(target) {
                    Some(target_listeners) => target_listeners,
                    None => {
                        let target_listeners = js::set::new(None);
                        set_event_handler_listeners(target, target_listeners);
                        target_listeners
                    }
                };
                target_listeners.add(&listener);
                Box::new(move || {
                    target_listeners.delete(&listener);
                })
            }
        });
        *this_handle.borrow_mut() = Some(Rc::downgrade(&event_handle));

        return Some(event_handle);
    }
    None
}
