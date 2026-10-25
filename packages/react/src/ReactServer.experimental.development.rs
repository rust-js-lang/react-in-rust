// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

#[cfg_attr(
    rust_js,
    rust_js::name = "__SERVER_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"
)]
pub use crate::react_shared_internals_server::REACT_SHARED_INTERNALS as __SERVER_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE;

use crate::react_children::{count, for_each, map, only, to_array};
use crate::react_client::Children;

// These are server-only
#[cfg_attr(rust_js, rust_js::name = "experimental_taintObjectReference")]
pub use crate::react_taint::taint_object_reference as experimental_taint_object_reference;
#[cfg_attr(rust_js, rust_js::name = "experimental_taintUniqueValue")]
pub use crate::react_taint::taint_unique_value as experimental_taint_unique_value;

pub use crate::react_cache_server::{cache, cache_signal};
pub use crate::react_create_ref::create_ref;
pub use crate::react_forward_ref::forward_ref;
#[cfg_attr(rust_js, rust_js::name = "unstable_getCacheForType")]
pub use crate::react_hooks::get_cache_for_type as unstable_get_cache_for_type;
pub use crate::react_hooks::{r#use, use_callback, use_debug_value, use_id, use_memo};
pub use crate::react_jsx_element::{clone_element, create_element, is_valid_element};
pub use crate::react_lazy::lazy;
pub use crate::react_memo::memo;
pub use crate::react_start_transition::start_transition;
#[cfg_attr(rust_js, rust_js::name = "Activity")]
pub use shared::react_symbols::REACT_ACTIVITY_TYPE as ACTIVITY;
#[cfg_attr(rust_js, rust_js::name = "Fragment")]
pub use shared::react_symbols::REACT_FRAGMENT_TYPE as FRAGMENT;
#[cfg_attr(rust_js, rust_js::name = "Profiler")]
pub use shared::react_symbols::REACT_PROFILER_TYPE as PROFILER;
#[cfg_attr(rust_js, rust_js::name = "StrictMode")]
pub use shared::react_symbols::REACT_STRICT_MODE_TYPE as STRICT_MODE;
#[cfg_attr(rust_js, rust_js::name = "Suspense")]
pub use shared::react_symbols::REACT_SUSPENSE_TYPE as SUSPENSE;
#[cfg_attr(rust_js, rust_js::name = "ViewTransition")]
pub use shared::react_symbols::REACT_VIEW_TRANSITION_TYPE as VIEW_TRANSITION;
#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;
// Experimental
#[cfg_attr(rust_js, rust_js::name = "unstable_SuspenseList")]
pub use shared::react_symbols::REACT_SUSPENSE_LIST_TYPE as UNSTABLE_SUSPENSE_LIST;
// enableOptimisticKey
#[cfg_attr(rust_js, rust_js::name = "optimisticKey")]
pub use shared::react_symbols::REACT_OPTIMISTIC_KEY as OPTIMISTIC_KEY;
// DEV-only
pub use crate::react_owner_stack::capture_owner_stack;

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
