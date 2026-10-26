// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#requestPostPaintCallback"
)]
fn request_post_paint_callback(callback: impl Fn(f64) + 'static) {
    unreachable!()
}

thread_local! {
    static POST_PAINT_CALLBACK_SCHEDULED: Cell<bool> = const { Cell::new(false) };
    static CALLBACKS: RefCell<Vec<&'static dyn Fn(f64)>> = const { RefCell::new(Vec::new()) };
}

pub fn schedule_post_paint_callback(callback: &'static dyn Fn(f64)) {
    CALLBACKS.with_borrow_mut(|callbacks| callbacks.push(callback));
    if !POST_PAINT_CALLBACK_SCHEDULED.get() {
        POST_PAINT_CALLBACK_SCHEDULED.set(true);
        request_post_paint_callback(|end_time| {
            let mut i = 0;
            while let Some(callback) = CALLBACKS.with_borrow(|callbacks| callbacks.get(i).copied())
            {
                callback(end_time);
                i += 1;
            }
            POST_PAINT_CALLBACK_SCHEDULED.set(false);
            CALLBACKS.set(Vec::new());
        });
    }
}
