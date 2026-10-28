// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::Unknown;
use webapi::{Blob, Element, Event, EventTargetExt, url};

pub fn set_src_object(dom_element: &'static Element, tag: &str, value: &Unknown) {
    // We optimistically create the URL regardless of object type. This lets us
    // support cross-realms and any type that the browser supports like new types.
    let url = url::create_object_url(unsafe { js::cast::<&Blob>(Some(value)) });
    let load_event = if tag == "img" { "load" } else { "loadstart" };
    let clean_up_listener = Rc::new(Cell::new(None));
    let clean_up = webapi::listener({
        let clean_up_listener = Rc::clone(&clean_up_listener);
        let url = url.clone();
        move |_: &Event| {
            // Once the object has started loading, then it's already collected by the
            // browser and it won't refer to it by the URL anymore so we can now revoke it.
            url::revoke_object_url(&url);
            if let Some(clean_up) = clean_up_listener.get() {
                dom_element.remove_event_listener_named(load_event, clean_up, None);
                dom_element.remove_event_listener_named("error", clean_up, None);
            }
        }
    });
    clean_up_listener.set(Some(clean_up));
    dom_element.add_event_listener_named(load_event, clean_up, None);
    dom_element.add_event_listener_named("error", clean_up, None);
    dom_element.set_attribute("src", &url);
}
