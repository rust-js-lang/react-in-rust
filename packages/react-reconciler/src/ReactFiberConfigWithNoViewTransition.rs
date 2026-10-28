// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support view transitions
// can re-export everything from this module.

use js::Unknown;

fn shim() -> ! {
    panic!(concat!(
        "The current renderer does not support view transitions. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// View Transitions (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "applyViewTransitionName")]
pub const APPLY_VIEW_TRANSITION_NAME: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "restoreViewTransitionName")]
pub const RESTORE_VIEW_TRANSITION_NAME: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "cancelViewTransitionName")]
pub const CANCEL_VIEW_TRANSITION_NAME: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "cancelRootViewTransitionName")]
pub const CANCEL_ROOT_VIEW_TRANSITION_NAME: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "restoreRootViewTransitionName")]
pub const RESTORE_ROOT_VIEW_TRANSITION_NAME: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "cloneRootViewTransitionContainer")]
pub const CLONE_ROOT_VIEW_TRANSITION_CONTAINER: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "removeRootViewTransitionClone")]
pub const REMOVE_ROOT_VIEW_TRANSITION_CLONE: Shim = shim;
pub type InstanceMeasurement = Unknown;
#[cfg_attr(rust_js, rust_js::name = "measureInstance")]
pub const MEASURE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "measureClonedInstance")]
pub const MEASURE_CLONED_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "wasInstanceInViewport")]
pub const WAS_INSTANCE_IN_VIEWPORT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hasInstanceChanged")]
pub const HAS_INSTANCE_CHANGED: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hasInstanceAffectedParent")]
pub const HAS_INSTANCE_AFFECTED_PARENT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "startViewTransition")]
pub const START_VIEW_TRANSITION: Shim = shim;
pub type RunningViewTransition = Unknown;
#[cfg_attr(rust_js, rust_js::name = "startGestureTransition")]
pub const START_GESTURE_TRANSITION: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "stopViewTransition")]
pub const STOP_VIEW_TRANSITION: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "addViewTransitionFinishedListener")]
pub const ADD_VIEW_TRANSITION_FINISHED_LISTENER: Shim = shim;
pub type ViewTransitionInstance = Unknown;
#[cfg_attr(rust_js, rust_js::name = "createViewTransitionInstance")]
pub const CREATE_VIEW_TRANSITION_INSTANCE: Shim = shim;
