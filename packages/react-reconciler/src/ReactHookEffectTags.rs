// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub type HookFlags = i32;

#[cfg_attr(rust_js, rust_js::name = "NoFlags")]
pub const NO_FLAGS: HookFlags = /*   */ 0b0000;

// Represents whether effect should fire.
#[cfg_attr(rust_js, rust_js::name = "HasEffect")]
pub const HAS_EFFECT: HookFlags = /* */ 0b0001;

// Represents the phase in which the effect (not the clean-up) fires.
#[cfg_attr(rust_js, rust_js::name = "Insertion")]
pub const INSERTION: HookFlags = /* */ 0b0010;
#[cfg_attr(rust_js, rust_js::name = "Layout")]
pub const LAYOUT: HookFlags = /*    */ 0b0100;
#[cfg_attr(rust_js, rust_js::name = "Passive")]
pub const PASSIVE: HookFlags = /*   */ 0b1000;
