// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use shared::environment::__DEV__;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;

use crate::get_component_name_from_fiber::get_component_name_from_owner;
use crate::react_internal_types::Fiber;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberComponentStack.js#getOwnerStackByFiberInDev"
)]
fn get_owner_stack_by_fiber_in_dev(work_in_progress: &Fiber) -> String {
    unreachable!()
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CURRENT: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    pub static IS_RENDERING: Cell<bool> = const { Cell::new(false) };
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_current_fiber_owner_name_in_dev_or_null() -> Option<String> {
    if __DEV__ {
        let current = CURRENT.with_borrow(|current| current.clone())?;
        let owner = current.debug_owner.get();
        if let Some(owner) = owner {
            return get_component_name_from_owner(owner);
        }
    }
    None
}

fn get_current_fiber_stack_in_dev() -> String {
    if __DEV__ {
        let Some(current) = CURRENT.with_borrow(|current| current.clone()) else {
            return String::new();
        };
        // Safe because if current fiber exists, we are reconciling,
        // and it is guaranteed to be the work-in-progress version.
        // TODO: The above comment is not actually true. We might be
        // in a commit phase or preemptive set state callback.
        return get_owner_stack_by_fiber_in_dev(&current);
    }
    String::new()
}

struct RestoreCurrentFiber(Option<Rc<Fiber>>);

impl Drop for RestoreCurrentFiber {
    fn drop(&mut self) {
        set_current_fiber(self.0.take());
    }
}

#[cfg_attr(rust_js, rust_js::name = "runWithFiberInDEV")]
#[cfg_attr(rust_js, rust_js::nullable(fiber))]
pub fn run_with_fiber_in_dev<A0, A1, A2, A3, A4, T>(
    fiber: Option<Rc<Fiber>>,
    callback: impl FnOnce(A0, A1, A2, A3, A4) -> T,
    arg0: A0,
    arg1: A1,
    arg2: A2,
    arg3: A3,
    arg4: A4,
) -> T {
    if __DEV__ {
        let previous_fiber = CURRENT.with_borrow(|current| current.clone());
        set_current_fiber(fiber.clone());
        let _restore = RestoreCurrentFiber(previous_fiber);
        if let Some(fiber) = &fiber
            && let Some(debug_task) = fiber.debug_task.get()
        {
            return debug_task.run(move || callback(arg0, arg1, arg2, arg3, arg4));
        }
        return callback(arg0, arg1, arg2, arg3, arg4);
    }
    // These errors should never make it into a build so we don't need to encode them in codes.json
    // eslint-disable-next-line react-internal/prod-error-codes
    panic!("runWithFiberInDEV should never be called in production. This is a bug in React.");
}

pub fn reset_current_fiber() {
    if __DEV__ {
        REACT_SHARED_INTERNALS.set_get_current_stack(None);
        IS_RENDERING.set(false);
    }
    CURRENT.set(None);
}

#[cfg_attr(rust_js, rust_js::nullable(fiber))]
pub fn set_current_fiber(fiber: Option<Rc<Fiber>>) {
    if __DEV__ {
        REACT_SHARED_INTERNALS.set_get_current_stack(if fiber.is_none() {
            None
        } else {
            Some(get_current_fiber_stack_in_dev)
        });
        IS_RENDERING.set(false);
    }
    CURRENT.set(fiber);
}

pub fn set_is_rendering(rendering: bool) {
    if __DEV__ {
        IS_RENDERING.set(rendering);
    }
}

pub fn get_is_rendering() -> Option<bool> {
    if __DEV__ {
        return Some(IS_RENDERING.get());
    }
    None
}
