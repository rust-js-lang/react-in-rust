// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// These are semi-public constants exposed to any third-party renderers.
// Only expose the minimal subset necessary to implement a host config.

pub use crate::react_event_priorities::{
    CONTINUOUS_EVENT_PRIORITY, DEFAULT_EVENT_PRIORITY, DISCRETE_EVENT_PRIORITY,
    IDLE_EVENT_PRIORITY, NO_EVENT_PRIORITY,
};
pub use crate::react_root_tags::{CONCURRENT_ROOT, LEGACY_ROOT};
