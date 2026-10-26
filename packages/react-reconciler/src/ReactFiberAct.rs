// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use webapi::console;

use crate::react_internal_types::Fiber;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#warnsIfNotActing"]
    safe static WARNS_IF_NOT_ACTING: bool;

    // Whether a test says it runs its updates in act(...), where it says so:
    // any value, read as a boolean.
    #[link_name = "IS_REACT_ACT_ENVIRONMENT"]
    safe static IS_REACT_ACT_ENVIRONMENT: Option<&'static Unknown>;

    // Jest's, where the test runs in it.
    #[link_name = "jest"]
    safe static JEST: &'static Unknown;
}

pub fn is_legacy_act_environment(_fiber: &Fiber) -> bool {
    if __DEV__ {
        // Legacy mode. We preserve the behavior of React 17's act. It assumes an
        // act environment whenever `jest` is defined, but you can still turn off
        // spurious warnings by setting IS_REACT_ACT_ENVIRONMENT explicitly
        // to false.

        let is_react_act_environment_global = if js::is_defined(IS_REACT_ACT_ENVIRONMENT) {
            IS_REACT_ACT_ENVIRONMENT
        } else {
            None
        };

        let jest_is_defined = js::is_defined(JEST);
        return WARNS_IF_NOT_ACTING
            && jest_is_defined
            && !js::strict_equals(is_react_act_environment_global, false);
    }
    false
}

pub fn is_concurrent_act_environment() -> Option<bool> {
    if __DEV__ {
        let is_react_act_environment_global = if js::is_defined(IS_REACT_ACT_ENVIRONMENT) {
            IS_REACT_ACT_ENVIRONMENT
        } else {
            None
        };

        if !js::truthy(is_react_act_environment_global)
            && REACT_SHARED_INTERNALS.act_queue().is_some()
        {
            // TODO: Include link to relevant documentation page.
            console::error((concat!(
                "The current testing environment is not configured to support ",
                "act(...)"
            ),));
        }
        return unsafe { js::cast(is_react_act_environment_global) };
    }
    Some(false)
}
