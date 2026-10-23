// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This file replaces DefaultPrepareStackTrace in Edge/Node Server builds.

use js::JsError;
use node::CallSite;

fn prepare_stack_trace(error: &JsError, structured_stack_trace: &[&CallSite]) -> String {
    let mut name = error.name();
    if name.is_empty() {
        name = "Error".to_string();
    }
    let message = error.message();
    let mut stack = name + ": " + &message;
    for call_site in structured_stack_trace {
        stack += "\n    at ";
        stack += &call_site.to_string();
    }
    stack
}

js::export_default!(prepare_stack_trace);
