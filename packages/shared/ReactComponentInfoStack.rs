// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::catch_unwind;

use js::JsError;

use crate::environment::__DEV__;
use crate::react_component_stack_frame::describe_built_in_component_frame;
use crate::react_owner_stack_frames::format_owner_stack;
use crate::react_types::ReactComponentInfo;

pub fn get_owner_stack_by_component_info_in_dev(component_info: &ReactComponentInfo) -> String {
    if !__DEV__ {
        return String::new();
    }
    match catch_unwind(|| {
        let mut info = String::new();

        // The owner stack of the current component will be where it was created, i.e. inside its owner.
        // There's no actual name of the currently executing component. Instead, that is available
        // on the regular stack that's currently executing. However, if there is no owner at all, then
        // there's no stack frame so we add the name of the root component to the stack to know which
        // component is currently executing.
        if component_info.owner.is_none() {
            return describe_built_in_component_frame(&component_info.name);
        }

        let mut owner = Some(component_info);

        while let Some(current) = owner {
            let owner_stack = current.debug_stack;
            if let Some(owner_stack) = owner_stack {
                // Server Component
                owner = current.owner;
                if owner.is_some() {
                    // TODO: Should we stash this somewhere for caching purposes?
                    info += "\n";
                    info += &format_owner_stack(owner_stack);
                }
            } else {
                break;
            }
        }
        info
    }) {
        Ok(info) => info,
        Err(x) => match x.downcast_ref::<&'static JsError>() {
            Some(x) => format!(
                "\nError generating stack: {}\n{}",
                x.message(),
                x.stack().unwrap_or_default()
            ),
            None => format!(
                "\nError generating stack: {}\n",
                x.downcast_ref::<String>().cloned().unwrap_or_default()
            ),
        },
    }
}
