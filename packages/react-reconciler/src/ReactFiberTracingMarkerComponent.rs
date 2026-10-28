// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Map, Set};
use react_isomorphic::react_start_transition::Transition;
use shared::react_feature_flags::ENABLE_TRANSITION_TRACING;

use crate::react_fiber_offscreen_component::OffscreenInstance;
use crate::react_fiber_stack::{StackCursor, create_cursor, pop, push};
use crate::react_fiber_work_loop::get_work_in_progress_transitions;
use crate::react_internal_types::{
    Fiber, FiberRoot, TransitionTracingCallbacks, TransitionTracingDeletion,
};

pub struct SuspenseInfo {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub name: Option<String>,
}

/// A tracing marker's progress: the boundaries it waits for, and its
/// transitions.
pub struct MarkerProgress {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending_boundaries: Option<&'static PendingBoundaries>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transitions: Option<&'static Set<Transition>>,
}

/// Why a tracing marker didn't complete, and its transitions.
pub struct MarkerIncomplete {
    pub aborts: TransitionAborts,
    pub transitions: &'static Set<Transition>,
}

pub struct PendingTransitionCallbacks {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transition_start: Option<Vec<&'static Transition>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transition_progress: Option<&'static Map<Transition, &'static PendingBoundaries>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transition_complete: Option<Vec<&'static Transition>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub marker_progress: Option<&'static Map<str, MarkerProgress>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub marker_incomplete: Option<&'static Map<str, MarkerIncomplete>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub marker_complete: Option<&'static Map<str, &'static Set<Transition>>>,
}

// TODO: Is there a way to not include the tag or name here?
pub struct TracingMarkerInstance {
    pub tag: Option<TracingMarkerTag>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub transitions: Cell<Option<&'static Set<Transition>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending_boundaries: Cell<Option<&'static PendingBoundaries>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub aborts: RefCell<Option<TransitionAborts>>,
    pub name: RefCell<Option<String>>,
}

