// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use shared::react_feature_flags::ENABLE_VIEW_TRANSITION;

use crate::react_fiber_lane::includes_transition_lane;
use crate::react_internal_types::FiberRoot;

pub fn queue_transition_types(root: &FiberRoot, transition_types: &[String]) {
    if ENABLE_VIEW_TRANSITION {
        // TODO: We should really store transitionTypes per lane in a LaneMap on
        // the root. Then merge it when we commit. We currently assume that all
        // Transitions are entangled.
        if includes_transition_lane(root.pending_lanes.get()) {
            let queued = Rc::clone(
                root.transition_types
                    .borrow_mut()
                    .get_or_insert_with(|| Rc::new(RefCell::new(Vec::new()))),
            );
            for transition_type in transition_types {
                if !queued.borrow().contains(transition_type) {
                    queued.borrow_mut().push(transition_type.clone());
                }
            }
        }
    }
}

thread_local! {
    // Store all types while we're entangled with an async Transition.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static ENTANGLED_TRANSITION_TYPES: RefCell<Option<Rc<RefCell<Vec<String>>>>> =
        const { RefCell::new(None) };
}

pub fn entangle_async_transition_types(transition_types: &[String]) {
    if ENABLE_VIEW_TRANSITION {
        let queued = ENTANGLED_TRANSITION_TYPES.with_borrow_mut(|entangled_transition_types| {
            Rc::clone(
                entangled_transition_types.get_or_insert_with(|| Rc::new(RefCell::new(Vec::new()))),
            )
        });
        for transition_type in transition_types {
            if !queued.borrow().contains(transition_type) {
                queued.borrow_mut().push(transition_type.clone());
            }
        }
    }
}

pub fn clear_entangled_async_transition_types() {
    // Called when all Async Actions are done.
    ENTANGLED_TRANSITION_TYPES.set(None);
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn claim_queued_transition_types(root: &FiberRoot) -> Option<Rc<RefCell<Vec<String>>>> {
    root.transition_types.take()
}
