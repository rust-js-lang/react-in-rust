// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use js::{Dict, dict};
use shared::environment::__DEV__;
use shared::react_feature_flags::ENABLE_CREATE_EVENT_HANDLE_API;
use webapi::console;

use crate::dom_event_names::DOMEventName;

thread_local! {
    pub static ALL_NATIVE_EVENTS: RefCell<HashSet<DOMEventName>> = RefCell::new(HashSet::new());
}

js::on_load! {
    if ENABLE_CREATE_EVENT_HANDLE_API {
        ALL_NATIVE_EVENTS.with_borrow_mut(|all_native_events| {
            all_native_events.insert("beforeblur".to_owned());
            all_native_events.insert("afterblur".to_owned());
        });
    }
}

thread_local! {
    /// Mapping from registration name to event name
    pub static REGISTRATION_NAME_DEPENDENCIES: &'static Dict<Rc<Vec<DOMEventName>>> = dict::new();

    /// Mapping from lowercase registration names to the properly cased version,
    /// used to warn in the case of missing event handlers. Available
    /// only in __DEV__.
    /// @type {Object}
    pub static POSSIBLE_REGISTRATION_NAMES: Option<&'static Dict<String>> = if __DEV__ { Some(dict::new()) } else { js::null() };
}
// Trust the developer to only use possibleRegistrationNames in __DEV__

pub fn register_two_phase_event(registration_name: &str, dependencies: Rc<Vec<DOMEventName>>) {
    register_direct_event(registration_name, Rc::clone(&dependencies));
    register_direct_event(&(registration_name.to_owned() + "Capture"), dependencies);
}

pub fn register_direct_event(registration_name: &str, dependencies: Rc<Vec<DOMEventName>>) {
    if __DEV__
        && REGISTRATION_NAME_DEPENDENCIES.with(|registration_name_dependencies| {
            dict::get(registration_name_dependencies, registration_name).is_some()
        })
    {
        console::error((
            concat!(
                "EventRegistry: More than one plugin attempted to publish the same ",
                "registration name, `%s`."
            ),
            registration_name,
        ));
    }

    REGISTRATION_NAME_DEPENDENCIES.with(|registration_name_dependencies| {
        dict::set(
            registration_name_dependencies,
            registration_name,
            Rc::clone(&dependencies),
        )
    });

    if __DEV__
        && let Some(possible_registration_names) = POSSIBLE_REGISTRATION_NAMES.with(|names| *names)
    {
        let lower_cased_name = registration_name.to_lowercase();
        dict::set(
            possible_registration_names,
            &lower_cased_name,
            registration_name.to_owned(),
        );

        if registration_name == "onDoubleClick" {
            dict::set(
                possible_registration_names,
                "ondblclick",
                registration_name.to_owned(),
            );
        }
    }

    ALL_NATIVE_EVENTS.with_borrow_mut(|all_native_events| {
        for dependency in dependencies.iter() {
            all_native_events.insert(dependency.clone());
        }
    });
}
