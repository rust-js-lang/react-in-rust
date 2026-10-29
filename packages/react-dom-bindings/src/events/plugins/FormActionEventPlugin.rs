// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Unknown, object};
use react_reconciler::react_fiber_hooks::{FormAction, start_host_transition};
use react_reconciler::react_fiber_root_scheduler::did_current_event_schedule_transition;
use react_reconciler::react_internal_types::Fiber;
use shared::check_string_coercion::check_attribute_string_coercion;
use shared::environment::__DEV__;
use webapi::{EventTarget, FormData, HTMLFormElement, form_data, submit_event};

use crate::dom_plugin_event_system::{DispatchEntry, DispatchListener, DispatchQueue};
use crate::event_system_flags::EventSystemFlags;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::get_fiber_current_props_from_node;
use crate::react_dom_form_actions::FormStatus;
use crate::react_synthetic_event_type::ReactSyntheticEvent;
use crate::sanitize_url::sanitize_url;
use crate::synthetic_event::SYNTHETIC_EVENT;

#[cfg_attr(rust_js, rust_js::nullable)]
fn coerce_form_action_prop(action_prop: Option<&'static Unknown>) -> Option<&'static Unknown> {
    // This should match the logic in ReactDOMComponent
    if action_prop.is_none()
        || js::type_of(action_prop) == "symbol"
        || js::type_of(action_prop) == "boolean"
    {
        None
    } else if js::type_of(action_prop) == "function" {
        action_prop
    } else {
        if __DEV__ {
            check_attribute_string_coercion(action_prop, "action");
        }
        sanitize_url(action_prop)
    }
}

/// This plugin invokes action functions on forms, inputs and buttons if
/// the form doesn't prevent default.
#[cfg_attr(rust_js, rust_js::nullable(maybe_target_inst, native_event_target))]
pub fn extract_events(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    maybe_target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
    _event_system_flags: EventSystemFlags,
    _target_container: &'static EventTarget,
) {
    if dom_event_name != "submit" {
        return;
    }
    let form_inst = match maybe_target_inst {
        Some(maybe_target_inst)
            if js::strict_equals(maybe_target_inst.state_node.get(), native_event_target) =>
        {
            maybe_target_inst
        }
        _ => {
            // If we're inside a parent root that itself is a parent of this root, then
            // its deepest target won't be the actual form that's being submitted.
            return;
        }
    };
    let form: &'static HTMLFormElement = unsafe { js::cast(native_event_target.map(js::unknown)) };
    let mut action = coerce_form_action_prop(js::get(
        get_fiber_current_props_from_node(js::unknown(form)),
        "action",
    ));
    let mut submitter = submit_event::unchecked_from(native_event).submitter();
    if let Some(submitter_element) = submitter {
        let submitter_props = get_fiber_current_props_from_node(js::unknown(submitter_element));
        let submitter_action = match submitter_props {
            Some(submitter_props) => {
                coerce_form_action_prop(js::get(submitter_props, "formAction"))
            }
            // The built-in Flow type is ?string, wider than the spec
            None => submitter_element
                .get_attribute("formAction")
                .map(js::unknown_of),
        };
        if submitter_action.is_some() {
            // The submitter overrides the form action.
            action = submitter_action;
            // If the action is a function, we don't want to pass its name
            // value to the FormData since it's controlled by the server.
            submitter = None;
        }
    }

    let event: &'static ReactSyntheticEvent = js::new(
        SYNTHETIC_EVENT.with(|ctor| *ctor),
        (
            Some("action".to_owned()),
            "action".to_owned(),
            js::null(),
            native_event,
            native_event_target,
        ),
    );

    let submit_form = move || {
        if native_event.default_prevented() {
            // An earlier event prevented form submission. If a transition update was
            // also scheduled, we should trigger a pending form status — even if
            // no action function was provided.
            if did_current_event_schedule_transition() {
                // We're going to set the pending form status, but because the submission
                // was prevented, we should not fire the action function.
                let form_data = form_data::new(Some(form), submitter);
                let pending_state = js::unknown_of(FormStatus {
                    pending: true,
                    data: Some(form_data),
                    method: Some(form.method()),
                    action,
                });
                if __DEV__ {
                    object::freeze(pending_state);
                }
                start_host_transition(
                    &form_inst,
                    Some(pending_state),
                    // Pass `null` as the action
                    // TODO: Consider splitting up startHostTransition into two separate
                    // functions, one that sets the form status and one that invokes
                    // the action.
                    js::null(),
                    Some(js::unknown(form_data)),
                );
            } else {
                // No earlier event scheduled a transition. Exit without setting a
                // pending form status.
            }
        } else if js::type_of(action) == "function" {
            // A form action was provided. Prevent native navigation.
            (event.prevent_default)();

            // Dispatch the action and set a pending form status.
            let form_data = form_data::new(Some(form), submitter);
            let pending_state = js::unknown_of(FormStatus {
                pending: true,
                data: Some(form_data),
                method: Some(form.method()),
                action,
            });
            if __DEV__ {
                object::freeze(pending_state);
            }
            start_host_transition(
                &form_inst,
                Some(pending_state),
                unsafe { js::cast(action) },
                Some(js::unknown(form_data)),
            );
        } else {
            // No earlier event prevented the default submission, and no action was
            // provided. Exit without setting a pending form status.
        }
    };

    dispatch_queue.push(DispatchEntry {
        event,
        listeners: vec![DispatchListener {
            instance: None,
            listener: js::unknown_of(submit_form),
            current_target: form,
        }],
    });
}

pub fn dispatch_replayed_form_action(
    form_inst: &Rc<Fiber>,
    form: &HTMLFormElement,
    action: &'static FormAction,
    form_data: &'static FormData,
) {
    let pending_state = js::unknown_of(FormStatus {
        pending: true,
        data: Some(form_data),
        method: Some(form.method()),
        action: Some(js::unknown(action)),
    });
    if __DEV__ {
        object::freeze(pending_state);
    }
    start_host_transition(
        form_inst,
        Some(pending_state),
        Some(action),
        Some(js::unknown(form_data)),
    );
}
