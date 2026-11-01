// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};

/// The types of ART's nodes, as a React element names them.
pub struct Types {
    #[cfg_attr(rust_js, rust_js::name = "CLIPPING_RECTANGLE")]
    pub clipping_rectangle: &'static str,
    #[cfg_attr(rust_js, rust_js::name = "GROUP")]
    pub group: &'static str,
    #[cfg_attr(rust_js, rust_js::name = "SHAPE")]
    pub shape: &'static str,
    #[cfg_attr(rust_js, rust_js::name = "TEXT")]
    pub text: &'static str,
}

pub const TYPES: Types = Types {
    clipping_rectangle: "ClippingRectangle",
    group: "Group",
    shape: "Shape",
    text: "Text",
};

/// The events ART's nodes are subscribed to, by the prop that listens.
pub struct EventTypes {
    pub on_click: &'static str,
    pub on_mouse_move: &'static str,
    pub on_mouse_over: &'static str,
    pub on_mouse_out: &'static str,
    pub on_mouse_up: &'static str,
    pub on_mouse_down: &'static str,
}

pub const EVENT_TYPES: EventTypes = EventTypes {
    on_click: "click",
    on_mouse_move: "mousemove",
    on_mouse_over: "mouseover",
    on_mouse_out: "mouseout",
    on_mouse_up: "mouseup",
    on_mouse_down: "mousedown",
};

pub fn children_as_string(children: Option<&Unknown>) -> String {
    if !js::truthy(children) {
        String::new()
    } else if let Kind::String(children) = js::classify(children) {
        children.to_owned()
    } else if js::truthy(js::get(children, "length")) {
        // SAFETY: what has a length is joined as upstream joins it, by its
        // own `join`, which throws where it has none.
        js::array::join(unsafe { js::cast(children) }, "")
    } else {
        String::new()
    }
}
