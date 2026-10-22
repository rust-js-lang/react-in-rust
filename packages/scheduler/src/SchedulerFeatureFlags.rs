// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

#[cfg_attr(rust_js, rust_js::name = "enableProfiling")]
pub const ENABLE_PROFILING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "frameYieldMs")]
pub const FRAME_YIELD_MS: f64 = 5.0;

#[cfg_attr(rust_js, rust_js::name = "userBlockingPriorityTimeout")]
pub const USER_BLOCKING_PRIORITY_TIMEOUT: f64 = 250.0;
#[cfg_attr(rust_js, rust_js::name = "normalPriorityTimeout")]
pub const NORMAL_PRIORITY_TIMEOUT: f64 = 5000.0;
#[cfg_attr(rust_js, rust_js::name = "lowPriorityTimeout")]
pub const LOW_PRIORITY_TIMEOUT: f64 = 10000.0;
#[cfg_attr(rust_js, rust_js::name = "enableRequestPaint")]
pub const ENABLE_REQUEST_PAINT: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableAlwaysYieldScheduler")]
pub const ENABLE_ALWAYS_YIELD_SCHEDULER: bool = cfg!(feature = "experimental");
