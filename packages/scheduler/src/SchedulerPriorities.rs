// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub type PriorityLevel = u8;

// TODO: Use symbols?
#[cfg_attr(rust_js, rust_js::name = "NoPriority")]
pub const NO_PRIORITY: PriorityLevel = 0;
#[cfg_attr(rust_js, rust_js::name = "ImmediatePriority")]
pub const IMMEDIATE_PRIORITY: PriorityLevel = 1;
#[cfg_attr(rust_js, rust_js::name = "UserBlockingPriority")]
pub const USER_BLOCKING_PRIORITY: PriorityLevel = 2;
#[cfg_attr(rust_js, rust_js::name = "NormalPriority")]
pub const NORMAL_PRIORITY: PriorityLevel = 3;
#[cfg_attr(rust_js, rust_js::name = "LowPriority")]
pub const LOW_PRIORITY: PriorityLevel = 4;
#[cfg_attr(rust_js, rust_js::name = "IdlePriority")]
pub const IDLE_PRIORITY: PriorityLevel = 5;
