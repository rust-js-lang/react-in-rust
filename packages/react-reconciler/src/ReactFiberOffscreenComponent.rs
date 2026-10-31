// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{JsObject, Set, Unknown, WeakSet};
use react_isomorphic::react_start_transition::Transition;

use crate::react_fiber_lane::Lanes;
use crate::react_fiber_suspense_component::RetryQueue;
use crate::react_fiber_tracing_marker_component::TracingMarkerInstance;
use shared::react_types::ReactNodeList;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OffscreenMode {
    #[cfg_attr(rust_js, rust_js::name = "hidden")]
    Hidden,
    #[cfg_attr(rust_js, rust_js::name = "unstable-defer-without-hiding")]
    UnstableDeferWithoutHiding,
    #[cfg_attr(rust_js, rust_js::name = "visible")]
    Visible,
}

pub struct LegacyHiddenProps {
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub mode: Option<OffscreenMode>,
    pub children: Option<&'static ReactNodeList>,
}

unsafe impl js::Defined for LegacyHiddenProps {}

pub struct OffscreenProps {
    // TODO: Pick an API before exposing the Offscreen type. I've chosen an enum
    // for now, since we might have multiple variants. For example, hiding the
    // content without changing the layout.
    //
    // Default mode is visible. Kind of a weird default for a component
    // called "Offscreen." Possible alt: <Visibility />?
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub mode: Option<OffscreenMode>,
    pub children: Option<&'static ReactNodeList>,
}

unsafe impl js::Defined for OffscreenProps {}

// We use the existence of the state object as an indicator that the component
// is hidden.
pub struct OffscreenState {
    // TODO: This doesn't do anything, yet. It's always NoLanes. But eventually it
    // will represent the pending work that must be included in the render in
    // order to unhide the component.
    pub base_lanes: Lanes,
    // ReactFiberCacheComponent's `SpawnedCachePool`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub cache_pool: Option<&'static Unknown>,
}

pub struct OffscreenQueue {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transitions: RefCell<Option<Rc<Vec<&'static Transition>>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub marker_instances: RefCell<Option<Rc<Vec<Rc<TracingMarkerInstance>>>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub retry_queue: Cell<Option<&'static RetryQueue>>,
}

// Safety: each is a JS object: an Offscreen's state, its update queue and
// its instance.
unsafe impl js::Defined for OffscreenState {}
unsafe impl js::Defined for OffscreenQueue {}
unsafe impl js::Defined for OffscreenInstance {}

pub type OffscreenVisibility = i32;

#[cfg_attr(rust_js, rust_js::name = "OffscreenVisible")]
pub const OFFSCREEN_VISIBLE: OffscreenVisibility = /*                     */ 0b001;
#[cfg_attr(rust_js, rust_js::name = "OffscreenPassiveEffectsConnected")]
pub const OFFSCREEN_PASSIVE_EFFECTS_CONNECTED: OffscreenVisibility = /*     */ 0b010;

pub struct OffscreenInstance {
    #[cfg_attr(rust_js, rust_js::name = "_visibility")]
    pub visibility: Cell<OffscreenVisibility>,
    #[cfg_attr(rust_js, rust_js::name = "_pendingMarkers")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending_markers: Cell<Option<&'static Set<TracingMarkerInstance>>>,
    #[cfg_attr(rust_js, rust_js::name = "_transitions")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transitions: Cell<Option<&'static Set<Transition>>>,
    // shared/ReactTypes' `Wakeable`s: a `WeakSet` where the host has one, a
    // `Set` where it hasn't.
    #[cfg_attr(rust_js, rust_js::name = "_retryCache")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub retry_cache: Cell<Option<&'static WeakSet<JsObject>>>,
}
