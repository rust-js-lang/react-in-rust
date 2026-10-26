// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{JsError, Unknown, WeakKey, WeakMap};

use crate::react_internal_types::Fiber;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberComponentStack.js#getStackByFiberInDevAndProd"
)]
fn get_stack_by_fiber_in_dev_and_prod(work_in_progress: &Fiber) -> String {
    unreachable!()
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "CapturedStacks")]
    static CAPTURED_STACKS: &'static WeakMap<dyn WeakKey, Rc<CapturedValue<Option<&'static Unknown>>>> =
        js::weak_map::new();
}

pub struct CapturedValue<T> {
    pub value: T,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub source: Option<Rc<Fiber>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub stack: Option<String>,
}

pub fn create_captured_value_at_fiber(
    value: Option<&'static Unknown>,
    source: &Rc<Fiber>,
) -> Rc<CapturedValue<Option<&'static Unknown>>> {
    // If the value is an error, call this function immediately after it is thrown
    // so the stack is accurate.
    if let Some(object) = js::classify(value).object() {
        let existing = CAPTURED_STACKS.with(|captured_stacks| captured_stacks.get(object));
        if let Some(existing) = existing {
            return existing;
        }
        let captured = Rc::new(CapturedValue {
            value,
            source: Some(Rc::clone(source)),
            stack: Some(get_stack_by_fiber_in_dev_and_prod(source)),
        });
        CAPTURED_STACKS.with(|captured_stacks| captured_stacks.set(object, Rc::clone(&captured)));
        captured
    } else {
        Rc::new(CapturedValue {
            value,
            source: Some(Rc::clone(source)),
            stack: Some(get_stack_by_fiber_in_dev_and_prod(source)),
        })
    }
}

#[cfg_attr(rust_js, rust_js::nullable(stack))]
pub fn create_captured_value_from_error(
    value: &'static JsError,
    stack: Option<String>,
) -> Rc<CapturedValue<Option<&'static Unknown>>> {
    let captured = Rc::new(CapturedValue {
        value: value.as_unknown(),
        source: None,
        stack,
    });
    if captured.stack.is_some() {
        CAPTURED_STACKS.with(|captured_stacks| captured_stacks.set(value, Rc::clone(&captured)));
    }
    captured
}
