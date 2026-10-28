// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Kind;
use shared::execution_environment::CAN_USE_DOM;
use webapi::{document, tags::Div};

js::export_default! {
    /// Checks if an event is supported in the current execution environment.
    ///
    /// NOTE: This will not work correctly for non-generic events such as `change`,
    /// `reset`, `load`, `error`, and `select`.
    ///
    /// Borrows from Modernizr.
    ///
    /// @param {string} eventNameSuffix Event name, e.g. "click".
    /// @return {boolean} True if the event is supported.
    /// @internal
    /// @license Modernizr 3.0.0pre (Custom Build) | MIT
    pub fn is_event_supported(event_name_suffix: &str) -> bool {
        if !CAN_USE_DOM.with(|can_use_dom| *can_use_dom) {
            return false;
        }

        let event_name = "on".to_owned() + event_name_suffix;
        let mut is_supported = js::has(document, &event_name);

        if !is_supported {
            let element = document.create_element(Div);
            element.set_attribute(&event_name, "return;");
            is_supported = matches!(js::classify(js::get(element, &event_name)), Kind::Function(_));
        }

        is_supported
    }
}
