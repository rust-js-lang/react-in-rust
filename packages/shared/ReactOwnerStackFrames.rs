// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::JsError;
use node::js_error;

use crate::default_prepare_stack_trace::DEFAULT_PREPARE_STACK_TRACE;

pub fn format_owner_stack(error: &JsError) -> String {
    let prev_prepare_stack_trace = js_error::prepare_stack_trace();
    js_error::set_prepare_stack_trace(DEFAULT_PREPARE_STACK_TRACE);
    let stack = error.stack().unwrap_or_default();
    js_error::set_prepare_stack_trace(prev_prepare_stack_trace);
    let mut stack = stack.as_str();
    if let Some(rest) = stack.strip_prefix("Error: react-stack-top-frame\n") {
        // V8's default formatting prefixes with the error message which we
        // don't want/need.
        stack = rest;
    }
    if let Some(idx) = stack.find('\n') {
        // Pop the JSX frame.
        stack = &stack[idx + 1..];
    }
    let idx = stack
        .find("react_stack_bottom_frame")
        .and_then(|idx| stack[..idx].rfind('\n'));
    if let Some(idx) = idx {
        // Cut off everything after the bottom frame since it'll be internals.
        stack = &stack[..idx];
    } else {
        // We didn't find any internal callsite out to user space.
        // This means that this was called outside an owner or the owner is fully internal.
        // To keep things light we exclude the entire trace in this case.
        return String::new();
    }
    stack.to_string()
}
