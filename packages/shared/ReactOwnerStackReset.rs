// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use crate::environment::__DEV__;
use crate::react_shared_internals::REACT_SHARED_INTERNALS;

thread_local! {
    static LAST_RESET_TIME: Cell<f64> = const { Cell::new(0.0) };
}

thread_local! {
    static GET_CURRENT_TIME: Box<dyn Fn() -> f64> = if js::is_object(webapi::performance) && js::is_function(webapi::Performance::now) {
        let local_performance = webapi::performance;
        Box::new(move || local_performance.now())
    } else {
        Box::new(js::date::now)
    };
}

pub fn reset_owner_stack_limit() {
    if __DEV__ {
        let now = GET_CURRENT_TIME.with(|get_current_time| get_current_time());
        let time_since_last_reset = now - LAST_RESET_TIME.get();
        if time_since_last_reset > 1000.0 {
            REACT_SHARED_INTERNALS.set_recently_created_owner_stacks(0.0);
            LAST_RESET_TIME.set(now);
        }
    } else {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "resetOwnerStackLimit should never be called in production mode. This is a bug in React."
        );
    }
}
