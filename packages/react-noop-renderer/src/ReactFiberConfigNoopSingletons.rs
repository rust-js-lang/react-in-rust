// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support mutation
// can re-export everything from this module.

fn shim() -> ! {
    panic!(concat!(
        "react-noop-renderer does not support Singletons. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// Resources (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "supportsSingletons")]
pub const SUPPORTS_SINGLETONS: bool = false;
#[cfg_attr(rust_js, rust_js::name = "resolveSingletonInstance")]
pub const RESOLVE_SINGLETON_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "acquireSingletonInstance")]
pub const ACQUIRE_SINGLETON_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "releaseSingletonInstance")]
pub const RELEASE_SINGLETON_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "isHostSingletonType")]
pub const IS_HOST_SINGLETON_TYPE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "isSingletonScope")]
pub const IS_SINGLETON_SCOPE: Shim = shim;
