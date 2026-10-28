// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support microtasks
// can re-export everything from this module.

fn shim() -> ! {
    panic!(concat!(
        "The current renderer does not support microtasks. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// Test selectors (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "supportsMicrotasks")]
pub const SUPPORTS_MICROTASKS: bool = false;
#[cfg_attr(rust_js, rust_js::name = "scheduleMicrotask")]
pub const SCHEDULE_MICROTASK: Shim = shim;
