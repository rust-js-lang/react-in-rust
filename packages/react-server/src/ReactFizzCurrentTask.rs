// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use shared::environment::__DEV__;

use crate::react_fizz_server::Task;

thread_local! {
    // DEV-only global reference to the currently executing task
    #[cfg_attr(rust_js, rust_js::name = "currentTaskInDEV", rust_js::nullable)]
    pub static CURRENT_TASK_IN_DEV: RefCell<Option<Rc<Task>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::name = "setCurrentTaskInDEV")]
pub fn set_current_task_in_dev(task: Option<Rc<Task>>) {
    if __DEV__ {
        CURRENT_TASK_IN_DEV.set(task);
    }
}
