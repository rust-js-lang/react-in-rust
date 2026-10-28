// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;
use react_reconciler::react_internal_types::Fiber;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/client/ReactDOMComponentTree.js#getFiberCurrentPropsFromNode"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_fiber_current_props_from_node(node: &Unknown) -> Option<&'static Unknown> {
    unreachable!()
}

fn is_interactive(tag: &str) -> bool {
    tag == "button" || tag == "input" || tag == "select" || tag == "textarea"
}

fn should_prevent_mouse_event(name: &str, r#type: &str, props: &Unknown) -> bool {
    match name {
        "onClick"
        | "onClickCapture"
        | "onDoubleClick"
        | "onDoubleClickCapture"
        | "onMouseDown"
        | "onMouseDownCapture"
        | "onMouseMove"
        | "onMouseMoveCapture"
        | "onMouseUp"
        | "onMouseUpCapture"
        | "onMouseEnter" => js::truthy(js::get(props, "disabled")) && is_interactive(r#type),
        _ => false,
    }
}

js::export_default! {
    /// @param {object} inst The instance, which is the source of events.
    /// @param {string} registrationName Name of listener (e.g. `onClick`).
    /// @return {?function} The stored callback.
    pub fn get_listener(inst: &Fiber, registration_name: &str) -> Option<&'static Unknown> {
        let Some(state_node) = inst.state_node.get() else {
            // Work in progress (ex: onload events in incremental mode).
            return js::null();
        };
        let Some(props) = get_fiber_current_props_from_node(state_node) else {
            // Work in progress.
            return js::null();
        };
        // $FlowFixMe[invalid-computed-prop]
        let listener = js::get(props, registration_name);
        if should_prevent_mouse_event(registration_name, unsafe { js::cast(inst.r#type.get()) }, props) {
            return js::null();
        }

        if js::truthy(listener) && js::type_of(listener) != "function" {
            panic!(
                "Expected `{registration_name}` listener to be a function, instead got a value of `{}` type.",
                js::type_of(listener)
            );
        }

        listener
    }
}
