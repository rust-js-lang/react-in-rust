// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::object::assign;
use react_dom_bindings::react_dom_event_listener::is_enabled;

use crate::react_dom_shared_internals_fb::INTERNALS;

/// The event system's state www's classic code reads.
struct ReactBrowserEventEmitter {
    is_enabled: fn() -> bool,
}

/// The internals www's classic code already reads.
struct ClassicInternals {
    #[cfg_attr(rust_js, rust_js::name = "ReactBrowserEventEmitter")]
    react_browser_event_emitter: ReactBrowserEventEmitter,
}

unsafe impl js::Defined for ClassicInternals {}

js::on_load! {
    // For classic WWW builds, include a few internals that are already in use.
    INTERNALS.with(|internals| {
        assign(
            js::unknown(internals),
            (&ClassicInternals {
                react_browser_event_emitter: ReactBrowserEventEmitter { is_enabled },
            },),
        );
    });
}

pub use crate::react_dom_shared_internals_fb::INTERNALS as __DOM_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE;

pub use crate::react_dom_client_fb::{
    FLUSH_SYNC,
    VERSION,
    browser,
    create_portal,
    preconnect,
    prefetch_dns,
    preinit,
    preinit_module,
    preload,
    preload_module,
    request_form_reset,
    unstable_create_event_handle,
    // DO NOT USE: Temporarily exposed to migrate off of Scheduler.runWithPriority.
    unstable_run_with_priority,
    use_form_state,
    use_form_status,
};

pub use crate::react_dom_root_fb::{
    create_root, find_dom_node, hydrate_root, render, unmount_component_at_node,
    unstable_batched_updates,
};
