// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support test selectors
// can re-export everything from this module.

fn shim() -> ! {
    panic!(concat!(
        "The current renderer does not support test selectors. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// Test selectors (when unsupported)
#[cfg_attr(rust_js, rust_js::name = "supportsTestSelectors")]
pub const SUPPORTS_TEST_SELECTORS: bool = false;
#[cfg_attr(rust_js, rust_js::name = "findFiberRoot")]
pub const FIND_FIBER_ROOT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getBoundingRect")]
pub const GET_BOUNDING_RECT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getTextContent")]
pub const GET_TEXT_CONTENT: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "isHiddenSubtree")]
pub const IS_HIDDEN_SUBTREE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "matchAccessibilityRole")]
pub const MATCH_ACCESSIBILITY_ROLE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "setFocusIfFocusable")]
pub const SET_FOCUS_IF_FOCUSABLE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "setupIntersectionObserver")]
pub const SETUP_INTERSECTION_OBSERVER: Shim = shim;
