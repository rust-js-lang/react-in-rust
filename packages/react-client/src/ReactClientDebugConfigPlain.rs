// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use shared::environment::__DEV__;
use webapi::console;

thread_local! {
    static HAS_CONFIRMED_EVAL: Cell<bool> = const { Cell::new(false) };
}

pub fn check_eval_availability_once_dev() {
    if __DEV__ {
        if !HAS_CONFIRMED_EVAL.get() {
            HAS_CONFIRMED_EVAL.set(true);
            if js::eval("null").is_err() {
                console::error((concat!(
                    "eval() is not supported in this environment. ",
                    "React requires eval() in development mode for various debugging features ",
                    "like reconstructing callstacks from a different environment.\n",
                    "React will never use eval() in production mode",
                ),));
            }
        }
    } else {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        panic!(
            "checkEvalAvailabilityOnceDev should never be called in production mode. This is a bug in React."
        );
    }
}
