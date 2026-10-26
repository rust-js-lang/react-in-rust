// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub type RootTag = u8;

#[cfg_attr(rust_js, rust_js::name = "LegacyRoot")]
pub const LEGACY_ROOT: RootTag = 0;
#[cfg_attr(rust_js, rust_js::name = "ConcurrentRoot")]
pub const CONCURRENT_ROOT: RootTag = 1;
