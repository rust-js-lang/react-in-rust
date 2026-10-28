// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub type EventSystemFlags = i32;

pub const IS_EVENT_HANDLE_NON_MANAGED_NODE: i32 = 1;
pub const IS_NON_DELEGATED: i32 = 1 << 1;
pub const IS_CAPTURE_PHASE: i32 = 1 << 2;
pub const IS_PASSIVE: i32 = 1 << 3;
pub const IS_LEGACY_FB_SUPPORT_MODE: i32 = 1 << 4;

pub const SHOULD_NOT_DEFER_CLICK_FOR_FB_SUPPORT_MODE: i32 =
    IS_LEGACY_FB_SUPPORT_MODE | IS_CAPTURE_PHASE;

// We do not want to defer if the event system has already been
// set to LEGACY_FB_SUPPORT. LEGACY_FB_SUPPORT only gets set when
// we call willDeferLaterForLegacyFBSupport, thus not bailing out
// will result in endless cycles like an infinite loop.
// We also don't want to defer during event replaying.
pub const SHOULD_NOT_PROCESS_POLYFILL_EVENT_PLUGINS: i32 =
    IS_EVENT_HANDLE_NON_MANAGED_NODE | IS_NON_DELEGATED | IS_CAPTURE_PHASE;
