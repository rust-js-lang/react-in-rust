// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::environment::__DEV__;
use webapi::console;

use crate::react_hooks::Dispatch;

pub use crate::react_client::ACTIVITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_Activity")]
pub use crate::react_client::ACTIVITY as UNSTABLE_ACTIVITY;
pub use crate::react_client::FRAGMENT;
pub use crate::react_client::OPTIMISTIC_KEY;
pub use crate::react_client::PROFILER;
pub use crate::react_client::STRICT_MODE;
pub use crate::react_client::SUSPENSE;
pub use crate::react_client::UNSTABLE_SUSPENSE_LIST;
pub use crate::react_client::VERSION;
pub use crate::react_client::VIEW_TRANSITION;
pub use crate::react_client::{
    __CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE, __COMPILER_RUNTIME, CACHE,
    CACHE_SIGNAL, CHILDREN, Component, PureComponent, add_transition_type, clone_element,
    create_context, create_element, create_ref, forward_ref, is_valid_element, lazy, memo,
    start_transition, unstable_get_cache_for_type, unstable_start_gesture_transition,
    unstable_use_cache_refresh, r#use, use_action_state, use_callback, use_context,
    use_debug_value, use_deferred_value, use_effect, use_effect_event, use_id,
    use_imperative_handle, use_insertion_effect, use_layout_effect, use_memo, use_optimistic,
    use_reducer, use_ref, use_state, use_sync_external_store, use_transition,
};

#[cfg_attr(rust_js, rust_js::name = "experimental_useOptimistic")]
pub fn experimental_use_optimistic<S, A>(
    passthrough: S,
    reducer: Option<&'static dyn Fn(S, A) -> S>,
) -> (S, &'static Dispatch<A>) {
    if __DEV__ {
        console::error((concat!(
            "useOptimistic is now in canary. Remove the experimental_ prefix. ",
            "The prefixed alias will be removed in an upcoming release."
        ),));
    }
    use_optimistic(passthrough, reducer)
}
