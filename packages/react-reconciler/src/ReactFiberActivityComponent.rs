// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};

use js::Unknown;

use crate::react_fiber_hydration_context::HydrationErrors;
use crate::react_fiber_lane::Lane;
use crate::react_fiber_tree_context::TreeContext;

// A non-null ActivityState represents a dehydrated Activity boundary.
pub struct ActivityState {
    // ReactFiberConfig's `ActivityInstance`.
    pub dehydrated: &'static Unknown,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub tree_context: Option<TreeContext>,
    // Represents the lane we should attempt to hydrate a dehydrated boundary at.
    // OffscreenLane is the default for dehydrated boundaries.
    // NoLane is the default for normal boundaries, which turns into "normal" pri.
    pub retry_lane: Cell<Lane>,
    // Stashed Errors that happened while attempting to hydrate this boundary.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub hydration_errors: RefCell<Option<HydrationErrors>>,
}

unsafe impl js::Defined for ActivityState {}
