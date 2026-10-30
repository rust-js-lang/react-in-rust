// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_isomorphic::react_start_transition::{GestureProvider, GestureTransitionOptions};
use react_isomorphic::react_transition_type::TransitionTypes;

use crate::react_fiber_lane::{
    GESTURE_LANE, Lane, NO_LANE, NO_LANES, mark_root_entangled, mark_root_finished,
};
use crate::react_fiber_root_scheduler::{ensure_root_is_scheduled, request_transition_lane};
use crate::react_fiber_work_loop::{ping_gesture_root, restart_gesture_root};
use crate::react_internal_types::FiberRoot;

// ReactFiberConfig's `GestureTimeline`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getCurrentGestureOffset"
)]
fn get_current_gesture_offset(provider: &GestureProvider) -> f64 {
    unreachable!()
}

// ReactFiberConfig's `RunningViewTransition`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#stopViewTransition"
)]
fn stop_view_transition(transition: &'static Unknown) {
    unreachable!()
}

// This type keeps track of any scheduled or active gestures.
pub struct ScheduledGesture {
    // ReactFiberConfig's `GestureTimeline`.
    pub provider: &'static GestureProvider,
    pub count: Cell<i32>, // The number of times this same provider has been started.
    pub range_start: Cell<f64>, // The percentage along the timeline where the "current" state starts.
    pub range_end: Cell<f64>, // The percentage along the timeline where the "destination" state is reached.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub types: RefCell<Option<Rc<RefCell<TransitionTypes>>>>, // Any addTransitionType call made during startGestureTransition.
    // ReactFiberConfig's `RunningViewTransition`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub running: Cell<Option<&'static Unknown>>, // Used to cancel the running transition after we're done.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub commit: RefCell<Option<&'static dyn Fn()>>, // Callback to run to commit if there's a pending commit.
    pub committing: Cell<bool>, // If the gesture was released in a committed state and should actually commit.
    pub revert_lane: Cell<Lane>, // The Lane that we'll use to schedule the revert.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub prev: RefCell<Option<Rc<ScheduledGesture>>>, // The previous scheduled gesture in the queue for this root.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub next: RefCell<Option<Rc<ScheduledGesture>>>, // The next scheduled gesture in the queue for this root.
}

pub fn schedule_gesture(
    root: &Rc<FiberRoot>,
    provider: &'static GestureProvider,
) -> Rc<ScheduledGesture> {
    let mut prev = root.pending_gestures.borrow().clone();
    while let Some(gesture) = &prev {
        if js::strict_equals(gesture.provider, provider) {
            // Existing instance found.
            return Rc::clone(gesture);
        }
        let next = gesture.next.borrow().clone();
        // $FlowFixMe[invalid-compare]
        let Some(next) = next else {
            break;
        };
        prev = Some(next);
    }
    let gesture = Rc::new(ScheduledGesture {
        provider,
        count: Cell::new(0),
        range_start: Cell::new(0.0), // Uninitialized
        range_end: Cell::new(100.0), // Uninitialized
        types: RefCell::new(js::null()),
        running: Cell::new(js::null()),
        commit: RefCell::new(js::null()),
        committing: Cell::new(false),
        revert_lane: Cell::new(NO_LANE), // Starts uninitialized.
        prev: RefCell::new(prev.clone()),
        next: RefCell::new(js::null()),
    });
    match &prev {
        None => {
            root.pending_gestures.replace(Some(Rc::clone(&gesture)));
        }
        Some(prev) => {
            prev.next.replace(Some(Rc::clone(&gesture)));
        }
    }
    ensure_root_is_scheduled(root);
    gesture
}

#[cfg_attr(rust_js, rust_js::nullable)]
#[cfg_attr(rust_js, rust_js::nullable(transition_types))]
pub fn start_scheduled_gesture(
    root: &FiberRoot,
    gesture_timeline: &GestureProvider,
    gesture_options: Option<&GestureTransitionOptions>,
    transition_types: Option<Rc<RefCell<TransitionTypes>>>,
) -> Option<Rc<ScheduledGesture>> {
    let range_start = gesture_options
        .and_then(|gesture_options| gesture_options.range_start)
        .unwrap_or_else(|| get_current_gesture_offset(gesture_timeline));
    let range_end = gesture_options
        .and_then(|gesture_options| gesture_options.range_end)
        .unwrap_or(if range_start < 50.0 { 100.0 } else { 0.0 });
    let mut prev = root.pending_gestures.borrow().clone();
    while let Some(gesture) = &prev {
        if js::strict_equals(gesture.provider, gesture_timeline) {
            // Existing instance found.
            gesture.count.set(gesture.count.get() + 1);
            // Update the options.
            gesture.range_start.set(range_start);
            gesture.range_end.set(range_end);
            if let Some(transition_types) = transition_types {
                let scheduled_types = Rc::clone(
                    gesture
                        .types
                        .borrow_mut()
                        .get_or_insert_with(|| Rc::new(RefCell::new(Vec::new()))),
                );
                for transition_type in transition_types.borrow().iter() {
                    if !scheduled_types.borrow().contains(transition_type) {
                        scheduled_types.borrow_mut().push(transition_type.clone());
                    }
                }
            }
            return prev;
        }
        let next = gesture.next.borrow().clone();
        // $FlowFixMe[invalid-compare]
        let Some(next) = next else {
            break;
        };
        prev = Some(next);
    }
    // No scheduled gestures. It must mean nothing for this renderer updated but
    // some other renderer might have updated.
    None
}

