// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::environment::__DEV__;

use crate::react_client::capture_owner_stack as capture_owner_stack_impl;

#[cfg_attr(rust_js, rust_js::name = "unstable_Activity")]
pub use crate::react_client::ACTIVITY as UNSTABLE_ACTIVITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_ViewTransition")]
pub use crate::react_client::VIEW_TRANSITION as UNSTABLE_VIEW_TRANSITION;
#[cfg_attr(rust_js, rust_js::name = "unstable_addTransitionType")]
pub use crate::react_client::add_transition_type as unstable_add_transition_type;
#[cfg_attr(rust_js, rust_js::name = "experimental_useEffectEvent")]
pub use crate::react_client::use_effect_event as experimental_use_effect_event;
pub use crate::react_client::{
    __CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE, __COMPILER_RUNTIME, ACTIVITY,
    CACHE, CACHE_SIGNAL, CHILDREN, Component, FRAGMENT, PROFILER, PureComponent, STRICT_MODE,
    SUSPENSE, UNSTABLE_LEGACY_HIDDEN, UNSTABLE_SCOPE, UNSTABLE_SUSPENSE_LIST,
    UNSTABLE_TRACING_MARKER, VERSION, VIEW_TRANSITION, act, add_transition_type, clone_element,
    create_context, create_element, create_ref, forward_ref, is_valid_element, lazy, memo,
    start_transition, unstable_get_cache_for_type, unstable_use_cache_refresh, r#use,
    use_action_state, use_callback, use_context, use_debug_value, use_deferred_value, use_effect,
    use_effect_event, use_id, use_imperative_handle, use_insertion_effect, use_layout_effect,
    use_memo, use_optimistic, use_reducer, use_ref, use_state, use_sync_external_store,
    use_transition,
};

pub use crate::react_jsx::{JSX, JSX_DEV, JSXS};

// export for backwards compatibility during upgrade
#[cfg_attr(rust_js, rust_js::name = "unstable_useMemoCache")]
pub use crate::react_hooks::use_memo_cache as unstable_use_memo_cache;

// export to match the name of the OSS function typically exported from
// react/compiler-runtime
pub use crate::react_hooks::use_memo_cache as c;

thread_local! {
    // Only export captureOwnerStack in development.
    pub static CAPTURE_OWNER_STACK: Option<fn() -> Option<String>> = if __DEV__ {
        Some(capture_owner_stack_impl)
    } else {
        None
    };
}
