// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Set, Unknown};

use crate::react_fiber_flags::{DID_CAPTURE, NO_FLAGS};
use crate::react_fiber_hydration_context::HydrationErrors;
use crate::react_fiber_lane::Lane;
use crate::react_fiber_tree_context::TreeContext;
use crate::react_internal_types::Fiber;
use crate::react_work_tags::{SUSPENSE_COMPONENT, SUSPENSE_LIST_COMPONENT};
use shared::react_types::SuspenseListTailMode;

// ReactFiberConfig's, of its `SuspenseInstance`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isSuspenseInstancePending"
)]
fn is_suspense_instance_pending(instance: &Unknown) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isSuspenseInstanceFallback"
)]
fn is_suspense_instance_fallback(instance: &Unknown) -> bool {
    unreachable!()
}

// A null SuspenseState represents an unsuspended normal Suspense boundary.
// A non-null SuspenseState means that it is blocked for one reason or another.
// - A non-null dehydrated field means it's blocked pending hydration.
//   - A non-null dehydrated field can use isSuspenseInstancePending or
//     isSuspenseInstanceFallback to query the reason for being dehydrated.
// - A null dehydrated field means it's blocked by something suspending and
//   we're currently showing a fallback instead.
pub struct SuspenseState {
    // If this boundary is still dehydrated, we store the SuspenseInstance
    // here to indicate that it is dehydrated (flag) and for quick access
    // to check things like isSuspenseInstancePending.
    // ReactFiberConfig's `SuspenseInstance`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub dehydrated: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub tree_context: Option<TreeContext>,
    // Represents the lane we should attempt to hydrate a dehydrated boundary at.
    // OffscreenLane is the default for dehydrated boundaries.
    // NoLane is the default for normal boundaries, which turns into "normal" pri.
    pub retry_lane: Cell<Lane>,
    // Stashed Errors that happened while attempting to hydrate this boundary.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub hydration_errors: RefCell<Option<HydrationErrors>>,
}

unsafe impl js::Defined for SuspenseState {}

pub struct SuspenseListRenderState {
    pub is_backwards: Cell<bool>,
    // The currently rendering tail row.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub rendering: RefCell<Option<Rc<Fiber>>>,
    // The absolute time when we started rendering the most recent tail row.
    pub rendering_start_time: Cell<f64>,
    // The last of the already rendered children.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub last: RefCell<Option<Rc<Fiber>>>,
    // Remaining rows on the tail of the list.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub tail: RefCell<Option<Rc<Fiber>>>,
    // Tail insertions setting.
    pub tail_mode: Cell<Option<SuspenseListTailMode>>,
    // Keep track of total number of forks during multiple passes
    pub tree_fork_count: Cell<i32>,
}

unsafe impl js::Defined for SuspenseListRenderState {}

// shared/ReactTypes' `Wakeable`s.
pub type RetryQueue = Set<Unknown>;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_first_suspended(row: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    let mut node = Rc::clone(row);
    loop {
        if node.tag.get() == SUSPENSE_COMPONENT {
            let state: Option<&SuspenseState> = unsafe { js::cast(node.memoized_state.get()) };
            if let Some(state) = state {
                let dehydrated = state.dehydrated;
                if dehydrated.is_none_or(|dehydrated| {
                    is_suspense_instance_pending(dehydrated)
                        || is_suspense_instance_fallback(dehydrated)
                }) {
                    return Some(node);
                }
            }
        } else if node.tag.get() == SUSPENSE_LIST_COMPONENT
            // Independent revealOrder can't be trusted because it doesn't
            // keep track of whether it suspended or not.
            && !js::strict_equals(js::get(node.memoized_props.get(), "revealOrder"), "independent")
        {
            let did_suspend = (node.flags.get() & DID_CAPTURE) != NO_FLAGS;
            if did_suspend {
                return Some(node);
            }
        } else {
            let child = node.child.borrow().clone();
            if let Some(child) = child {
                child.r#return.replace(Some(Rc::clone(&node)));
                node = child;
                continue;
            }
        }
        if Rc::ptr_eq(&node, row) {
            return None;
        }
        while node.sibling.borrow().is_none() {
            let return_fiber = node.r#return.borrow().clone();
            match return_fiber {
                Some(return_fiber) if !Rc::ptr_eq(&return_fiber, row) => node = return_fiber,
                _ => return None,
            }
        }
        let sibling = node.sibling.borrow().clone().unwrap();
        let return_fiber = node.r#return.borrow().clone();
        sibling.r#return.replace(return_fiber);
        node = sibling;
    }
}
