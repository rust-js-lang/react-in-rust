// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};

use js::Unknown;

use crate::react_fiber_lane::Lane;

// A non-null ActivityState represents a dehydrated Activity boundary.
pub struct ActivityState {
    // ReactFiberConfig's `ActivityInstance`.
    pub dehydrated: &'static Unknown,
    // ReactFiberTreeContext's `TreeContext`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub tree_context: Option<&'static Unknown>,
    // Represents the lane we should attempt to hydrate a dehydrated boundary at.
    // OffscreenLane is the default for dehydrated boundaries.
    // NoLane is the default for normal boundaries, which turns into "normal" pri.
    pub retry_lane: Cell<Lane>,
    // Stashed Errors that happened while attempting to hydrate this boundary.
    // ReactCapturedValue's `CapturedValue<mixed>`s.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub hydration_errors: RefCell<Option<Vec<&'static Unknown>>>,
}
