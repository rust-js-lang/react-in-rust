// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use js::Dict;
use shared::environment::__DEV__;
use webapi::console;

use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_fiber_view_transition_component::ViewTransitionProps;
use crate::react_internal_types::Fiber;

thread_local! {
    // Use in DEV to track mounted named ViewTransitions. This is used to warn for
    // duplicate names. This should technically be tracked per Document because you could
    // have two different documents that can have separate namespaces, but to keep things
    // simple we just use a global Map. Technically it should also include any manually
    // assigned view-transition-name outside React too.
    static MOUNTED_NAMED_VIEW_TRANSITIONS: RefCell<HashMap<String, Rc<Fiber>>> =
        RefCell::new(HashMap::new());
    static DID_WARN_ABOUT_NAME: &'static Dict<bool> = js::dict::new();
}

pub fn track_named_view_transition(fiber: &Rc<Fiber>) {
    if __DEV__ {
        let name = unsafe { js::cast::<&ViewTransitionProps>(fiber.memoized_props.get()) }
            .name
            .as_deref();
        if let Some(name) = name
            && name != "auto"
        {
            let existing =
                MOUNTED_NAMED_VIEW_TRANSITIONS.with_borrow(|mounted_named_view_transitions| {
                    mounted_named_view_transitions.get(name).cloned()
                });
            if let Some(existing) = existing {
                if existing != *fiber
                    && Some(&existing) != fiber.alternate.borrow().as_ref()
                    && !DID_WARN_ABOUT_NAME.with(|did_warn_about_name| {
                        js::dict::get(did_warn_about_name, name).is_some_and(|&warned| warned)
                    })
                {
                    DID_WARN_ABOUT_NAME.with(|did_warn_about_name| {
                        js::dict::set(did_warn_about_name, name, true);
                    });
                    let stringified_name = js::json::stringify(name);
                    run_with_fiber_in_dev(
                        Some(Rc::clone(fiber)),
                        |_: (), _: (), _: (), _: (), _: ()| {
                            console::error((
                                concat!(
                                    "There are two <ViewTransition name=%s> components with the same name mounted ",
                                    "at the same time. This is not supported and will cause View Transitions ",
                                    "to error. Try to use a more unique name e.g. by using a namespace prefix ",
                                    "and adding the id of an item to the name."
                                ),
                                &stringified_name,
                            ));
                        },
                        (),
                        (),
                        (),
                        (),
                        (),
                    );
                    run_with_fiber_in_dev(
                        Some(existing),
                        |_: (), _: (), _: (), _: (), _: ()| {
                            console::error((
                                "The existing <ViewTransition name=%s> duplicate has this stack trace.",
                                &stringified_name,
                            ));
                        },
                        (),
                        (),
                        (),
                        (),
                        (),
                    );
                }
            } else {
                MOUNTED_NAMED_VIEW_TRANSITIONS.with_borrow_mut(|mounted_named_view_transitions| {
                    mounted_named_view_transitions.insert(name.to_string(), Rc::clone(fiber));
                });
            }
        }
    }
}

pub fn untrack_named_view_transition(fiber: &Fiber) {
    if __DEV__ {
        let name = unsafe { js::cast::<&ViewTransitionProps>(fiber.memoized_props.get()) }
            .name
            .as_deref();
        if let Some(name) = name
            && name != "auto"
        {
            let existing =
                MOUNTED_NAMED_VIEW_TRANSITIONS.with_borrow(|mounted_named_view_transitions| {
                    mounted_named_view_transitions.get(name).cloned()
                });
            if let Some(existing) = existing
                && (*existing == *fiber || Some(&existing) == fiber.alternate.borrow().as_ref())
            {
                MOUNTED_NAMED_VIEW_TRANSITIONS.with_borrow_mut(|mounted_named_view_transitions| {
                    mounted_named_view_transitions.remove(name);
                });
            }
        }
    }
}
