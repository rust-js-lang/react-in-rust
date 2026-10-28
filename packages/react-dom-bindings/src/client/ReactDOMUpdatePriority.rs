// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::convert::Infallible;

use react_reconciler::react_event_priorities::{
    DEFAULT_EVENT_PRIORITY, EventPriority, NO_EVENT_PRIORITY, event_priority_to_lane,
};
use shared::react_dom_shared_internals::REACT_DOM_SHARED_INTERNALS;
use webapi::window;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/events/ReactDOMEventListener.js#getEventPriority"
)]
fn get_event_priority(dom_event_name: &str) -> EventPriority {
    let _ = dom_event_name;
    unreachable!()
}

pub fn set_current_update_priority(
    new_priority: EventPriority,
    // Closure will consistently not inline this function when it has arity 1
    // however when it has arity 2 even if the second arg is omitted at every
    // callsite it seems to inline it even when the internal length of the function
    // is much longer. I hope this is consistent enough to rely on across builds
    #[cfg_attr(rust_js, rust_js::name = "IntentionallyUnusedArgument")]
    _intentionally_unused_argument: Option<Infallible>,
) {
    REACT_DOM_SHARED_INTERNALS.set_p(event_priority_to_lane(new_priority)); /* currentUpdatePriority */
}

pub fn get_current_update_priority() -> EventPriority {
    EventPriority::from(REACT_DOM_SHARED_INTERNALS.p()) /* currentUpdatePriority */
}

pub fn resolve_update_priority() -> EventPriority {
    let update_priority = EventPriority::from(REACT_DOM_SHARED_INTERNALS.p()); /* currentUpdatePriority */
    if update_priority != NO_EVENT_PRIORITY {
        return update_priority;
    }
    let Some(current_event) = window.event() else {
        return DEFAULT_EVENT_PRIORITY;
    };
    get_event_priority(&current_event.r#type())
}

/// What sets the update priority back however the work it was set for is left.
struct RestorePriority(EventPriority);

impl Drop for RestorePriority {
    fn drop(&mut self) {
        set_current_update_priority(self.0, None);
    }
}

pub fn run_with_priority<T>(priority: EventPriority, r#fn: impl FnOnce() -> T) -> T {
    let previous_priority = get_current_update_priority();
    let _restore = RestorePriority(previous_priority);
    set_current_update_priority(priority, None);
    r#fn()
}
