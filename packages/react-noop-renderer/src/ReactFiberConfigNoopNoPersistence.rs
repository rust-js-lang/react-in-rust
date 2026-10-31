// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support persistence
// can re-export everything from this module.

fn shim() -> ! {
    panic!(concat!(
        "This entrypoint of react-noop-renderer does not support persistence. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// Persistence (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "supportsPersistence")]
pub const SUPPORTS_PERSISTENCE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "cloneInstance")]
pub const CLONE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "createContainerChildSet")]
pub const CREATE_CONTAINER_CHILD_SET: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "appendChildToContainerChildSet")]
pub const APPEND_CHILD_TO_CONTAINER_CHILD_SET: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "finalizeContainerChildren")]
pub const FINALIZE_CONTAINER_CHILDREN: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "replaceContainerChildren")]
pub const REPLACE_CONTAINER_CHILDREN: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "cloneHiddenInstance")]
pub const CLONE_HIDDEN_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "cloneHiddenTextInstance")]
pub const CLONE_HIDDEN_TEXT_INSTANCE: Shim = shim;
