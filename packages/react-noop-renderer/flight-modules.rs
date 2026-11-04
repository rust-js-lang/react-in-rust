// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js::Function;

// This file is used as temporary storage for modules generated in Flight tests.
thread_local! {
    static MODULE_IDX: Cell<f64> = const { Cell::new(0.0) };
    static MODULES: RefCell<HashMap<String, &'static Function>> = RefCell::new(HashMap::new());
}

// This simulates what the compiler will do when it replaces render functions with server blocks.
pub fn save_module(render: &'static Function) -> String {
    let idx = MODULE_IDX.get().to_string();
    MODULE_IDX.set(MODULE_IDX.get() + 1.0);
    MODULES.with_borrow_mut(|modules| modules.insert(idx.clone(), render));
    idx
}

pub fn read_module(idx: &str) -> Option<&'static Function> {
    MODULES.with_borrow(|modules| modules.get(idx).copied())
}
