// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::marker::PhantomData;

use js::JsObject;
use shared::environment::__DEV__;
use webapi::ConsoleTask;

use crate::react_fizz_component_stack::ComponentStackNode;

/// A task of ReactFizzServer's, its JS's until it's ported: what's read of
/// it here.
pub struct Task(PhantomData<JsObject>);

unsafe impl js::Defined for Task {}

impl Task {
    #[cfg_attr(rust_js, rust_js::link_name = "get componentStack")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn component_stack(&self) -> Option<&'static ComponentStackNode> {
        unreachable!()
    }

    // DEV only
    #[cfg_attr(rust_js, rust_js::link_name = "get debugTask")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn debug_task(&self) -> Option<&'static ConsoleTask> {
        unreachable!()
    }
}

thread_local! {
    // DEV-only global reference to the currently executing task
    #[cfg_attr(rust_js, rust_js::name = "currentTaskInDEV", rust_js::nullable)]
    pub static CURRENT_TASK_IN_DEV: Cell<Option<&'static Task>> = const { Cell::new(None) };
}

#[cfg_attr(rust_js, rust_js::name = "setCurrentTaskInDEV")]
pub fn set_current_task_in_dev(task: Option<&'static Task>) {
    if __DEV__ {
        CURRENT_TASK_IN_DEV.set(task);
    }
}
