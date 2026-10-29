// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Flow type for SyntheticEvent class that includes private properties

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_reconciler::react_internal_types::Fiber;
use webapi::{Event, EventTarget};

use crate::dom_event_names::DOMEventName;

/// A `DispatchConfig`'s two phases' registration names.
pub struct PhasedRegistrationNames {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub bubbled: Option<String>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub captured: Option<String>,
}

pub struct DispatchConfig {
    pub dependencies: Option<Vec<DOMEventName>>,
    pub phased_registration_names: PhasedRegistrationNames,
    pub registration_name: Option<String>,
}

/// A synthetic event, its private properties too: a `KnownReactSyntheticEvent`
/// where its `_reactName` is a string, an `UnknownReactSyntheticEvent` where
/// it's `null`. SyntheticEvent's constructors make it.
pub struct ReactSyntheticEvent {
    pub is_propagation_stopped: Cell<fn() -> bool>,
    pub is_default_prevented: Cell<fn() -> bool>,
    // `null`, a fiber, or an array of fibers or `null`s.
    #[cfg_attr(rust_js, rust_js::name = "_dispatchInstances")]
    pub dispatch_instances: Option<&'static Unknown>,
    // `null`, a function, or an array of functions.
    #[cfg_attr(rust_js, rust_js::name = "_dispatchListeners")]
    pub dispatch_listeners: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "_targetInst", rust_js::nullable)]
    pub target_inst: RefCell<Option<Rc<Fiber>>>,
    pub native_event: Cell<Option<&'static Event>>,
    pub target: Cell<Option<&'static Unknown>>,
    pub related_target: Cell<Option<&'static Unknown>>,
    pub r#type: RefCell<String>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub current_target: Cell<Option<&'static EventTarget>>,
    #[cfg_attr(rust_js, rust_js::name = "_reactName", rust_js::nullable)]
    pub react_name: RefCell<Option<String>>,
}

unsafe impl js::Defined for ReactSyntheticEvent {}

/// The methods SyntheticEvent's constructors' prototypes give each event.
impl ReactSyntheticEvent {
    #[cfg_attr(rust_js, rust_js::link_name = "preventDefault")]
    pub fn prevent_default(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "stopPropagation")]
    pub fn stop_propagation(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "persist")]
    pub fn persist(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "isPersistent")]
    pub fn is_persistent(&self) -> bool {
        unreachable!()
    }
}

pub type KnownReactSyntheticEvent = ReactSyntheticEvent;
pub type UnknownReactSyntheticEvent = ReactSyntheticEvent;
