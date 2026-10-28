// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::get_vendor_prefixed_event_name::get_vendor_prefixed_event_name;

/// A DOM event's name, `'click'` say: one of the names the DOM fires,
/// some vendor-prefixed, as `getVendorPrefixedEventName` gives them, and
/// `'afterblur'` and `'beforeblur'`, not real events, used by event
/// experiments, and `'textInput'`, intentionally camelCase, non-standard.
pub type DOMEventName = String;

thread_local! {
    pub static ANIMATION_CANCEL: DOMEventName = get_vendor_prefixed_event_name("animationcancel");
    pub static ANIMATION_END: DOMEventName = get_vendor_prefixed_event_name("animationend");
    pub static ANIMATION_ITERATION: DOMEventName = get_vendor_prefixed_event_name("animationiteration");
    pub static ANIMATION_START: DOMEventName = get_vendor_prefixed_event_name("animationstart");

    pub static TRANSITION_RUN: DOMEventName = get_vendor_prefixed_event_name("transitionrun");
    pub static TRANSITION_START: DOMEventName = get_vendor_prefixed_event_name("transitionstart");
    pub static TRANSITION_CANCEL: DOMEventName = get_vendor_prefixed_event_name("transitioncancel");
    pub static TRANSITION_END: DOMEventName = get_vendor_prefixed_event_name("transitionend");
}
