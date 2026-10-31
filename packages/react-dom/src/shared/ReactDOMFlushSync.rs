// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;
use react_isomorphic::react_start_transition::{Transition, set_t, t};
use react_reconciler::react_event_priorities::DISCRETE_EVENT_PRIORITY;
use shared::environment::__DEV__;
use shared::react_dom_shared_internals::REACT_DOM_SHARED_INTERNALS;
use shared::react_feature_flags::DISABLE_LEGACY_MODE;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use webapi::console;

/// `flushSync(fn)`: what `fn` gives, or nothing where it's given none.
pub type FlushSync = fn(Option<&dyn Fn() -> Option<&'static Unknown>>) -> Option<&'static Unknown>;

/// What `flushSync` restores when it's done, as upstream's `finally` does.
struct RestoreFlushSync {
    previous_transition: Option<&'static Transition>,
    previous_update_priority: i32,
}

impl Drop for RestoreFlushSync {
    fn drop(&mut self) {
        set_t(REACT_SHARED_INTERNALS, self.previous_transition);
        REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentUpdatePriority */
            .set_p(self.previous_update_priority);
        let was_in_render = (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
            .d()
            .f)(); /* flushSyncWork */
        if __DEV__ && was_in_render.unwrap_or(false) {
            console::error((concat!(
                "flushSync was called from inside a lifecycle method. React cannot ",
                "flush when React is already rendering. Consider moving this call to ",
                "a scheduler task or micro task."
            ),));
        }
    }
}

fn flush_sync_impl(
    r#fn: Option<&dyn Fn() -> Option<&'static Unknown>>,
) -> Option<&'static Unknown> {
    let previous_transition = t(REACT_SHARED_INTERNALS);
    let previous_update_priority = REACT_DOM_SHARED_INTERNALS.p(); /* ReactDOMCurrentUpdatePriority */

    let _restore = RestoreFlushSync {
        previous_transition,
        previous_update_priority,
    };
    set_t(REACT_SHARED_INTERNALS, None);
    REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentUpdatePriority */
        .set_p(i32::from(DISCRETE_EVENT_PRIORITY));
    if let Some(r#fn) = r#fn { r#fn() } else { None }
}

fn flush_sync_error_in_builds_that_support_legacy_mode(
    _fn: Option<&dyn Fn() -> Option<&'static Unknown>>,
) -> Option<&'static Unknown> {
    panic!(
        "Expected this build of React to not support legacy mode but it does. This is a bug in React."
    );
}

#[cfg_attr(rust_js, rust_js::name = "flushSync")]
pub const FLUSH_SYNC: FlushSync = if DISABLE_LEGACY_MODE {
    flush_sync_impl
} else {
    flush_sync_error_in_builds_that_support_legacy_mode
};
