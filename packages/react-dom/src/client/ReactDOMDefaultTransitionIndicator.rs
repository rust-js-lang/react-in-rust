// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::function;
use webapi::window_global::navigation;
use webapi::{
    ErrorEvent, Event, EventTargetExt, NavigateEvent, NavigationInterceptOptions,
    NavigationNavigateOptions, events,
};

pub fn default_on_default_transition_indicator() -> Option<&'static dyn Fn()> {
    if !js::is_object(navigation) {
        // If the Navigation API is not available, then this is a noop.
        return None;
    }

    let is_cancelled = Rc::new(Cell::new(false));
    let pending_resolve = Rc::new(Cell::new(None));

    let handle_navigate = webapi::listener({
        let pending_resolve = Rc::clone(&pending_resolve);
        move |event: &NavigateEvent| {
            if event.can_intercept() && js::strict_equals(event.info(), "react-transition") {
                let pending_resolve = Rc::clone(&pending_resolve);
                event.intercept(Some(NavigationInterceptOptions {
                    handler: Some(Box::new(move || {
                        let pending_resolve = Rc::clone(&pending_resolve);
                        js::promise::new(move |resolve, _| pending_resolve.set(Some(resolve)))
                    })),
                    focus_reset: Some("manual"),
                    scroll: Some("manual"),
                    ..Default::default()
                }));
            }
        }
    });

    let start_fake_navigation: &'static dyn Fn() = function::with_this({
        let is_cancelled = Rc::clone(&is_cancelled);
        move |_, ()| {
            if is_cancelled.get() {
                // We already stopped this Transition.
                return;
            }
            if navigation().transition().is_some() {
                // There is an on-going Navigation already happening. Let's wait for it to
                // finish before starting our fake one.
                return;
            }
            // Trigger a fake navigation to the same page
            let current_entry = navigation().current_entry();
            if let Some(current_entry) = current_entry
                && let Some(url) = current_entry.url()
            {
                navigation().navigate(
                    &url,
                    Some(NavigationNavigateOptions {
                        state: current_entry
                            .get_state()
                            .as_ref()
                            .map(|state| state as &dyn js::IntoUnknown),
                        info: Some(&"react-transition"), // indicator to routers to ignore this navigation
                        history: Some("replace"),
                    }),
                );
            }
        }
    });

    let handle_navigate_complete: &'static dyn Fn() = function::with_this({
        let is_cancelled = Rc::clone(&is_cancelled);
        let pending_resolve = Rc::clone(&pending_resolve);
        move |_, ()| {
            if let Some(resolve) = pending_resolve.get() {
                // If this was not our navigation completing, we were probably cancelled.
                // We'll start a new one below.
                resolve(());
                pending_resolve.set(None);
            }
            if !is_cancelled.get() {
                // Some other navigation completed but we should still be running.
                // Start another fake one to keep the loading indicator going.
                // There needs to be an async gap to work around https://issues.chromium.org/u/1/issues/419746417.
                js::set_timeout(start_fake_navigation, Some(20.0));
            }
        }
    });

    let on_success = webapi::listener(move |_: &Event| handle_navigate_complete());
    let on_error = webapi::listener(move |_: &ErrorEvent| handle_navigate_complete());

    navigation().add_event_listener(events::Navigate, handle_navigate, None);
    navigation().add_event_listener(events::Navigatesuccess, on_success, None);
    navigation().add_event_listener(events::Navigateerror, on_error, None);

    // Delay the start a bit in case this is a fast Transition.
    js::set_timeout(start_fake_navigation, Some(100.0));

    Some(function::with_this(move |_, ()| {
        is_cancelled.set(true);
        navigation().remove_event_listener(events::Navigate, handle_navigate, None);
        navigation().remove_event_listener(events::Navigatesuccess, on_success, None);
        navigation().remove_event_listener(events::Navigateerror, on_error, None);
        if let Some(resolve) = pending_resolve.get() {
            resolve(());
            pending_resolve.set(None);
        }
    }))
}
