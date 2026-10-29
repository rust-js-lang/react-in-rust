// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic::catch_unwind;

use js::{Unknown, function};
use webapi::console;

use crate::environment::__DEV__;

type EnqueueTask = fn(Box<dyn Fn()>);

thread_local! {
    static DID_WARN_ABOUT_MESSAGE_CHANNEL: Cell<bool> = const { Cell::new(false) };
    static ENQUEUE_TASK_IMPL: Cell<Option<EnqueueTask>> = const { Cell::new(None) };
}

js::export_default! {
    pub fn enqueue_task(task: Box<dyn Fn()>) {
        if ENQUEUE_TASK_IMPL.get().is_none()
            && let Err(_err) = catch_unwind(|| {
                // read require off the module object to get around the bundlers.
                // we don't want them to detect a require and bundle a Node polyfill.
                let require_string = js::string::slice(
                    &format!("require{}", js::shown(js::math::random())),
                    0.0,
                    Some(7.0),
                );
                // $FlowFixMe[invalid-computed-prop]
                let node_require: &dyn Fn(&str) -> Option<&'static Unknown> =
                    unsafe { js::cast(js::get(js::unknown(node::module), &require_string)) };
                // assuming we're in node, let's try to get node's
                // version of setImmediate, bypassing fake timers if any.
                let timers: &Unknown = unsafe {
                    js::cast(function::call(
                        node_require,
                        Some(&node::module),
                        ("timers",),
                    ))
                };
                ENQUEUE_TASK_IMPL.set(Some(unsafe { js::cast(js::get(timers, "setImmediate")) }));
            })
        {
            // we're in a browser
            // we can't use regular timers because they may still be faked
            // so we try MessageChannel+postMessage instead
            ENQUEUE_TASK_IMPL.set(Some(|callback| {
                if __DEV__ && !DID_WARN_ABOUT_MESSAGE_CHANNEL.get() {
                    DID_WARN_ABOUT_MESSAGE_CHANNEL.set(true);
                    if !js::is_defined(webapi::message_channel::new) {
                        console::error((concat!(
                            "This browser does not have a MessageChannel implementation, ",
                            "so enqueuing tasks via await act(async () => ...) will fail. ",
                            "Please file an issue at https://github.com/react/react/issues ",
                            "if you encounter this warning."
                        ),));
                    }
                }
                let channel = webapi::message_channel::new();
                channel.port1().set_onmessage(Some(webapi::listener(move |_| callback())));
                channel.port2().post_message((), None);
            }));
        }
        ENQUEUE_TASK_IMPL.get().unwrap()(task)
    }
}
