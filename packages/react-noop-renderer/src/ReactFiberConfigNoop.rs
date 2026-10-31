// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{JsObject, Unknown};

pub use crate::react_fiber_config_noop_hydration::*;
pub use crate::react_fiber_config_noop_resources::*;
pub use crate::react_fiber_config_noop_scopes::*;
pub use crate::react_fiber_config_noop_singletons::*;
pub use crate::react_fiber_config_noop_test_selectors::*;
// createReactNoop will overwrite these with the mutation or persistence versions.
pub use crate::react_fiber_config_noop_no_mutation::*;
pub use crate::react_fiber_config_noop_no_persistence::*;

/// This module's namespace: every host config function its modules give,
/// which createReactNoop spreads into the host config it makes.
#[cfg_attr(rust_js, rust_js::namespace)]
pub struct ReactFiberConfigNoop;

unsafe impl js::Defined for ReactFiberConfigNoop {}

#[cfg_attr(rust_js, rust_js::namespace)]
impl ReactFiberConfigNoop {}

pub type HostContext = JsObject;

pub struct TextInstance {
    pub text: RefCell<String>,
    pub id: f64,
    // A number; `undefined` once it's appended to a container, which has no
    // `id` (appendChildToContainerOrInstance).
    pub parent: Cell<Option<f64>>,
    pub hidden: Cell<bool>,
    pub context: &'static HostContext,
}

unsafe impl js::Defined for TextInstance {}

pub struct Instance {
    pub r#type: String,
    pub id: f64,
    pub parent: Cell<Option<f64>>,
    #[cfg_attr(rust_js, rust_js::test)]
    pub children: Rc<RefCell<Vec<Child>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub text: RefCell<Option<String>>,
    pub prop: Cell<Option<&'static Unknown>>,
    pub hidden: Cell<bool>,
    pub context: &'static HostContext,
    // A suspensey thing's, set where its props give one.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub src: RefCell<Option<String>>,
}

unsafe impl js::Defined for Instance {}

/// `Instance | TextInstance`, which tells them apart by `children`, an
/// instance's array.
#[derive(Clone)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Child {
    Instance(Rc<Instance>),
    Text(Rc<TextInstance>),
}

unsafe impl js::Defined for Child {}

pub type PublicInstance = Instance;

pub type TransitionStatus = Unknown;

pub struct Container {
    #[cfg_attr(rust_js, rust_js::name = "rootID", rust_js::test)]
    pub root_id: String,
    pub children: RefCell<Rc<RefCell<Vec<Child>>>>,
    pub pending_children: RefCell<Rc<RefCell<Vec<Child>>>>,
}

unsafe impl js::Defined for Container {}
