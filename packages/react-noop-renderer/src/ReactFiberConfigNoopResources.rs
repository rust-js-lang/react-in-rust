// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support hydration
// can re-export everything from this module.

use js::Unknown;

fn shim() -> ! {
    panic!(concat!(
        "react-noop-renderer does not support Resources. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

pub type HoistableRoot = Unknown;
pub type Resource = Unknown;

// Resources (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "supportsResources")]
pub const SUPPORTS_RESOURCES: bool = false;
#[cfg_attr(rust_js, rust_js::name = "isHostHoistableType")]
pub const IS_HOST_HOISTABLE_TYPE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getHoistableRoot")]
pub const GET_HOISTABLE_ROOT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getResource")]
pub const GET_RESOURCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "acquireResource")]
pub const ACQUIRE_RESOURCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "releaseResource")]
pub const RELEASE_RESOURCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hydrateHoistable")]
pub const HYDRATE_HOISTABLE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "mountHoistable")]
pub const MOUNT_HOISTABLE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "unmountHoistable")]
pub const UNMOUNT_HOISTABLE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "createHoistableInstance")]
pub const CREATE_HOISTABLE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "prepareToCommitHoistables")]
pub const PREPARE_TO_COMMIT_HOISTABLES: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "mayResourceSuspendCommit")]
pub const MAY_RESOURCE_SUSPEND_COMMIT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "preloadResource")]
pub const PRELOAD_RESOURCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "suspendResource")]
pub const SUSPEND_RESOURCE: Shim = shim;
