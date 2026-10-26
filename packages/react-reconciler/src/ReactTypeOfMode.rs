// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub type TypeOfMode = i32;

#[cfg_attr(rust_js, rust_js::name = "NoMode")]
pub const NO_MODE: TypeOfMode = /*                         */ 0b0000000;
// TODO: Remove ConcurrentMode by reading from the root tag instead
#[cfg_attr(rust_js, rust_js::name = "ConcurrentMode")]
pub const CONCURRENT_MODE: TypeOfMode = /*                 */ 0b0000001;
#[cfg_attr(rust_js, rust_js::name = "ProfileMode")]
pub const PROFILE_MODE: TypeOfMode = /*                    */ 0b0000010;
//export const DebugTracingMode = /*             */ 0b0000100; // Removed
#[cfg_attr(rust_js, rust_js::name = "StrictLegacyMode")]
pub const STRICT_LEGACY_MODE: TypeOfMode = /*               */ 0b0001000;
#[cfg_attr(rust_js, rust_js::name = "StrictEffectsMode")]
pub const STRICT_EFFECTS_MODE: TypeOfMode = /*              */ 0b0010000;
// Keep track of if we're in a SuspenseyImages eligible subtree.
// TODO: Remove this when enableSuspenseyImages ship where it's always on.
#[cfg_attr(rust_js, rust_js::name = "SuspenseyImagesMode")]
pub const SUSPENSEY_IMAGES_MODE: TypeOfMode = /*            */ 0b0100000;