pub fn cancel_scheduled_gesture(root: &Rc<FiberRoot>, gesture: &Rc<ScheduledGesture>) {
    // Entangle any Transitions started in this event with the revertLane of the gesture
    // so that we commit them all together.
    if gesture.revert_lane.get() != NO_LANE {
        let entangled_lanes = gesture.revert_lane.get() | request_transition_lane(None);
        mark_root_entangled(root, entangled_lanes);
    }

    gesture.count.set(gesture.count.get() - 1);
    if gesture.count.get() == 0 {
        // If the end state is closer to the end than the beginning then we commit into the
        // end state before reverting back (or applying a new Transition).
        // Otherwise we just revert back and don't commit.
        let final_offset = get_current_gesture_offset(gesture.provider);
        let range_start = gesture.range_start.get();
        let range_end = gesture.range_end.get();
        let should_commit = if range_start < range_end {
            final_offset > range_start + (range_end - range_start) / 2.0
        } else {
            final_offset < range_end + (range_start - range_end) / 2.0
        };
        // TODO: If we're currently rendering this gesture, we need to restart the render
        // on a different gesture or cancel the render..
        // TODO: We might want to pause the View Transition at this point since you should
        // no longer be able to update the position of anything but it might be better to
        // just commit the gesture state.
        let running_transition = gesture.running.get();
        if running_transition.is_some() && should_commit {
            // If we are going to commit this gesture in its to state, we need to wait to
            // stop it until it commits. We should now schedule a render at the gesture
            // lane to actually commit it.
            gesture.committing.set(true);
            if root
                .pending_gestures
                .borrow()
                .as_ref()
                .is_some_and(|pending_gestures| Rc::ptr_eq(pending_gestures, gesture))
            {
                let commit_callback = gesture.commit.take();
                if let Some(commit_callback) = commit_callback {
                    // If we already have a commit prepared we can immediately commit the tree
                    // without rerendering.
                    // TODO: Consider scheduling this in a task instead of synchronously inside the last cancellation.s
                    commit_callback();
                } else {
                    // Ping the root given the new state. This is similar to pingSuspendedRoot.
                    ping_gesture_root(root);
                }
            }
        } else {
            // If we're not going to commit this gesture we can stop the View Transition
            // right away and delete the scheduled gesture from the pending queue.
            let gesture_prev = gesture.prev.borrow().clone();
            match gesture_prev {
                None => {
                    if root
                        .pending_gestures
                        .borrow()
                        .as_ref()
                        .is_some_and(|pending_gestures| Rc::ptr_eq(pending_gestures, gesture))
                    {
                        // This was the currently rendering gesture.
                        let next = gesture.next.borrow().clone();
                        root.pending_gestures.replace(next);
                        let mut remaining_lanes = root.pending_lanes.get();
                        if root.pending_gestures.borrow().is_none() {
                            // Gestures don't clear their lanes while the gesture is still active but it
                            // might not be scheduled to do any more renders and so we shouldn't schedule
                            // any more gesture lane work until a new gesture is scheduled.
                            remaining_lanes &= !GESTURE_LANE;
                        }
                        mark_root_finished(
                            root,
                            GESTURE_LANE,
                            remaining_lanes,
                            NO_LANE,
                            NO_LANE,
                            NO_LANES,
                        );
                        // If we had a currently rendering gesture we need to now reset the gesture lane to
                        // now render the next gesture or cancel if there's no more gestures in the queue.
                        restart_gesture_root(root);
                    }
                    gesture.running.set(js::null());
                    if let Some(running_transition) = running_transition {
                        stop_view_transition(running_transition);
                    }
                }
                Some(gesture_prev) => {
                    // This was not the current gesture so it doesn't affect the current render.
                    let next = gesture.next.borrow().clone();
                    gesture_prev.next.replace(next);
                    let next = gesture.next.borrow().clone();
                    if let Some(next) = next {
                        next.prev.replace(Some(gesture_prev));
                    }
                    gesture.prev.replace(js::null());
                    gesture.next.replace(js::null());
                }
            }
        }
    }
}

pub fn stop_committed_gesture(root: &FiberRoot) {
    // The top was just committed. We can delete it from the queue
    // and stop its View Transition now.
    let committed_gesture = root.pending_gestures.borrow().clone();
    if let Some(committed_gesture) = committed_gesture {
        // Mark it as no longer committing and should no longer be included in rerenders.
        committed_gesture.committing.set(false);
        let next_gesture = committed_gesture.next.borrow().clone();
        match &next_gesture {
            None => {
                // Gestures don't clear their lanes while the gesture is still active but it
                // might not be scheduled to do any more renders and so we shouldn't schedule
                // any more gesture lane work until a new gesture is scheduled.
                root.pending_lanes
                    .set(root.pending_lanes.get() & !GESTURE_LANE);
            }
            Some(next_gesture) => {
                next_gesture.prev.replace(js::null());
            }
        }
        root.pending_gestures.replace(next_gesture);
        let running_transition = committed_gesture.running.get();
        if let Some(running_transition) = running_transition {
            committed_gesture.running.set(js::null());
            stop_view_transition(running_transition);
        }
    }
}

pub fn schedule_gesture_commit(
    gesture: &Rc<ScheduledGesture>,
    callback: &'static dyn Fn(),
) -> Box<dyn Fn()> {
    gesture.commit.replace(Some(callback));
    let gesture = Rc::clone(gesture);
    Box::new(move || {
        gesture.commit.replace(js::null());
    })
}
