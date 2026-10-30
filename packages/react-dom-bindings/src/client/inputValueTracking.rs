// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use js::{IntoUnknown, PropertyDescriptor, Unknown, function, object};
use shared::check_string_coercion::check_form_field_value_string_coercion;
use shared::environment::__DEV__;
use webapi::HTMLInputElement;

/// What an input's value was when React last saw it, kept on the node as
/// its `_valueTracker`.
pub struct ValueTracker {
    pub get_value: Box<dyn Fn() -> String>,
    pub set_value: Box<dyn Fn(&str)>,
    pub stop_tracking: Box<dyn Fn()>,
}

fn is_checkable(elem: &HTMLInputElement) -> bool {
    let r#type = elem.r#type();
    let node_name = elem.node_name();
    !node_name.is_empty()
        && node_name.to_lowercase() == "input"
        && (r#type == "checkbox" || r#type == "radio")
}

fn get_tracker(node: &HTMLInputElement) -> Option<&'static ValueTracker> {
    unsafe { js::cast(js::get(node, "_valueTracker")) }
}

fn detach_tracker(node: &HTMLInputElement) {
    js::set(node, "_valueTracker", js::null::<&ValueTracker>());
}

fn get_value_from_node(node: Option<&HTMLInputElement>) -> String {
    let mut value = String::new();
    let Some(node) = node else {
        return value;
    };

    if is_checkable(node) {
        value = if node.checked() { "true" } else { "false" }.to_owned();
    } else {
        value = node.value();
    }

    value
}

fn track_value_on_node(
    node: &'static HTMLInputElement,
    value_field: &'static str,
    current_value: String,
) -> Option<ValueTracker> {
    let descriptor = object::get_own_property_descriptor(
        js::get(js::get(node, "constructor"), "prototype"),
        value_field,
    );

    // if someone has already defined a value or Safari, then bail
    // and don't track value will cause over reporting of changes,
    // but it's better then a hard failure
    // (needed for certain tests that spyOn input values and Safari)
    if js::unknown(node).has_own_property(value_field) {
        return None;
    }
    let descriptor = descriptor?;
    let (Some(get), Some(set)) = (descriptor.get, descriptor.set) else {
        return None;
    };
    let current_value = Rc::new(RefCell::new(current_value));
    let tracked_value = Rc::clone(&current_value);
    object::define_property(
        node,
        value_field,
        &PropertyDescriptor {
            configurable: Some(true),
            // $FlowFixMe[missing-this-annot]
            get: Some(function::with_this(move |this, ()| {
                function::call(get, this.map(|this| this as &dyn IntoUnknown), ())
            })),
            // $FlowFixMe[missing-local-annot]
            // $FlowFixMe[missing-this-annot]
            set: Some(function::with_this(
                move |this, (value,): (Option<&'static Unknown>,)| {
                    if __DEV__ {
                        check_form_field_value_string_coercion(value);
                    }
                    *tracked_value.borrow_mut() = js::concat_text(value).unwrap();
                    function::call(set, this.map(|this| this as &dyn IntoUnknown), (value,));
                },
            )),
            ..Default::default()
        },
    );
    // We could've passed this the first time
    // but it triggers a bug in IE11 and Edge 14/15.
    // Calling defineProperty() again should be equivalent.
    // https://github.com/react/react/issues/11768
    object::define_property(
        node,
        value_field,
        &PropertyDescriptor {
            enumerable: descriptor.enumerable,
            ..Default::default()
        },
    );

    let tracker = ValueTracker {
        get_value: Box::new({
            let current_value = Rc::clone(&current_value);
            move || current_value.borrow().clone()
        }),
        set_value: Box::new({
            let current_value = Rc::clone(&current_value);
            move |value: &str| {
                if __DEV__ {
                    check_form_field_value_string_coercion(Some(js::unknown(value)));
                }
                *current_value.borrow_mut() = js::concat_text(value);
            }
        }),
        stop_tracking: Box::new(move || {
            detach_tracker(node);
            js::delete(node, value_field);
        }),
    };
    Some(tracker)
}

pub fn track(node: &'static HTMLInputElement) {
    if get_tracker(node).is_some() {
        return;
    }

    let value_field = if is_checkable(node) {
        "checked"
    } else {
        "value"
    };
    // This is read from the DOM so always safe to coerce. We really shouldn't
    // be coercing to a string at all. It's just historical.
    // eslint-disable-next-line react-internal/safe-string-coercion
    let initial_value = js::concat_text(js::get(node, value_field)).unwrap();
    js::set(
        node,
        "_valueTracker",
        track_value_on_node(node, value_field, initial_value),
    );
}

pub fn track_hydrated(
    node: &'static HTMLInputElement,
    initial_value: &str,
    initial_checked: bool,
) -> bool {
    // For hydration, the initial value is not the current value but the value
    // that we last observed which is what the initial server render was.
    if get_tracker(node).is_some() {
        return false;
    }

    let (value_field, expected_value) = if is_checkable(node) {
        // eslint-disable-next-line react-internal/safe-string-coercion
        ("checked", initial_checked.to_string())
    } else {
        ("value", initial_value.to_owned())
    };
    let current_value =
        // eslint-disable-next-line react-internal/safe-string-coercion
        js::concat_text(js::get(node, value_field)).unwrap();
    js::set(
        node,
        "_valueTracker",
        track_value_on_node(node, value_field, expected_value.clone()),
    );
    current_value != expected_value
}

pub fn update_value_if_changed(node: Option<&'static HTMLInputElement>) -> bool {
    let Some(node) = node else {
        return false;
    };

    let tracker = get_tracker(node);
    // if there is no tracker at this point it's unlikely
    // that trying again will succeed
    let Some(tracker) = tracker else {
        return true;
    };

    let last_value = (tracker.get_value)();
    let next_value = get_value_from_node(Some(node));
    if next_value != last_value {
        (tracker.set_value)(&next_value);
        return true;
    }
    false
}

pub fn stop_tracking(node: &HTMLInputElement) {
    let tracker = get_tracker(node);
    if let Some(tracker) = tracker {
        (tracker.stop_tracking)();
    }
}
