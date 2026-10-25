// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use shared::environment::__DEV__;
use shared::react_feature_flags::{ENABLE_GESTURE_TRANSITION, ENABLE_VIEW_TRANSITION};
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use webapi::console;

use crate::react_start_transition::{start_transition, t};

pub type TransitionTypes = Vec<String>;

pub fn add_transition_type(r#type: &'static str) {
    if ENABLE_VIEW_TRANSITION {
        if let Some(transition) = t(REACT_SHARED_INTERNALS) {
            let transition_types = transition.types.borrow().clone();
            match transition_types {
                None => {
                    transition
                        .types
                        .replace(Some(Rc::new(RefCell::new(vec![r#type.to_string()]))));
                }
                Some(transition_types) => {
                    if !transition_types.borrow().iter().any(|t| t == r#type) {
                        transition_types.borrow_mut().push(r#type.to_string());
                    }
                }
            }
        } else {
            // We're in the async gap. Simulate an implicit startTransition around it.
            if __DEV__ && REACT_SHARED_INTERNALS.async_transitions() == 0.0 {
                if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
                    console::error((concat!(
                        "addTransitionType can only be called inside a `startTransition()` ",
                        "or `startGestureTransition()` callback. ",
                        "It must be associated with a specific Transition."
                    ),));
                } else {
                    console::error((concat!(
                        "addTransitionType can only be called inside a `startTransition()` ",
                        "callback. It must be associated with a specific Transition."
                    ),));
                }
            }
            start_transition(
                move || {
                    add_transition_type(r#type);
                    None
                },
                None,
            );
        }
    }
}
