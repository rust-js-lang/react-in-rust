// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

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
    unstable_batched_updates,
    unstable_create_event_handle,
    // DO NOT USE: Temporarily exposed to migrate off of Scheduler.runWithPriority.
    unstable_run_with_priority,
    use_form_state,
    use_form_status,
};

pub use crate::react_dom_root_fb::{create_root, hydrate_root};
