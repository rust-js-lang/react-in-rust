// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Unknown, object};
use react_reconciler::react_fiber_hooks::{Action, ActionStateAndDispatch};
use react_reconciler::react_internal_types::Dispatcher;
use shared::environment::__DEV__;
use shared::react_dom_shared_internals::REACT_DOM_SHARED_INTERNALS;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use webapi::{FormData, HTMLFormElement, console};

/// A form's status: whether it's pending, and if so the data, method and
/// action it's submitted with.
pub struct FormStatus {
    pub pending: bool,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub data: Option<&'static FormData>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub method: Option<String>,
    // A URL, or a function of the form's data.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub action: Option<&'static Unknown>,
}

unsafe impl js::Defined for FormStatus {}

// Since the "not pending" value is always the same, we can reuse the
// same object across all transitions.
#[cfg_attr(rust_js, rust_js::name = "sharedNotPendingObject")]
const SHARED_NOT_PENDING_OBJECT: &FormStatus = &FormStatus {
    pending: false,
    data: None,
    method: None,
    action: None,
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "NotPending")]
    pub static NOT_PENDING: &'static FormStatus = if __DEV__ {
        object::freeze(SHARED_NOT_PENDING_OBJECT)
    } else {
        SHARED_NOT_PENDING_OBJECT
    };
}

fn resolve_dispatcher() -> &'static Dispatcher {
    // Copied from react/src/ReactHooks.js. It's the same thing but in a
    // different package.
    let dispatcher = REACT_SHARED_INTERNALS.h();
    if __DEV__ && dispatcher.is_none() {
        console::error((concat!(
            "Invalid hook call. Hooks can only be called inside of the body of a function component. This could happen for",
            " one of the following reasons:\n",
            "1. You might have mismatching versions of React and the renderer (such as React DOM)\n",
            "2. You might be breaking the Rules of Hooks\n",
            "3. You might have more than one copy of React in the same app\n",
            "See https://react.dev/link/invalid-hook-call for tips about how to debug and fix this problem."
        ),));
    }
    // Will result in a null access error if accessed outside render phase. We
    // intentionally don't throw our own error because this is in a hot path.
    // Also helps ensure this is inlined.
    unsafe { js::cast(dispatcher.map(js::unknown)) }
}

pub fn use_form_status() -> Option<&'static Unknown> {
    let dispatcher = resolve_dispatcher();
    (dispatcher.use_host_transition_status)()
}

pub fn use_form_state(
    action: &'static Action,
    initial_state: Option<&'static Unknown>,
    permalink: Option<String>,
) -> ActionStateAndDispatch {
    let dispatcher = resolve_dispatcher();
    (dispatcher.use_form_state)(action, initial_state, permalink)
}

pub fn request_form_reset(form: &HTMLFormElement) {
    (REACT_DOM_SHARED_INTERNALS
        .d() /* ReactDOMCurrentDispatcher */
        .r)(/* requestFormReset */ form);
}
