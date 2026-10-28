// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support React Scopes
// can re-export everything from this module.

fn shim() -> ! {
    panic!(concat!(
        "The current renderer does not support React Scopes. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// React Scopes (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "prepareScopeUpdate")]
pub const PREPARE_SCOPE_UPDATE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getInstanceFromScope")]
pub const GET_INSTANCE_FROM_SCOPE: Shim = shim;