unsafe impl js::Defined for TracingMarkerInstance {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransitionAbortReason {
    #[cfg_attr(rust_js, rust_js::name = "error")]
    Error,
    #[cfg_attr(rust_js, rust_js::name = "unknown")]
    Unknown,
    #[cfg_attr(rust_js, rust_js::name = "marker")]
    Marker,
    #[cfg_attr(rust_js, rust_js::name = "suspense")]
    Suspense,
}

/// A tracing marker's or a root's aborts: shared with the pending
/// marker-incomplete callback, which sees later aborts too.
pub type TransitionAborts = Rc<RefCell<Vec<Rc<TransitionAbort>>>>;

pub struct TransitionAbort {
    pub reason: TransitionAbortReason,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub name: Option<String>,
}

#[cfg_attr(rust_js, rust_js::name = "TransitionRoot")]
pub const TRANSITION_ROOT: TracingMarkerTag = 0;
#[cfg_attr(rust_js, rust_js::name = "TransitionTracingMarker")]
pub const TRANSITION_TRACING_MARKER: TracingMarkerTag = 1;
pub type TracingMarkerTag = u8;

pub type PendingBoundaries = Map<OffscreenInstance, SuspenseInfo>;

pub fn process_transition_callbacks(
    pending_transitions: &PendingTransitionCallbacks,
    end_time: f64,
    callbacks: &TransitionTracingCallbacks,
) {
    if ENABLE_TRANSITION_TRACING {
        let transition_start = &pending_transitions.transition_start;
        let on_transition_start = callbacks.on_transition_start;
        if let Some(transition_start) = transition_start
            && let Some(on_transition_start) = on_transition_start
        {
            transition_start.iter().for_each(|transition| {
                if let Some(name) = &transition.name {
                    on_transition_start(name, transition.start_time.get().unwrap());
                }
            });
        }

        let marker_progress = pending_transitions.marker_progress;
        let on_marker_progress = callbacks.on_marker_progress;
        if let Some(on_marker_progress) = on_marker_progress
            && let Some(marker_progress) = marker_progress
        {
            marker_progress.for_each(|marker_instance, marker_name| {
                if let Some(transitions) = marker_instance.transitions {
                    // TODO: Clone the suspense object so users can't modify it
                    let pending: Vec<SuspenseInfo> = match marker_instance.pending_boundaries {
                        Some(pending_boundaries) => pending_boundaries.values().collect(),
                        None => Vec::new(),
                    };
                    transitions.for_each(|transition| {
                        if let Some(name) = &transition.name {
                            on_marker_progress(
                                name,
                                marker_name,
                                transition.start_time.get().unwrap(),
                                end_time,
                                &pending,
                            );
                        }
                    });
                }
            });
        }

        let marker_complete = pending_transitions.marker_complete;
        let on_marker_complete = callbacks.on_marker_complete;
        if let Some(marker_complete) = marker_complete
            && let Some(on_marker_complete) = on_marker_complete
        {
            marker_complete.for_each(|transitions, marker_name| {
                transitions.for_each(|transition| {
                    if let Some(name) = &transition.name {
                        on_marker_complete(
                            name,
                            marker_name,
                            transition.start_time.get().unwrap(),
                            end_time,
                        );
                    }
                });
            });
        }

        let marker_incomplete = pending_transitions.marker_incomplete;
        let on_marker_incomplete = callbacks.on_marker_incomplete;
        if let Some(on_marker_incomplete) = on_marker_incomplete
            && let Some(marker_incomplete) = marker_incomplete
        {
            marker_incomplete.for_each(
                |MarkerIncomplete {
                     transitions,
                     aborts,
                 },
                 marker_name| {
                    transitions.for_each(|transition| {
                        let mut filtered_aborts = Vec::new();
                        aborts.borrow().iter().for_each(|abort| match abort.reason {
                            TransitionAbortReason::Marker => {
                                filtered_aborts.push(TransitionTracingDeletion {
                                    r#type: "marker".to_string(),
                                    name: abort.name.clone(),
                                    end_time,
                                });
                            }
                            TransitionAbortReason::Suspense => {
                                filtered_aborts.push(TransitionTracingDeletion {
                                    r#type: "suspense".to_string(),
                                    name: abort.name.clone(),
                                    end_time,
                                });
                            }
                            _ => {}
                        });

                        if !filtered_aborts.is_empty()
                            && let Some(name) = &transition.name
                        {
                            on_marker_incomplete(
                                name,
                                marker_name,
                                transition.start_time.get().unwrap(),
                                &filtered_aborts,
                            );
                        }
                    });
                },
            );
        }

        let transition_progress = pending_transitions.transition_progress;
        let on_transition_progress = callbacks.on_transition_progress;
        if let Some(on_transition_progress) = on_transition_progress
            && let Some(transition_progress) = transition_progress
        {
            transition_progress.for_each(|pending, transition| {
                if let Some(name) = &transition.name {
                    on_transition_progress(
                        name,
                        transition.start_time.get().unwrap(),
                        end_time,
                        &pending.values().collect::<Vec<_>>(),
                    );
                }
            });
        }

        let transition_complete = &pending_transitions.transition_complete;
        let on_transition_complete = callbacks.on_transition_complete;
        if let Some(transition_complete) = transition_complete
            && let Some(on_transition_complete) = on_transition_complete
        {
            transition_complete.iter().for_each(|transition| {
                if let Some(name) = &transition.name {
                    on_transition_complete(name, transition.start_time.get().unwrap(), end_time);
                }
            });
        }
    }
}

thread_local! {
    // For every tracing marker, store a pointer to it. We will later access it
    // to get the set of suspense boundaries that need to resolve before the
    // tracing marker can be logged as complete
    // This code lives separate from the ReactFiberTransition code because
    // we push and pop on the tracing marker, not the suspense boundary
    #[cfg_attr(rust_js, rust_js::nullable)]
    static MARKER_INSTANCE_STACK: StackCursor<Option<Rc<Vec<Rc<TracingMarkerInstance>>>>> =
        create_cursor(None);
}

pub fn push_root_marker_instance(work_in_progress: &Rc<Fiber>) {
    if ENABLE_TRANSITION_TRACING {
        // On the root, every transition gets mapped to it's own map of
        // suspense boundaries. The transition is marked as complete when
        // the suspense boundaries map is empty. We do this because every
        // transition completes at different times and depends on different
        // suspense boundaries to complete. We store all the transitions
        // along with its map of suspense boundaries in the root incomplete
        // transitions map. Each entry in this map functions like a tracing
        // marker does, so we can push it onto the marker instance stack
        let transitions = get_work_in_progress_transitions();
        let root: Rc<FiberRoot> = unsafe { js::cast(work_in_progress.state_node.get()) };

        if let Some(transitions) = transitions {
            transitions.iter().for_each(|transition| {
                if !root.incomplete_transitions.has(transition) {
                    let marker_instance = Rc::new(TracingMarkerInstance {
                        tag: Some(TRANSITION_ROOT),
                        transitions: Cell::new(Some(js::set::new(Some(&[*transition])))),
                        pending_boundaries: Cell::new(None),
                        aborts: RefCell::new(None),
                        name: RefCell::new(js::null()),
                    });
                    root.incomplete_transitions.set(transition, marker_instance);
                }
            });
        }

        let mut marker_instances = Vec::new();
        // For ever transition on the suspense boundary, we push the transition
        // along with its map of pending suspense boundaries onto the marker
        // instance stack.
        root.incomplete_transitions.for_each(|marker_instance, _| {
            marker_instances.push(marker_instance);
        });
        MARKER_INSTANCE_STACK.with(|marker_instance_stack| {
            push(
                marker_instance_stack,
                Some(Rc::new(marker_instances)),
                work_in_progress,
            );
        });
    }
}

pub fn pop_root_marker_instance(work_in_progress: &Fiber) {
    if ENABLE_TRANSITION_TRACING {
        MARKER_INSTANCE_STACK
            .with(|marker_instance_stack| pop(marker_instance_stack, work_in_progress));
    }
}

pub fn push_marker_instance(
    work_in_progress: &Rc<Fiber>,
    marker_instance: Rc<TracingMarkerInstance>,
) {
    if ENABLE_TRANSITION_TRACING {
        MARKER_INSTANCE_STACK.with(|marker_instance_stack| {
            let current = marker_instance_stack.current.borrow().clone();
            match current {
                None => push(
                    marker_instance_stack,
                    Some(Rc::new(vec![marker_instance])),
                    work_in_progress,
                ),
                Some(current) => push(
                    marker_instance_stack,
                    Some(Rc::new([&current[..], &[marker_instance]].concat())),
                    work_in_progress,
                ),
            }
        });
    }
}

pub fn pop_marker_instance(work_in_progress: &Fiber) {
    if ENABLE_TRANSITION_TRACING {
        MARKER_INSTANCE_STACK
            .with(|marker_instance_stack| pop(marker_instance_stack, work_in_progress));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_marker_instances() -> Option<Rc<Vec<Rc<TracingMarkerInstance>>>> {
    if ENABLE_TRANSITION_TRACING {
        return MARKER_INSTANCE_STACK
            .with(|marker_instance_stack| marker_instance_stack.current.borrow().clone());
    }
    None
}
