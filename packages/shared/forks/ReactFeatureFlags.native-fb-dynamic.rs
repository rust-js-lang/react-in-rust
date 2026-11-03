// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::environment::__VARIANT__;

// In xplat, these flags are controlled by GKs. Because most GKs have some
// population running in either mode, we should run our tests that way, too,
//
// Use __VARIANT__ to simulate a GK. The tests will be run twice: once
// with the __VARIANT__ set to `true`, and once set to `false`.
//
// TODO: __VARIANT__ isn't supported for React Native flags yet. You can set the
// flag here but it won't be set to `true` in any of our test runs. Need to
// add a test configuration for React Native.

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "alwaysThrottleRetries")]
    pub static ALWAYS_THROTTLE_RETRIES: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableObjectFiber")]
    pub static ENABLE_OBJECT_FIBER: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "passChildrenWhenCloningPersistedNodes")]
    pub static PASS_CHILDREN_WHEN_CLONING_PERSISTED_NODES: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableViewTransitionForPersistenceMode")]
    pub static ENABLE_VIEW_TRANSITION_FOR_PERSISTENCE_MODE: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableConditionalUseWarning")]
    pub static ENABLE_CONDITIONAL_USE_WARNING: bool = __VARIANT__;
}
