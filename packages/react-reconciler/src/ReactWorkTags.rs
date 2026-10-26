// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub type WorkTag = u8;

#[cfg_attr(rust_js, rust_js::name = "FunctionComponent")]
pub const FUNCTION_COMPONENT: WorkTag = 0;
#[cfg_attr(rust_js, rust_js::name = "ClassComponent")]
pub const CLASS_COMPONENT: WorkTag = 1;
#[cfg_attr(rust_js, rust_js::name = "HostRoot")]
pub const HOST_ROOT: WorkTag = 3; // Root of a host tree. Could be nested inside another node.
#[cfg_attr(rust_js, rust_js::name = "HostPortal")]
pub const HOST_PORTAL: WorkTag = 4; // A subtree. Could be an entry point to a different renderer.
#[cfg_attr(rust_js, rust_js::name = "HostComponent")]
pub const HOST_COMPONENT: WorkTag = 5;
#[cfg_attr(rust_js, rust_js::name = "HostText")]
pub const HOST_TEXT: WorkTag = 6;
#[cfg_attr(rust_js, rust_js::name = "Fragment")]
pub const FRAGMENT: WorkTag = 7;
#[cfg_attr(rust_js, rust_js::name = "Mode")]
pub const MODE: WorkTag = 8;
#[cfg_attr(rust_js, rust_js::name = "ContextConsumer")]
pub const CONTEXT_CONSUMER: WorkTag = 9;
#[cfg_attr(rust_js, rust_js::name = "ContextProvider")]
pub const CONTEXT_PROVIDER: WorkTag = 10;
#[cfg_attr(rust_js, rust_js::name = "ForwardRef")]
pub const FORWARD_REF: WorkTag = 11;
#[cfg_attr(rust_js, rust_js::name = "Profiler")]
pub const PROFILER: WorkTag = 12;
#[cfg_attr(rust_js, rust_js::name = "SuspenseComponent")]
pub const SUSPENSE_COMPONENT: WorkTag = 13;
#[cfg_attr(rust_js, rust_js::name = "MemoComponent")]
pub const MEMO_COMPONENT: WorkTag = 14;
#[cfg_attr(rust_js, rust_js::name = "SimpleMemoComponent")]
pub const SIMPLE_MEMO_COMPONENT: WorkTag = 15;
#[cfg_attr(rust_js, rust_js::name = "LazyComponent")]
pub const LAZY_COMPONENT: WorkTag = 16;
#[cfg_attr(rust_js, rust_js::name = "IncompleteClassComponent")]
pub const INCOMPLETE_CLASS_COMPONENT: WorkTag = 17;
#[cfg_attr(rust_js, rust_js::name = "DehydratedFragment")]
pub const DEHYDRATED_FRAGMENT: WorkTag = 18;
#[cfg_attr(rust_js, rust_js::name = "SuspenseListComponent")]
pub const SUSPENSE_LIST_COMPONENT: WorkTag = 19;
#[cfg_attr(rust_js, rust_js::name = "ScopeComponent")]
pub const SCOPE_COMPONENT: WorkTag = 21;
#[cfg_attr(rust_js, rust_js::name = "OffscreenComponent")]
pub const OFFSCREEN_COMPONENT: WorkTag = 22;
#[cfg_attr(rust_js, rust_js::name = "LegacyHiddenComponent")]
pub const LEGACY_HIDDEN_COMPONENT: WorkTag = 23;
#[cfg_attr(rust_js, rust_js::name = "CacheComponent")]
pub const CACHE_COMPONENT: WorkTag = 24;
#[cfg_attr(rust_js, rust_js::name = "TracingMarkerComponent")]
pub const TRACING_MARKER_COMPONENT: WorkTag = 25;
#[cfg_attr(rust_js, rust_js::name = "HostHoistable")]
pub const HOST_HOISTABLE: WorkTag = 26;
#[cfg_attr(rust_js, rust_js::name = "HostSingleton")]
pub const HOST_SINGLETON: WorkTag = 27;
#[cfg_attr(rust_js, rust_js::name = "IncompleteFunctionComponent")]
pub const INCOMPLETE_FUNCTION_COMPONENT: WorkTag = 28;
#[cfg_attr(rust_js, rust_js::name = "Throw")]
pub const THROW: WorkTag = 29;
#[cfg_attr(rust_js, rust_js::name = "ViewTransitionComponent")]
pub const VIEW_TRANSITION_COMPONENT: WorkTag = 30;
#[cfg_attr(rust_js, rust_js::name = "ActivityComponent")]
pub const ACTIVITY_COMPONENT: WorkTag = 31;
