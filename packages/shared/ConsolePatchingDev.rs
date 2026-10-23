// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Helpers to patch console.logs to avoid logging during side-effect free
// replaying on render function. This currently only patches the object
// lazily which won't cover if the log function was extracted eagerly.
// We could also eagerly patch the method.

use std::cell::Cell;

use js::{IntoUnknown, PropertyDescriptor, PropertyDescriptorMap, Rest, object};
use webapi::console;

use crate::environment::__DEV__;

type ConsoleFn = fn(&'static dyn Rest<dyn IntoUnknown>);

thread_local! {
    static DISABLED_DEPTH: Cell<f64> = const { Cell::new(0.0) };
    static PREV_LOG: Cell<Option<ConsoleFn>> = const { Cell::new(None) };
    static PREV_INFO: Cell<Option<ConsoleFn>> = const { Cell::new(None) };
    static PREV_WARN: Cell<Option<ConsoleFn>> = const { Cell::new(None) };
    static PREV_ERROR: Cell<Option<ConsoleFn>> = const { Cell::new(None) };
    static PREV_GROUP: Cell<Option<ConsoleFn>> = const { Cell::new(None) };
    static PREV_GROUP_COLLAPSED: Cell<Option<ConsoleFn>> = const { Cell::new(None) };
    static PREV_GROUP_END: Cell<Option<fn()>> = const { Cell::new(None) };
}

fn disabled_log() {}
js::on_load! {
    js::set(js::unknown(&disabled_log), "__reactDisabledLog", true);
}

struct Logs<'a> {
    info: PropertyDescriptor<'a>,
    log: PropertyDescriptor<'a>,
    warn: PropertyDescriptor<'a>,
    error: PropertyDescriptor<'a>,
    group: PropertyDescriptor<'a>,
    group_collapsed: PropertyDescriptor<'a>,
    group_end: PropertyDescriptor<'a>,
}

unsafe impl PropertyDescriptorMap for Logs<'_> {}

pub fn disable_logs() {
    if __DEV__ {
        if DISABLED_DEPTH.get() == 0.0 {
            PREV_LOG.set(Some(console::log));
            PREV_INFO.set(Some(console::info));
            PREV_WARN.set(Some(console::warn));
            PREV_ERROR.set(Some(console::error));
            PREV_GROUP.set(Some(console::group));
            PREV_GROUP_COLLAPSED.set(Some(console::group_collapsed));
            PREV_GROUP_END.set(Some(console::group_end));
            // https://github.com/react/react/issues/19099
            let props = PropertyDescriptor {
                configurable: Some(true),
                enumerable: Some(true),
                value: Some(&disabled_log),
                writable: Some(true),
                ..Default::default()
            };
            object::define_properties(
                webapi::console,
                &Logs {
                    info: props,
                    log: props,
                    warn: props,
                    error: props,
                    group: props,
                    group_collapsed: props,
                    group_end: props,
                },
            );
        }
        DISABLED_DEPTH.set(DISABLED_DEPTH.get() + 1.0);
    }
}

pub fn reenable_logs() {
    if __DEV__ {
        DISABLED_DEPTH.set(DISABLED_DEPTH.get() - 1.0);
        if DISABLED_DEPTH.get() == 0.0 {
            let props = PropertyDescriptor {
                configurable: Some(true),
                enumerable: Some(true),
                writable: Some(true),
                ..Default::default()
            };
            object::define_properties(
                webapi::console,
                &Logs {
                    log: PropertyDescriptor {
                        value: Some(&PREV_LOG.get()),
                        ..props
                    },
                    info: PropertyDescriptor {
                        value: Some(&PREV_INFO.get()),
                        ..props
                    },
                    warn: PropertyDescriptor {
                        value: Some(&PREV_WARN.get()),
                        ..props
                    },
                    error: PropertyDescriptor {
                        value: Some(&PREV_ERROR.get()),
                        ..props
                    },
                    group: PropertyDescriptor {
                        value: Some(&PREV_GROUP.get()),
                        ..props
                    },
                    group_collapsed: PropertyDescriptor {
                        value: Some(&PREV_GROUP_COLLAPSED.get()),
                        ..props
                    },
                    group_end: PropertyDescriptor {
                        value: Some(&PREV_GROUP_END.get()),
                        ..props
                    },
                },
            );
        }
        if DISABLED_DEPTH.get() < 0.0 {
            console::error((concat!(
                "disabledDepth fell below zero. ",
                "This is a bug in React. Please file an issue."
            ),));
        }
    }
}
