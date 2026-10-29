// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

use crate::dom_event_names::DOMEventName;
use crate::react_synthetic_event_type::ReactSyntheticEvent;

/// What `createEventHandle` gives: given an `EventTarget` or a
/// `ReactScopeInstance`, and a callback, what removes the listener.
pub type ReactDOMEventHandle = dyn Fn(&'static Unknown, &'static Unknown) -> Box<dyn Fn()>;

pub struct ReactDOMEventHandleListener {
    pub callback: &'static dyn Fn(&ReactSyntheticEvent),
    pub capture: bool,
    pub r#type: DOMEventName,
}
