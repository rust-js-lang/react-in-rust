// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

use crate::react_children::{count, for_each, map, only, to_array};

pub use crate::react_base_classes::{Component, PureComponent};
pub use crate::react_cache_client::{CACHE, CACHE_SIGNAL};
#[cfg_attr(rust_js, rust_js::name = "__COMPILER_RUNTIME")]
pub use crate::react_compiler_runtime as __COMPILER_RUNTIME;
pub use crate::react_context::create_context;
pub use crate::react_create_ref::create_ref;
pub use crate::react_forward_ref::forward_ref;
pub use crate::react_hooks::{
    use_action_state, use_callback, use_context, use_debug_value, use_effect, use_effect_event,
    use_imperative_handle, use_insertion_effect, use_layout_effect, use_memo, use_optimistic,
    use_reducer, use_ref, use_state, use_sync_external_store,
};
pub use crate::react_jsx_element::{clone_element, create_element, is_valid_element};
pub use crate::react_lazy::lazy;
pub use crate::react_memo::memo;
#[cfg_attr(
    rust_js,
    rust_js::name = "__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"
)]
pub use crate::react_shared_internals_client::REACT_SHARED_INTERNALS as __CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE;
#[cfg_attr(rust_js, rust_js::name = "Fragment")]
pub use shared::react_symbols::REACT_FRAGMENT_TYPE as FRAGMENT;
#[cfg_attr(rust_js, rust_js::name = "Profiler")]
pub use shared::react_symbols::REACT_PROFILER_TYPE as PROFILER;
#[cfg_attr(rust_js, rust_js::name = "StrictMode")]
pub use shared::react_symbols::REACT_STRICT_MODE_TYPE as STRICT_MODE;
#[cfg_attr(rust_js, rust_js::name = "Suspense")]
pub use shared::react_symbols::REACT_SUSPENSE_TYPE as SUSPENSE;
#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;
// Concurrent Mode
#[cfg_attr(rust_js, rust_js::name = "unstable_getCacheForType")]
pub use crate::react_hooks::get_cache_for_type as unstable_get_cache_for_type;
#[cfg_attr(rust_js, rust_js::name = "unstable_useCacheRefresh")]
pub use crate::react_hooks::use_cache_refresh as unstable_use_cache_refresh;
pub use crate::react_hooks::{r#use, use_deferred_value, use_transition};
pub use crate::react_start_transition::start_transition;
#[cfg_attr(rust_js, rust_js::name = "Activity")]
pub use shared::react_symbols::REACT_ACTIVITY_TYPE as ACTIVITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_LegacyHidden")]
pub use shared::react_symbols::REACT_LEGACY_HIDDEN_TYPE as UNSTABLE_LEGACY_HIDDEN;
#[cfg_attr(rust_js, rust_js::name = "unstable_SuspenseList")]
pub use shared::react_symbols::REACT_SUSPENSE_LIST_TYPE as UNSTABLE_SUSPENSE_LIST;
// enableScopeAPI
#[cfg_attr(rust_js, rust_js::name = "unstable_Scope")]
pub use shared::react_symbols::REACT_SCOPE_TYPE as UNSTABLE_SCOPE;
// enableTransitionTracing
#[cfg_attr(rust_js, rust_js::name = "unstable_TracingMarker")]
pub use shared::react_symbols::REACT_TRACING_MARKER_TYPE as UNSTABLE_TRACING_MARKER;
// enableViewTransition
pub use crate::react_transition_type::add_transition_type;
#[cfg_attr(rust_js, rust_js::name = "ViewTransition")]
pub use shared::react_symbols::REACT_VIEW_TRANSITION_TYPE as VIEW_TRANSITION;
// enableGestureTransition
#[cfg_attr(rust_js, rust_js::name = "unstable_startGestureTransition")]
pub use crate::react_start_transition::start_gesture_transition as unstable_start_gesture_transition;
// enableOptimisticKey
#[cfg_attr(rust_js, rust_js::name = "optimisticKey")]
pub use shared::react_symbols::REACT_OPTIMISTIC_KEY as OPTIMISTIC_KEY;
// DEV-only
pub use crate::react_act::act;
pub use crate::react_hooks::use_id;
pub use crate::react_owner_stack::capture_owner_stack;

/// `React.Children.map`.
type MapChildren = fn(
    Option<&'static Unknown>,
    &dyn Fn(Option<&'static Unknown>, u32) -> Option<&'static Unknown>,
    Option<&'static Unknown>,
) -> Option<Vec<Option<&'static Unknown>>>;

/// `React.Children.forEach`.
type ForEachChildren = fn(
    Option<&'static Unknown>,
    &(dyn Fn(Option<&'static Unknown>) + 'static),
    Option<&'static Unknown>,
);

/// `React.Children`: the functions that deal with `props.children`.
pub struct Children {
    pub map: MapChildren,
    pub for_each: ForEachChildren,
    pub count: fn(Option<&'static Unknown>) -> u32,
    pub to_array: fn(Option<&'static Unknown>) -> Vec<Option<&'static Unknown>>,
    pub only: fn(Option<&'static Unknown>) -> Option<&'static Unknown>,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "Children")]
    pub static CHILDREN: Children = Children {
        map,
        for_each,
        count,
        to_array,
        only,
    };
}
