// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use shared::environment::__DEV__;
use webapi::console;

use crate::react_internal_types::Fiber;

pub struct StackCursor<T> {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub current: RefCell<T>,
}

thread_local! {
    static VALUE_STACK: RefCell<Vec<Option<&'static Unknown>>> = const { RefCell::new(Vec::new()) };

    static FIBER_STACK: RefCell<Vec<Option<Rc<Fiber>>>> = const { RefCell::new(Vec::new()) };

    static INDEX: Cell<i32> = const { Cell::new(-1) };
}

pub fn create_cursor<T>(default_value: T) -> StackCursor<T> {
    StackCursor {
        current: RefCell::new(default_value),
    }
}

pub fn pop<T>(cursor: &StackCursor<T>, fiber: &Fiber) {
    if INDEX.get() < 0 {
        if __DEV__ {
            console::error(("Unexpected pop.",));
        }
        return;
    }
    let index = INDEX.get() as usize;

    if __DEV__
        && FIBER_STACK.with_borrow(|fiber_stack| Some(fiber) != fiber_stack[index].as_deref())
    {
        console::error(("Unexpected Fiber popped.",));
    }

    cursor
        .current
        .replace(unsafe { js::cast(VALUE_STACK.with_borrow(|value_stack| value_stack[index])) });

    VALUE_STACK.with_borrow_mut(|value_stack| value_stack[index] = None);

    if __DEV__ {
        FIBER_STACK.with_borrow_mut(|fiber_stack| fiber_stack[index] = None);
    }

    INDEX.set(INDEX.get() - 1);
}

pub fn push<T: 'static>(cursor: &StackCursor<T>, value: T, fiber: &Rc<Fiber>) {
    INDEX.set(INDEX.get() + 1);
    let index = INDEX.get() as usize;

    let previous = js::any_of(cursor.current.replace(value));
    VALUE_STACK.with_borrow_mut(|value_stack| {
        if index < value_stack.len() {
            value_stack[index] = previous;
        } else {
            value_stack.push(previous);
        }
    });

    if __DEV__ {
        FIBER_STACK.with_borrow_mut(|fiber_stack| {
            if index < fiber_stack.len() {
                fiber_stack[index] = Some(Rc::clone(fiber));
            } else {
                fiber_stack.push(Some(Rc::clone(fiber)));
            }
        });
    }
}
