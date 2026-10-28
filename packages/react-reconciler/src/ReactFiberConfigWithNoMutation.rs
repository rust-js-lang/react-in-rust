// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support mutation
// can re-export everything from this module.

use js::Unknown;

fn shim() -> ! {
    panic!(concat!(
        "The current renderer does not support mutation. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// Mutation (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "supportsMutation")]
pub const SUPPORTS_MUTATION: bool = false;
#[cfg_attr(rust_js, rust_js::name = "cloneMutableInstance")]
pub const CLONE_MUTABLE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "cloneMutableTextInstance")]
pub const CLONE_MUTABLE_TEXT_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "appendChild")]
pub const APPEND_CHILD: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "appendChildToContainer")]
pub const APPEND_CHILD_TO_CONTAINER: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "commitTextUpdate")]
pub const COMMIT_TEXT_UPDATE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "commitMount")]
pub const COMMIT_MOUNT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "commitUpdate")]
pub const COMMIT_UPDATE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "insertBefore")]
pub const INSERT_BEFORE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "insertInContainerBefore")]
pub const INSERT_IN_CONTAINER_BEFORE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "removeChild")]
pub const REMOVE_CHILD: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "removeChildFromContainer")]
pub const REMOVE_CHILD_FROM_CONTAINER: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "resetTextContent")]
pub const RESET_TEXT_CONTENT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hideInstance")]
pub const HIDE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hideTextInstance")]
pub const HIDE_TEXT_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "unhideInstance")]
pub const UNHIDE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "unhideTextInstance")]
pub const UNHIDE_TEXT_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "clearContainer")]
pub const CLEAR_CONTAINER: Shim = shim;
pub type GestureTimeline = Unknown;
#[cfg_attr(rust_js, rust_js::name = "getCurrentGestureOffset")]
pub const GET_CURRENT_GESTURE_OFFSET: Shim = shim;
