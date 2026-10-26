// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, ENABLE_DEFAULT_TRANSITION_INDICATOR, ENABLE_GESTURE_TRANSITION,
    ENABLE_RETRY_LANE_EXPIRATION, ENABLE_SCHEDULING_PROFILER, ENABLE_TRANSITION_TRACING,
    ENABLE_UPDATER_TRACKING, RETRY_LANE_EXPIRATION_MS, SYNC_LANE_EXPIRATION_MS,
    TRANSITION_LANE_EXPIRATION_MS,
};
use webapi::console;

use crate::clz32::CLZ32;
use crate::react_fiber_concurrent_updates::ConcurrentUpdate;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_root_tags::LEGACY_ROOT;

// TODO: Ideally these types would be opaque but that doesn't work well with
// our reconciler fork infra, since these leak into non-reconciler packages.

pub type Lanes = i32;
pub type Lane = i32;
pub type LaneMap<T> = Vec<T>;

unsafe extern "Rust" {
    #[cfg_attr(rust_js, rust_js::name = "isDevToolsPresent")]
    #[link_name = "./src/ReactFiberDevToolsHook.js#isDevToolsPresent"]
    safe static IS_DEV_TOOLS_PRESENT: bool;
}

// Lane values below should be kept in sync with getLabelForLane(), used by react-devtools-timeline.
// If those values are changed that package should be rebuilt and redeployed.

#[cfg_attr(rust_js, rust_js::name = "TotalLanes")]
pub const TOTAL_LANES: usize = 31;

#[cfg_attr(rust_js, rust_js::name = "NoLanes")]
pub const NO_LANES: Lanes = /*                        */ 0b0000000000000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "NoLane")]
pub const NO_LANE: Lane = /*                          */ 0b0000000000000000000000000000000;

#[cfg_attr(rust_js, rust_js::name = "SyncHydrationLane")]
pub const SYNC_HYDRATION_LANE: Lane = /*               */ 0b0000000000000000000000000000001;
#[cfg_attr(rust_js, rust_js::name = "SyncLane")]
pub const SYNC_LANE: Lane = /*                        */ 0b0000000000000000000000000000010;
#[cfg_attr(rust_js, rust_js::name = "SyncLaneIndex")]
pub const SYNC_LANE_INDEX: usize = 1;

#[cfg_attr(rust_js, rust_js::name = "InputContinuousHydrationLane")]
pub const INPUT_CONTINUOUS_HYDRATION_LANE: Lane = /*    */ 0b0000000000000000000000000000100;
#[cfg_attr(rust_js, rust_js::name = "InputContinuousLane")]
pub const INPUT_CONTINUOUS_LANE: Lane = /*             */ 0b0000000000000000000000000001000;

#[cfg_attr(rust_js, rust_js::name = "DefaultHydrationLane")]
pub const DEFAULT_HYDRATION_LANE: Lane = /*            */ 0b0000000000000000000000000010000;
#[cfg_attr(rust_js, rust_js::name = "DefaultLane")]
pub const DEFAULT_LANE: Lane = /*                     */ 0b0000000000000000000000000100000;

#[cfg_attr(rust_js, rust_js::name = "SyncUpdateLanes")]
pub const SYNC_UPDATE_LANES: Lane = SYNC_LANE | INPUT_CONTINUOUS_LANE | DEFAULT_LANE;

#[cfg_attr(rust_js, rust_js::name = "GestureLane")]
pub const GESTURE_LANE: Lane = /*                     */ 0b0000000000000000000000001000000;

#[cfg_attr(rust_js, rust_js::name = "TransitionHydrationLane")]
const TRANSITION_HYDRATION_LANE: Lane = /*                */ 0b0000000000000000000000010000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLanes")]
const TRANSITION_LANES: Lanes = /*                       */ 0b0000000001111111111111100000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane1")]
const TRANSITION_LANE_1: Lane = /*                        */ 0b0000000000000000000000100000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane2")]
const TRANSITION_LANE_2: Lane = /*                        */ 0b0000000000000000000001000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane3")]
const TRANSITION_LANE_3: Lane = /*                        */ 0b0000000000000000000010000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane4")]
const TRANSITION_LANE_4: Lane = /*                        */ 0b0000000000000000000100000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane5")]
const TRANSITION_LANE_5: Lane = /*                        */ 0b0000000000000000001000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane6")]
const TRANSITION_LANE_6: Lane = /*                        */ 0b0000000000000000010000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane7")]
const TRANSITION_LANE_7: Lane = /*                        */ 0b0000000000000000100000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane8")]
const TRANSITION_LANE_8: Lane = /*                        */ 0b0000000000000001000000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane9")]
const TRANSITION_LANE_9: Lane = /*                        */ 0b0000000000000010000000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane10")]
const TRANSITION_LANE_10: Lane = /*                       */ 0b0000000000000100000000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane11")]
const TRANSITION_LANE_11: Lane = /*                       */ 0b0000000000001000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane12")]
const TRANSITION_LANE_12: Lane = /*                       */ 0b0000000000010000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane13")]
const TRANSITION_LANE_13: Lane = /*                       */ 0b0000000000100000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "TransitionLane14")]
const TRANSITION_LANE_14: Lane = /*                       */ 0b0000000001000000000000000000000;

#[cfg_attr(rust_js, rust_js::name = "SomeTransitionLane")]
pub const SOME_TRANSITION_LANE: Lane = TRANSITION_LANE_1;

#[cfg_attr(rust_js, rust_js::name = "TransitionUpdateLanes")]
const TRANSITION_UPDATE_LANES: Lanes = TRANSITION_LANE_1
    | TRANSITION_LANE_2
    | TRANSITION_LANE_3
    | TRANSITION_LANE_4
    | TRANSITION_LANE_5
    | TRANSITION_LANE_6
    | TRANSITION_LANE_7
    | TRANSITION_LANE_8
    | TRANSITION_LANE_9
    | TRANSITION_LANE_10;
#[cfg_attr(rust_js, rust_js::name = "TransitionDeferredLanes")]
const TRANSITION_DEFERRED_LANES: Lanes =
    TRANSITION_LANE_11 | TRANSITION_LANE_12 | TRANSITION_LANE_13 | TRANSITION_LANE_14;

#[cfg_attr(rust_js, rust_js::name = "RetryLanes")]
const RETRY_LANES: Lanes = /*                            */ 0b0000011110000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "RetryLane1")]
const RETRY_LANE_1: Lane = /*                             */ 0b0000000010000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "RetryLane2")]
const RETRY_LANE_2: Lane = /*                             */ 0b0000000100000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "RetryLane3")]
const RETRY_LANE_3: Lane = /*                             */ 0b0000001000000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "RetryLane4")]
const RETRY_LANE_4: Lane = /*                             */ 0b0000010000000000000000000000000;

#[cfg_attr(rust_js, rust_js::name = "SomeRetryLane")]
pub const SOME_RETRY_LANE: Lane = RETRY_LANE_1;

#[cfg_attr(rust_js, rust_js::name = "SelectiveHydrationLane")]
pub const SELECTIVE_HYDRATION_LANE: Lane = /*          */ 0b0000100000000000000000000000000;

#[cfg_attr(rust_js, rust_js::name = "NonIdleLanes")]
const NON_IDLE_LANES: Lanes = /*                          */ 0b0000111111111111111111111111111;

#[cfg_attr(rust_js, rust_js::name = "IdleHydrationLane")]
pub const IDLE_HYDRATION_LANE: Lane = /*               */ 0b0001000000000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "IdleLane")]
pub const IDLE_LANE: Lane = /*                        */ 0b0010000000000000000000000000000;

#[cfg_attr(rust_js, rust_js::name = "OffscreenLane")]
pub const OFFSCREEN_LANE: Lane = /*                   */ 0b0100000000000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "DeferredLane")]
pub const DEFERRED_LANE: Lane = /*                    */ 0b1000000000000000000000000000000;

// Any lane that might schedule an update. This is used to detect infinite
// update loops, so it doesn't include hydration lanes or retries.
#[cfg_attr(rust_js, rust_js::name = "UpdateLanes")]
pub const UPDATE_LANES: Lanes =
    SYNC_LANE | INPUT_CONTINUOUS_LANE | DEFAULT_LANE | TRANSITION_UPDATE_LANES;

#[cfg_attr(rust_js, rust_js::name = "HydrationLanes")]
pub const HYDRATION_LANES: Lanes = SYNC_HYDRATION_LANE
    | INPUT_CONTINUOUS_HYDRATION_LANE
    | DEFAULT_HYDRATION_LANE
    | TRANSITION_HYDRATION_LANE
    | SELECTIVE_HYDRATION_LANE
    | IDLE_HYDRATION_LANE;

// This function is used for the experimental timeline (react-devtools-timeline)
// It should be kept in sync with the Lanes values above.
pub fn get_label_for_lane(lane: Lane) -> Option<&'static str> {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        if lane & SYNC_HYDRATION_LANE != 0 {
            return Some("SyncHydrationLane");
        }
        if lane & SYNC_LANE != 0 {
            return Some("Sync");
        }
        if lane & INPUT_CONTINUOUS_HYDRATION_LANE != 0 {
            return Some("InputContinuousHydration");
        }
        if lane & INPUT_CONTINUOUS_LANE != 0 {
            return Some("InputContinuous");
        }
        if lane & DEFAULT_HYDRATION_LANE != 0 {
            return Some("DefaultHydration");
        }
        if lane & DEFAULT_LANE != 0 {
            return Some("Default");
        }
        if lane & TRANSITION_HYDRATION_LANE != 0 {
            return Some("TransitionHydration");
        }
        if lane & TRANSITION_LANES != 0 {
            return Some("Transition");
        }
        if lane & RETRY_LANES != 0 {
            return Some("Retry");
        }
        if lane & SELECTIVE_HYDRATION_LANE != 0 {
            return Some("SelectiveHydration");
        }
        if lane & IDLE_HYDRATION_LANE != 0 {
            return Some("IdleHydration");
        }
        if lane & IDLE_LANE != 0 {
            return Some("Idle");
        }
        if lane & OFFSCREEN_LANE != 0 {
            return Some("Offscreen");
        }
        if lane & DEFERRED_LANE != 0 {
            return Some("Deferred");
        }
    }
    None
}

#[cfg_attr(rust_js, rust_js::name = "NoTimestamp")]
pub const NO_TIMESTAMP: f64 = -1.0;

thread_local! {
    static NEXT_TRANSITION_UPDATE_LANE: Cell<Lane> = const { Cell::new(TRANSITION_LANE_1) };
    static NEXT_TRANSITION_DEFERRED_LANE: Cell<Lane> = const { Cell::new(TRANSITION_LANE_11) };
    static NEXT_RETRY_LANE: Cell<Lane> = const { Cell::new(RETRY_LANE_1) };
}

fn get_highest_priority_lanes(lanes: Lanes) -> Lanes {
    let pending_sync_lanes = lanes & SYNC_UPDATE_LANES;
    if pending_sync_lanes != 0 {
        return pending_sync_lanes;
    }
    match get_highest_priority_lane(lanes) {
        SYNC_HYDRATION_LANE => SYNC_HYDRATION_LANE,
        SYNC_LANE => SYNC_LANE,
        INPUT_CONTINUOUS_HYDRATION_LANE => INPUT_CONTINUOUS_HYDRATION_LANE,
        INPUT_CONTINUOUS_LANE => INPUT_CONTINUOUS_LANE,
        DEFAULT_HYDRATION_LANE => DEFAULT_HYDRATION_LANE,
        DEFAULT_LANE => DEFAULT_LANE,
        GESTURE_LANE => GESTURE_LANE,
        TRANSITION_HYDRATION_LANE => TRANSITION_HYDRATION_LANE,
        TRANSITION_LANE_1 | TRANSITION_LANE_2 | TRANSITION_LANE_3 | TRANSITION_LANE_4
        | TRANSITION_LANE_5 | TRANSITION_LANE_6 | TRANSITION_LANE_7 | TRANSITION_LANE_8
        | TRANSITION_LANE_9 | TRANSITION_LANE_10 => get_highest_priority_lane(lanes),
        TRANSITION_LANE_11 | TRANSITION_LANE_12 | TRANSITION_LANE_13 | TRANSITION_LANE_14 => {
            lanes & TRANSITION_DEFERRED_LANES
        }
        RETRY_LANE_1 | RETRY_LANE_2 | RETRY_LANE_3 | RETRY_LANE_4 => lanes & RETRY_LANES,
        SELECTIVE_HYDRATION_LANE => SELECTIVE_HYDRATION_LANE,
        IDLE_HYDRATION_LANE => IDLE_HYDRATION_LANE,
        IDLE_LANE => IDLE_LANE,
        OFFSCREEN_LANE => OFFSCREEN_LANE,
        DEFERRED_LANE => {
            // This shouldn't be reachable because deferred work is always entangled
            // with something else.
            NO_LANES
        }
        _ => {
            if __DEV__ {
                console::error(("Should have found matching lanes. This is a bug in React.",));
            }
            // This shouldn't be reachable, but as a fallback, return the entire bitmask.
            lanes
        }
    }
}

pub fn get_next_lanes(root: &FiberRoot, wip_lanes: Lanes, root_has_pending_commit: bool) -> Lanes {
    // Early bailout if there's no pending work left.
    let pending_lanes = root.pending_lanes.get();
    if pending_lanes == NO_LANES {
        return NO_LANES;
    }

    let mut next_lanes: Lanes = NO_LANES;

    let suspended_lanes = root.suspended_lanes.get();
    let pinged_lanes = root.pinged_lanes.get();
    let warm_lanes = root.warm_lanes.get();

    // finishedLanes represents a completed tree that is ready to commit.
    //
    // It's not worth doing discarding the completed tree in favor of performing
    // speculative work. So always check this before deciding to warm up
    // the siblings.
    //
    // Note that this is not set in a "suspend indefinitely" scenario, like when
    // suspending outside of a Suspense boundary, or in the shell during a
    // transition — only in cases where we are very likely to commit the tree in
    // a brief amount of time (i.e. below the "Just Noticeable Difference"
    // threshold).
    //

    // Do not work on any idle work until all the non-idle work has finished,
    // even if the work is suspended.
    let non_idle_pending_lanes = pending_lanes & NON_IDLE_LANES;
    if non_idle_pending_lanes != NO_LANES {
        // First check for fresh updates.
        let non_idle_unblocked_lanes = non_idle_pending_lanes & !suspended_lanes;
        if non_idle_unblocked_lanes != NO_LANES {
            next_lanes = get_highest_priority_lanes(non_idle_unblocked_lanes);
        } else {
            // No fresh updates. Check if suspended work has been pinged.
            let non_idle_pinged_lanes = non_idle_pending_lanes & pinged_lanes;
            if non_idle_pinged_lanes != NO_LANES {
                next_lanes = get_highest_priority_lanes(non_idle_pinged_lanes);
            } else {
                // Nothing has been pinged. Check for lanes that need to be prewarmed.
                if !root_has_pending_commit {
                    let lanes_to_prewarm = non_idle_pending_lanes & !warm_lanes;
                    if lanes_to_prewarm != NO_LANES {
                        next_lanes = get_highest_priority_lanes(lanes_to_prewarm);
                    }
                }
            }
        }
    } else {
        // The only remaining work is Idle.
        // TODO: Idle isn't really used anywhere, and the thinking around
        // speculative rendering has evolved since this was implemented. Consider
        // removing until we've thought about this again.

        // First check for fresh updates.
        let unblocked_lanes = pending_lanes & !suspended_lanes;
        if unblocked_lanes != NO_LANES {
            next_lanes = get_highest_priority_lanes(unblocked_lanes);
        } else {
            // No fresh updates. Check if suspended work has been pinged.
            if pinged_lanes != NO_LANES {
                next_lanes = get_highest_priority_lanes(pinged_lanes);
            } else {
                // Nothing has been pinged. Check for lanes that need to be prewarmed.
                if !root_has_pending_commit {
                    let lanes_to_prewarm = pending_lanes & !warm_lanes;
                    if lanes_to_prewarm != NO_LANES {
                        next_lanes = get_highest_priority_lanes(lanes_to_prewarm);
                    }
                }
            }
        }
    }

    if next_lanes == NO_LANES {
        // This should only be reachable if we're suspended
        // TODO: Consider warning in this path if a fallback timer is not scheduled.
        return NO_LANES;
    }

    // If we're already in the middle of a render, switching lanes will interrupt
    // it and we'll lose our progress. We should only do this if the new lanes are
    // higher priority.
    if wip_lanes != NO_LANES
        && wip_lanes != next_lanes
        // If we already suspended with a delay, then interrupting is fine. Don't
        // bother waiting until the root is complete.
        && (wip_lanes & suspended_lanes) == NO_LANES
    {
        let next_lane = get_highest_priority_lane(next_lanes);
        let wip_lane = get_highest_priority_lane(wip_lanes);
        if
        // Tests whether the next lane is equal or lower priority than the wip
        // one. This works because the bits decrease in priority as you go left.
        next_lane >= wip_lane
            // Default priority updates should not interrupt transition updates. The
            // only difference between default updates and transition updates is that
            // default updates do not support refresh transitions.
            || (next_lane == DEFAULT_LANE && (wip_lane & TRANSITION_LANES) != NO_LANES)
        {
            // Keep working on the existing in-progress tree. Do not interrupt.
            return wip_lanes;
        }
    }

    next_lanes
}

pub fn get_next_lanes_to_flush_sync(root: &FiberRoot, extra_lanes_to_force_sync: Lanes) -> Lanes {
    // Similar to getNextLanes, except instead of choosing the next lanes to work
    // on based on their priority, it selects all the lanes that have equal or
    // higher priority than those are given. That way they can be synchronously
    // rendered in a single batch.
    //
    // The main use case is updates scheduled by popstate events, which are
    // flushed synchronously even though they are transitions.
    // Note that we intentionally treat this as a sync flush to include any
    // sync updates in a single pass but also intentionally disables View Transitions
    // inside popstate. Because they can start synchronously before scroll restoration
    // happens.
    let lanes_to_flush = SYNC_UPDATE_LANES | extra_lanes_to_force_sync;

    // Early bailout if there's no pending work left.
    let pending_lanes = root.pending_lanes.get();
    if pending_lanes == NO_LANES {
        return NO_LANES;
    }

    let suspended_lanes = root.suspended_lanes.get();
    let pinged_lanes = root.pinged_lanes.get();

    // Remove lanes that are suspended (but not pinged)
    let unblocked_lanes = pending_lanes & !(suspended_lanes & !pinged_lanes);
    let unblocked_lanes_with_matching_priority =
        unblocked_lanes & get_lanes_of_equal_or_higher_priority(lanes_to_flush);

    // If there are matching hydration lanes, we should do those by themselves.
    // Hydration lanes must never include updates.
    if unblocked_lanes_with_matching_priority & HYDRATION_LANES != 0 {
        return (unblocked_lanes_with_matching_priority & HYDRATION_LANES) | SYNC_HYDRATION_LANE;
    }

    if unblocked_lanes_with_matching_priority != 0 {
        // Always include the SyncLane as part of the result, even if there's no
        // pending sync work, to indicate the priority of the entire batch of work
        // is considered Sync.
        return unblocked_lanes_with_matching_priority | SYNC_LANE;
    }

    NO_LANES
}

pub fn check_if_root_is_prerendering(root: &FiberRoot, render_lanes: Lanes) -> bool {
    let pending_lanes = root.pending_lanes.get();
    let suspended_lanes = root.suspended_lanes.get();
    let pinged_lanes = root.pinged_lanes.get();
    // Remove lanes that are suspended (but not pinged)
    let unblocked_lanes = pending_lanes & !(suspended_lanes & !pinged_lanes);

    // If there are no unsuspended or pinged lanes, that implies that we're
    // performing a prerender.
    (unblocked_lanes & render_lanes) == 0
}

pub fn get_entangled_lanes(root: &FiberRoot, render_lanes: Lanes) -> Lanes {
    let mut entangled_lanes = render_lanes;

    if (entangled_lanes & INPUT_CONTINUOUS_LANE) != NO_LANES {
        // When updates are sync by default, we entangle continuous priority updates
        // and default updates, so they render in the same batch. The only reason
        // they use separate lanes is because continuous updates should interrupt
        // transitions, but default updates should not.
        entangled_lanes |= entangled_lanes & DEFAULT_LANE;
    }

    // Check for entangled lanes and add them to the batch.
    //
    // A lane is said to be entangled with another when it's not allowed to render
    // in a batch that does not also include the other lane. Typically we do this
    // when multiple updates have the same source, and we only want to respond to
    // the most recent event from that source.
    //
    // Note that we apply entanglements *after* checking for partial work above.
    // This means that if a lane is entangled during an interleaved event while
    // it's already rendering, we won't interrupt it. This is intentional, since
    // entanglement is usually "best effort": we'll try our best to render the
    // lanes in the same batch, but it's not worth throwing out partially
    // completed work in order to do it.
    // TODO: Reconsider this. The counter-argument is that the partial work
    // represents an intermediate state, which we don't want to show to the user.
    // And by spending extra time finishing it, we're increasing the amount of
    // time it takes to show the final state, which is what they are actually
    // waiting for.
    //
    // For those exceptions where entanglement is semantically important,
    // we should ensure that there is no partial work at the
    // time we apply the entanglement.
    let all_entangled_lanes = root.entangled_lanes.get();
    if all_entangled_lanes != NO_LANES {
        let entanglements = root.entanglements.borrow();
        let mut lanes = entangled_lanes & all_entangled_lanes;
        while lanes > 0 {
            let index = pick_arbitrary_lane_index(lanes);
            let lane = 1 << index;

            entangled_lanes |= entanglements[index];

            lanes &= !lane;
        }
    }

    entangled_lanes
}

fn compute_expiration_time(lane: Lane, current_time: f64) -> f64 {
    match lane {
        SYNC_HYDRATION_LANE
        | SYNC_LANE
        | INPUT_CONTINUOUS_HYDRATION_LANE
        | INPUT_CONTINUOUS_LANE
        | GESTURE_LANE => {
            // User interactions should expire slightly more quickly.
            //
            // NOTE: This is set to the corresponding constant as in Scheduler.js.
            // When we made it larger, a product metric in www regressed, suggesting
            // there's a user interaction that's being starved by a series of
            // synchronous updates. If that theory is correct, the proper solution is
            // to fix the starvation. However, this scenario supports the idea that
            // expiration times are an important safeguard when starvation
            // does happen.
            current_time + SYNC_LANE_EXPIRATION_MS
        }
        DEFAULT_HYDRATION_LANE
        | DEFAULT_LANE
        | TRANSITION_HYDRATION_LANE
        | TRANSITION_LANE_1
        | TRANSITION_LANE_2
        | TRANSITION_LANE_3
        | TRANSITION_LANE_4
        | TRANSITION_LANE_5
        | TRANSITION_LANE_6
        | TRANSITION_LANE_7
        | TRANSITION_LANE_8
        | TRANSITION_LANE_9
        | TRANSITION_LANE_10
        | TRANSITION_LANE_11
        | TRANSITION_LANE_12
        | TRANSITION_LANE_13
        | TRANSITION_LANE_14 => current_time + TRANSITION_LANE_EXPIRATION_MS,
        RETRY_LANE_1 | RETRY_LANE_2 | RETRY_LANE_3 | RETRY_LANE_4 => {
            // TODO: Retries should be allowed to expire if they are CPU bound for
            // too long, but when I made this change it caused a spike in browser
            // crashes. There must be some other underlying bug; not super urgent but
            // ideally should figure out why and fix it. Unfortunately we don't have
            // a repro for the crashes, only detected via production metrics.
            if ENABLE_RETRY_LANE_EXPIRATION {
                current_time + RETRY_LANE_EXPIRATION_MS
            } else {
                NO_TIMESTAMP
            }
        }
        SELECTIVE_HYDRATION_LANE
        | IDLE_HYDRATION_LANE
        | IDLE_LANE
        | OFFSCREEN_LANE
        | DEFERRED_LANE => {
            // Anything idle priority or lower should never expire.
            NO_TIMESTAMP
        }
        _ => {
            if __DEV__ {
                console::error(("Should have found matching lanes. This is a bug in React.",));
            }
            NO_TIMESTAMP
        }
    }
}

pub fn mark_starved_lanes_as_expired(root: &FiberRoot, current_time: f64) {
    // TODO: This gets called every time we yield. We can optimize by storing
    // the earliest expiration time on the root. Then use that to quickly bail out
    // of this function.

    let pending_lanes = root.pending_lanes.get();
    let suspended_lanes = root.suspended_lanes.get();
    let pinged_lanes = root.pinged_lanes.get();
    let mut expiration_times = root.expiration_times.borrow_mut();

    // Iterate through the pending lanes and check if we've reached their
    // expiration time. If so, we'll assume the update is being starved and mark
    // it as expired to force it to finish.
    // TODO: We should be able to replace this with upgradePendingLanesToSync
    //
    // We exclude retry lanes because those must always be time sliced, in order
    // to unwrap uncached promises.
    // TODO: Write a test for this
    let mut lanes = if ENABLE_RETRY_LANE_EXPIRATION {
        pending_lanes
    } else {
        pending_lanes & !RETRY_LANES
    };
    while lanes > 0 {
        let index = pick_arbitrary_lane_index(lanes);
        let lane = 1 << index;

        let expiration_time = expiration_times[index];
        if expiration_time == NO_TIMESTAMP {
            // Found a pending lane with no expiration time. If it's not suspended, or
            // if it's pinged, assume it's CPU-bound. Compute a new expiration time
            // using the current time.
            if (lane & suspended_lanes) == NO_LANES || (lane & pinged_lanes) != NO_LANES {
                // Assumes timestamps are monotonically increasing.
                expiration_times[index] = compute_expiration_time(lane, current_time);
            }
        } else if expiration_time <= current_time {
            // This lane expired
            root.expired_lanes.set(root.expired_lanes.get() | lane);
        }

        lanes &= !lane;
    }
}

// This returns the highest priority pending lanes regardless of whether they
// are suspended.
pub fn get_highest_priority_pending_lanes(root: &FiberRoot) -> Lanes {
    get_highest_priority_lanes(root.pending_lanes.get())
}

pub fn get_lanes_to_retry_synchronously_on_error(
    root: &FiberRoot,
    originally_attempted_lanes: Lanes,
) -> Lanes {
    if root.error_recovery_disabled_lanes.get() & originally_attempted_lanes != 0 {
        // The error recovery mechanism is disabled until these lanes are cleared.
        return NO_LANES;
    }

    let everything_but_offscreen = root.pending_lanes.get() & !OFFSCREEN_LANE;
    if everything_but_offscreen != NO_LANES {
        return everything_but_offscreen;
    }
    if everything_but_offscreen & OFFSCREEN_LANE != 0 {
        return OFFSCREEN_LANE;
    }
    NO_LANES
}

pub fn includes_sync_lane(lanes: Lanes) -> bool {
    (lanes & (SYNC_LANE | SYNC_HYDRATION_LANE)) != NO_LANES
}

pub fn includes_non_idle_work(lanes: Lanes) -> bool {
    (lanes & NON_IDLE_LANES) != NO_LANES
}
pub fn includes_only_retries(lanes: Lanes) -> bool {
    (lanes & RETRY_LANES) == lanes
}
pub fn includes_only_non_urgent_lanes(lanes: Lanes) -> bool {
    // TODO: Should hydration lanes be included here? This function is only
    // used in `updateDeferredValueImpl`.
    #[cfg_attr(rust_js, rust_js::name = "UrgentLanes")]
    const URGENT_LANES: Lanes = SYNC_LANE | INPUT_CONTINUOUS_LANE | DEFAULT_LANE | GESTURE_LANE;
    (lanes & URGENT_LANES) == NO_LANES
}
pub fn includes_only_transitions(lanes: Lanes) -> bool {
    (lanes & TRANSITION_LANES) == lanes
}

pub fn includes_transition_lane(lanes: Lanes) -> bool {
    (lanes & TRANSITION_LANES) != NO_LANES
}

pub fn includes_retry_lane(lanes: Lanes) -> bool {
    (lanes & RETRY_LANES) != NO_LANES
}

pub fn includes_idle_group_lanes(lanes: Lanes) -> bool {
    (lanes
        & (SELECTIVE_HYDRATION_LANE
            | IDLE_HYDRATION_LANE
            | IDLE_LANE
            | OFFSCREEN_LANE
            | DEFERRED_LANE))
        != NO_LANES
}

pub fn includes_only_hydration_lanes(lanes: Lanes) -> bool {
    (lanes & HYDRATION_LANES) == lanes
}

pub fn includes_only_offscreen_lanes(lanes: Lanes) -> bool {
    (lanes & OFFSCREEN_LANE) == lanes
}

pub fn includes_only_hydration_or_offscreen_lanes(lanes: Lanes) -> bool {
    (lanes & (HYDRATION_LANES | OFFSCREEN_LANE)) == lanes
}

pub fn includes_only_view_transition_eligible_lanes(lanes: Lanes) -> bool {
    (lanes & (TRANSITION_LANES | RETRY_LANES | IDLE_LANE)) == lanes
}

pub fn includes_only_suspensey_commit_eligible_lanes(lanes: Lanes) -> bool {
    (lanes & (TRANSITION_LANES | RETRY_LANES | IDLE_LANE | GESTURE_LANE)) == lanes
}

pub fn includes_loading_indicator_lanes(lanes: Lanes) -> bool {
    (lanes & (SYNC_LANE | DEFAULT_LANE)) != NO_LANES
}

pub fn includes_blocking_lane(lanes: Lanes) -> bool {
    #[cfg_attr(rust_js, rust_js::name = "SyncDefaultLanes")]
    const SYNC_DEFAULT_LANES: Lanes = SYNC_HYDRATION_LANE
        | SYNC_LANE
        | INPUT_CONTINUOUS_HYDRATION_LANE
        | INPUT_CONTINUOUS_LANE
        | DEFAULT_HYDRATION_LANE
        | DEFAULT_LANE
        | GESTURE_LANE;
    (lanes & SYNC_DEFAULT_LANES) != NO_LANES
}

pub fn includes_expired_lane(root: &FiberRoot, lanes: Lanes) -> bool {
    // This is a separate check from includesBlockingLane because a lane can
    // expire after a render has already started.
    (lanes & root.expired_lanes.get()) != NO_LANES
}

pub fn is_blocking_lane(lane: Lane) -> bool {
    #[cfg_attr(rust_js, rust_js::name = "SyncDefaultLanes")]
    const SYNC_DEFAULT_LANES: Lanes = SYNC_HYDRATION_LANE
        | SYNC_LANE
        | INPUT_CONTINUOUS_HYDRATION_LANE
        | INPUT_CONTINUOUS_LANE
        | DEFAULT_HYDRATION_LANE
        | DEFAULT_LANE
        | GESTURE_LANE;
    (lane & SYNC_DEFAULT_LANES) != NO_LANES
}

pub fn is_transition_lane(lane: Lane) -> bool {
    (lane & TRANSITION_LANES) != NO_LANES
}

pub fn is_gesture_render(lanes: Lanes) -> bool {
    if !ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        return false;
    }
    // This should render only the one lane.
    lanes == GESTURE_LANE
}

pub fn claim_next_transition_update_lane() -> Lane {
    // Cycle through the lanes, assigning each new transition to the next lane.
    // In most cases, this means every transition gets its own lane, until we
    // run out of lanes and cycle back to the beginning.
    let lane = NEXT_TRANSITION_UPDATE_LANE.get();
    NEXT_TRANSITION_UPDATE_LANE.set(NEXT_TRANSITION_UPDATE_LANE.get() << 1);
    if (NEXT_TRANSITION_UPDATE_LANE.get() & TRANSITION_UPDATE_LANES) == NO_LANES {
        NEXT_TRANSITION_UPDATE_LANE.set(TRANSITION_LANE_1);
    }
    lane
}

pub fn claim_next_transition_deferred_lane() -> Lane {
    let lane = NEXT_TRANSITION_DEFERRED_LANE.get();
    NEXT_TRANSITION_DEFERRED_LANE.set(NEXT_TRANSITION_DEFERRED_LANE.get() << 1);
    if (NEXT_TRANSITION_DEFERRED_LANE.get() & TRANSITION_DEFERRED_LANES) == NO_LANES {
        NEXT_TRANSITION_DEFERRED_LANE.set(TRANSITION_LANE_11);
    }
    lane
}

pub fn claim_next_retry_lane() -> Lane {
    let lane = NEXT_RETRY_LANE.get();
    NEXT_RETRY_LANE.set(NEXT_RETRY_LANE.get() << 1);
    if (NEXT_RETRY_LANE.get() & RETRY_LANES) == NO_LANES {
        NEXT_RETRY_LANE.set(RETRY_LANE_1);
    }
    lane
}

pub fn get_highest_priority_lane(lanes: Lanes) -> Lane {
    lanes.isolate_lowest_one()
}

fn get_lanes_of_equal_or_higher_priority(lanes: Lanes) -> Lanes {
    // Create a mask with all bits to the right or same as the highest bit.
    // So if lanes is 0b100, the result would be 0b111.
    // If lanes is 0b101, the result would be 0b111.
    let lowest_priority_lane_index = 31 - CLZ32.with(|clz32| clz32(lanes as u32));
    (1 << (lowest_priority_lane_index + 1)) - 1
}

pub fn pick_arbitrary_lane(lanes: Lanes) -> Lane {
    // This wrapper function gets inlined. Only exists so to communicate that it
    // doesn't matter which bit is selected; you can pick any bit without
    // affecting the algorithms where its used. Here I'm using
    // getHighestPriorityLane because it requires the fewest operations.
    get_highest_priority_lane(lanes)
}

fn pick_arbitrary_lane_index(lanes: Lanes) -> usize {
    31 - CLZ32.with(|clz32| clz32(lanes as u32)) as usize
}

fn lane_to_index(lane: Lane) -> usize {
    pick_arbitrary_lane_index(lane)
}

pub fn includes_some_lane(a: Lanes, b: Lanes) -> bool {
    (a & b) != NO_LANES
}

pub fn is_subset_of_lanes(set: Lanes, subset: Lanes) -> bool {
    (set & subset) == subset
}

pub fn merge_lanes(a: Lanes, b: Lanes) -> Lanes {
    a | b
}

pub fn remove_lanes(set: Lanes, subset: Lanes) -> Lanes {
    set & !subset
}

pub fn intersect_lanes(a: Lanes, b: Lanes) -> Lanes {
    a & b
}

// Seems redundant, but it changes the type from a single lane (used for
// updates) to a group of lanes (used for flushing work).
pub fn lane_to_lanes(lane: Lane) -> Lanes {
    lane
}

pub fn higher_priority_lane(a: Lane, b: Lane) -> Lane {
    // This works because the bit ranges decrease in priority as you go left.
    if a != NO_LANE && a < b { a } else { b }
}

pub fn create_lane_map<T: Copy>(initial: T) -> LaneMap<T> {
    // Intentionally pushing one by one.
    // https://v8.dev/blog/elements-kinds#avoid-creating-holes
    let mut lane_map = Vec::new();
    for _ in 0..TOTAL_LANES {
        lane_map.push(initial);
    }
    lane_map
}

pub fn mark_root_updated(root: &FiberRoot, update_lane: Lane) {
    root.pending_lanes
        .set(root.pending_lanes.get() | update_lane);
    if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        // Mark that this lane might need a loading indicator to be shown.
        root.indicator_lanes
            .set(root.indicator_lanes.get() | (update_lane & TRANSITION_LANES));
    }

    // If there are any suspended transitions, it's possible this new update
    // could unblock them. Clear the suspended lanes so that we can try rendering
    // them again.
    //
    // TODO: We really only need to unsuspend only lanes that are in the
    // `subtreeLanes` of the updated fiber, or the update lanes of the return
    // path. This would exclude suspended updates in an unrelated sibling tree,
    // since there's no way for this update to unblock it.
    //
    // We don't do this if the incoming update is idle, because we never process
    // idle updates until after all the regular updates have finished; there's no
    // way it could unblock a transition.
    if update_lane != IDLE_LANE {
        root.suspended_lanes.set(NO_LANES);
        root.pinged_lanes.set(NO_LANES);
        root.warm_lanes.set(NO_LANES);
    }
}

pub fn mark_root_suspended(
    root: &FiberRoot,
    suspended_lanes: Lanes,
    spawned_lane: Lane,
    did_attempt_entire_tree: bool,
) {
    // TODO: Split this into separate functions for marking the root at the end of
    // a render attempt versus suspending while the root is still in progress.
    root.suspended_lanes
        .set(root.suspended_lanes.get() | suspended_lanes);
    root.pinged_lanes
        .set(root.pinged_lanes.get() & !suspended_lanes);

    if did_attempt_entire_tree {
        // Mark these lanes as warm so we know there's nothing else to work on.
        root.warm_lanes.set(root.warm_lanes.get() | suspended_lanes);
    } else {
        // Render unwound without attempting all the siblings. Do no mark the lanes
        // as warm. This will cause a prewarm render to be scheduled.
    }

    // The suspended lanes are no longer CPU-bound. Clear their expiration times.
    let mut expiration_times = root.expiration_times.borrow_mut();
    let mut lanes = suspended_lanes;
    while lanes > 0 {
        let index = pick_arbitrary_lane_index(lanes);
        let lane = 1 << index;

        expiration_times[index] = NO_TIMESTAMP;

        lanes &= !lane;
    }
    drop(expiration_times);

    if spawned_lane != NO_LANE {
        mark_spawned_deferred_lane(root, spawned_lane, suspended_lanes);
    }
}

pub fn mark_root_pinged(root: &FiberRoot, pinged_lanes: Lanes) {
    root.pinged_lanes
        .set(root.pinged_lanes.get() | (root.suspended_lanes.get() & pinged_lanes));
    // The data that just resolved could have unblocked additional children, which
    // will also need to be prewarmed if something suspends again.
    root.warm_lanes.set(root.warm_lanes.get() & !pinged_lanes);
}

pub fn mark_root_finished(
    root: &FiberRoot,
    finished_lanes: Lanes,
    remaining_lanes: Lanes,
    spawned_lane: Lane,
    updated_lanes: Lanes,
    suspended_retry_lanes: Lanes,
) {
    let previously_pending_lanes = root.pending_lanes.get();
    let no_longer_pending_lanes = previously_pending_lanes & !remaining_lanes;

    root.pending_lanes.set(remaining_lanes);

    // Let's try everything again
    root.suspended_lanes.set(NO_LANES);
    root.pinged_lanes.set(NO_LANES);
    root.warm_lanes.set(NO_LANES);

    if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        root.indicator_lanes
            .set(root.indicator_lanes.get() & remaining_lanes);
    }

    root.expired_lanes
        .set(root.expired_lanes.get() & remaining_lanes);

    root.entangled_lanes
        .set(root.entangled_lanes.get() & remaining_lanes);

    root.error_recovery_disabled_lanes
        .set(root.error_recovery_disabled_lanes.get() & remaining_lanes);
    root.shell_suspend_counter.set(0);

    let mut entanglements = root.entanglements.borrow_mut();
    let mut expiration_times = root.expiration_times.borrow_mut();
    let mut hidden_updates = root.hidden_updates.borrow_mut();

    // Clear the lanes that no longer have pending work
    let mut lanes = no_longer_pending_lanes;
    while lanes > 0 {
        let index = pick_arbitrary_lane_index(lanes);
        let lane = 1 << index;

        entanglements[index] = NO_LANES;
        expiration_times[index] = NO_TIMESTAMP;

        if let Some(hidden_updates_for_lane) = hidden_updates[index].take() {
            // "Hidden" updates are updates that were made to a hidden component. They
            // have special logic associated with them because they may be entangled
            // with updates that occur outside that tree. But once the outer tree
            // commits, they behave like regular updates.
            for update in hidden_updates_for_lane {
                update.lane.set(update.lane.get() & !OFFSCREEN_LANE);
            }
        }

        lanes &= !lane;
    }
    drop(entanglements);
    drop(expiration_times);
    drop(hidden_updates);

    if spawned_lane != NO_LANE {
        mark_spawned_deferred_lane(
            root,
            spawned_lane,
            // This render finished successfully without suspending, so we don't need
            // to entangle the spawned task with the parent task.
            NO_LANES,
        );
    }

    // suspendedRetryLanes represents the retry lanes spawned by new Suspense
    // boundaries during this render that were not later pinged.
    //
    // These lanes were marked as pending on their associated Suspense boundary
    // fiber during the render phase so that we could start rendering them
    // before new data streams in. As soon as the fallback commits, we can try
    // to render them again.
    //
    // But since we know they're still suspended, we can skip straight to the
    // "prerender" mode (i.e. don't skip over siblings after something
    // suspended) instead of the regular mode (i.e. unwind and skip the siblings
    // as soon as something suspends to unblock the rest of the update).
    if suspended_retry_lanes != NO_LANES
        // Note that we only do this if there were no updates since we started
        // rendering. This mirrors the logic in markRootUpdated — whenever we
        // receive an update, we reset all the suspended and pinged lanes.
        && updated_lanes == NO_LANES
        && !(DISABLE_LEGACY_MODE && root.tag == LEGACY_ROOT)
    {
        // We also need to avoid marking a retry lane as suspended if it was already
        // pending before this render. We can't say these are now suspended if they
        // weren't included in our attempt.
        let freshly_spawned_retry_lanes = suspended_retry_lanes
            // Remove any retry lane that was already pending before our just-finished
            // attempt, and also wasn't included in that attempt.
            & !(previously_pending_lanes & !finished_lanes);
        root.suspended_lanes
            .set(root.suspended_lanes.get() | freshly_spawned_retry_lanes);
    }
}

fn mark_spawned_deferred_lane(root: &FiberRoot, spawned_lane: Lane, entangled_lanes: Lanes) {
    // This render spawned a deferred task. Mark it as pending.
    root.pending_lanes
        .set(root.pending_lanes.get() | spawned_lane);
    root.suspended_lanes
        .set(root.suspended_lanes.get() & !spawned_lane);

    // Entangle the spawned lane with the DeferredLane bit so that we know it
    // was the result of another render. This lets us avoid a useDeferredValue
    // waterfall — only the first level will defer.
    // TODO: Now that there is a reserved set of transition lanes that are used
    // exclusively for deferred work, we should get rid of this special
    // DeferredLane bit; the same information can be inferred by checking whether
    // the lane is one of the TransitionDeferredLanes. The only reason this still
    // exists is because we need to also do the same for OffscreenLane. That
    // requires additional changes because there are more places around the
    // codebase that treat OffscreenLane as a magic value; would need to check
    // for a new OffscreenDeferredLane, too. Will leave this for a follow-up.
    let spawned_lane_index = lane_to_index(spawned_lane);
    root.entangled_lanes
        .set(root.entangled_lanes.get() | spawned_lane);
    root.entanglements.borrow_mut()[spawned_lane_index] |= DEFERRED_LANE
        // If the parent render task suspended, we must also entangle those lanes
        // with the spawned task, so that the deferred task includes all the same
        // updates that the parent task did. We can exclude any lane that is not
        // used for updates (e.g. Offscreen).
        | (entangled_lanes & UPDATE_LANES);
}

pub fn mark_root_entangled(root: &FiberRoot, entangled_lanes: Lanes) {
    // In addition to entangling each of the given lanes with each other, we also
    // have to consider _transitive_ entanglements. For each lane that is already
    // entangled with *any* of the given lanes, that lane is now transitively
    // entangled with *all* the given lanes.
    //
    // Translated: If C is entangled with A, then entangling A with B also
    // entangles C with B.
    //
    // If this is hard to grasp, it might help to intentionally break this
    // function and look at the tests that fail in ReactTransition-test.js. Try
    // commenting out one of the conditions below.

    let root_entangled_lanes = root.entangled_lanes.get() | entangled_lanes;
    root.entangled_lanes.set(root_entangled_lanes);
    let mut entanglements = root.entanglements.borrow_mut();
    let mut lanes = root_entangled_lanes;
    while lanes != 0 {
        let index = pick_arbitrary_lane_index(lanes);
        let lane = 1 << index;
        if
        // Is this one of the newly entangled lanes?
        (lane & entangled_lanes)
            // Is this lane transitively entangled with the newly entangled lanes?
            | (entanglements[index] & entangled_lanes)
            != 0
        {
            entanglements[index] |= entangled_lanes;
        }
        lanes &= !lane;
    }
}

pub fn upgrade_pending_lanes_to_sync(root: &FiberRoot, lanes_to_upgrade: Lanes) {
    // Same as upgradePendingLaneToSync but accepts multiple lanes, so it's a
    // bit slower.
    root.pending_lanes.set(root.pending_lanes.get() | SYNC_LANE);
    root.entangled_lanes
        .set(root.entangled_lanes.get() | SYNC_LANE);
    let mut lanes = lanes_to_upgrade;
    while lanes != 0 {
        let index = pick_arbitrary_lane_index(lanes);
        let lane = 1 << index;
        root.entanglements.borrow_mut()[SYNC_LANE_INDEX] |= lane;
        lanes &= !lane;
    }
}

pub fn mark_hidden_update(root: &FiberRoot, update: &Rc<ConcurrentUpdate>, lane: Lane) {
    let index = lane_to_index(lane);
    let mut hidden_updates = root.hidden_updates.borrow_mut();
    match &mut hidden_updates[index] {
        None => hidden_updates[index] = Some(vec![Rc::clone(update)]),
        Some(hidden_updates_for_lane) => hidden_updates_for_lane.push(Rc::clone(update)),
    }
    update.lane.set(lane | OFFSCREEN_LANE);
}

pub fn get_bumped_lane_for_hydration(root: &FiberRoot, render_lanes: Lanes) -> Lane {
    let render_lane = get_highest_priority_lane(render_lanes);
    let bumped_lane = if (render_lane & SYNC_UPDATE_LANES) != NO_LANE {
        // Unify sync lanes. We don't do this inside getBumpedLaneForHydrationByLane
        // because that causes things to flush synchronously when they shouldn't.
        // TODO: This is not coherent but that's beacuse the unification is not coherent.
        // We need to get merge these into an actual single lane.
        SYNC_HYDRATION_LANE
    } else {
        get_bumped_lane_for_hydration_by_lane(render_lane)
    };
    // Check if the lane we chose is suspended. If so, that indicates that we
    // already attempted and failed to hydrate at that level. Also check if we're
    // already rendering that lane, which is rare but could happen.
    // TODO: This should move into the caller to decide whether giving up is valid.
    if (bumped_lane & (root.suspended_lanes.get() | render_lanes)) != NO_LANE {
        // Give up trying to hydrate and fall back to client render.
        return NO_LANE;
    }
    bumped_lane
}

pub fn get_bumped_lane_for_hydration_by_lane(lane: Lane) -> Lane {
    match lane {
        SYNC_LANE => SYNC_HYDRATION_LANE,
        INPUT_CONTINUOUS_LANE => INPUT_CONTINUOUS_HYDRATION_LANE,
        DEFAULT_LANE => DEFAULT_HYDRATION_LANE,
        TRANSITION_LANE_1 | TRANSITION_LANE_2 | TRANSITION_LANE_3 | TRANSITION_LANE_4
        | TRANSITION_LANE_5 | TRANSITION_LANE_6 | TRANSITION_LANE_7 | TRANSITION_LANE_8
        | TRANSITION_LANE_9 | TRANSITION_LANE_10 | TRANSITION_LANE_11 | TRANSITION_LANE_12
        | TRANSITION_LANE_13 | TRANSITION_LANE_14 | RETRY_LANE_1 | RETRY_LANE_2 | RETRY_LANE_3
        | RETRY_LANE_4 => TRANSITION_HYDRATION_LANE,
        IDLE_LANE => IDLE_HYDRATION_LANE,
        _ => {
            // Everything else is already either a hydration lane, or shouldn't
            // be retried at a hydration lane.
            NO_LANE
        }
    }
}

pub fn add_fiber_to_lanes_map(root: &FiberRoot, fiber: &Rc<Fiber>, mut lanes: Lanes) {
    if !ENABLE_UPDATER_TRACKING.with(|&enabled| enabled) {
        return;
    }
    if !IS_DEV_TOOLS_PRESENT {
        return;
    }
    let mut pending_updaters_lane_map = root.pending_updaters_lane_map.borrow_mut();
    while lanes > 0 {
        let index = lane_to_index(lanes);
        let lane = 1 << index;

        let updaters = &mut pending_updaters_lane_map[index];
        updaters.insert(Rc::clone(fiber));

        lanes &= !lane;
    }
}

pub fn move_pending_fibers_to_memoized(root: &FiberRoot, mut lanes: Lanes) {
    if !ENABLE_UPDATER_TRACKING.with(|&enabled| enabled) {
        return;
    }
    if !IS_DEV_TOOLS_PRESENT {
        return;
    }
    let mut pending_updaters_lane_map = root.pending_updaters_lane_map.borrow_mut();
    let mut memoized_updaters = root.memoized_updaters.borrow_mut();
    while lanes > 0 {
        let index = lane_to_index(lanes);
        let lane = 1 << index;

        let updaters = &mut pending_updaters_lane_map[index];
        if !updaters.is_empty() {
            updaters.iter().for_each(|fiber| {
                let alternate = fiber.alternate.borrow().clone();
                if alternate.is_none_or(|alternate| !memoized_updaters.contains(&alternate)) {
                    memoized_updaters.insert(Rc::clone(fiber));
                }
            });
            updaters.clear();
        }

        lanes &= !lane;
    }
}

pub fn add_transition_to_lanes_map(root: &FiberRoot, transition: &Unknown, lane: Lane) {
    if ENABLE_TRANSITION_TRACING {
        let mut transition_lanes_map = root.transition_lanes.borrow_mut();
        let index = lane_to_index(lane);
        let transitions = transition_lanes_map[index].unwrap_or_else(|| js::set::new(None));
        transitions.add(transition);

        transition_lanes_map[index] = Some(transitions);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_transitions_for_lanes(
    root: &FiberRoot,
    mut lanes: Lanes,
) -> Option<Vec<&'static Unknown>> {
    if !ENABLE_TRANSITION_TRACING {
        return None;
    }

    let mut transitions_for_lanes = Vec::new();
    while lanes > 0 {
        let index = lane_to_index(lanes);
        let lane = 1 << index;
        let transitions = root.transition_lanes.borrow()[index];
        if let Some(transitions) = transitions {
            transitions.for_each(|transition| {
                transitions_for_lanes.push(transition);
            });
        }

        lanes &= !lane;
    }

    if transitions_for_lanes.is_empty() {
        return None;
    }

    Some(transitions_for_lanes)
}

pub fn clear_transitions_for_lanes(root: &FiberRoot, mut lanes: Lanes) {
    if !ENABLE_TRANSITION_TRACING {
        return;
    }

    while lanes > 0 {
        let index = lane_to_index(lanes);
        let lane = 1 << index;

        let mut transition_lanes = root.transition_lanes.borrow_mut();
        let transitions = transition_lanes[index];
        if transitions.is_some() {
            transition_lanes[index] = None;
        }

        lanes &= !lane;
    }
}

// Used to name the Performance Track
pub fn get_group_name_of_highest_priority_lane(lanes: Lanes) -> &'static str {
    if lanes
        & (SYNC_HYDRATION_LANE
            | SYNC_LANE
            | INPUT_CONTINUOUS_HYDRATION_LANE
            | INPUT_CONTINUOUS_LANE
            | DEFAULT_HYDRATION_LANE
            | DEFAULT_LANE)
        != 0
    {
        return "Blocking";
    }
    if lanes & GESTURE_LANE != 0 {
        return "Gesture";
    }
    if lanes & (TRANSITION_HYDRATION_LANE | TRANSITION_LANES) != 0 {
        return "Transition";
    }
    if lanes & RETRY_LANES != 0 {
        return "Suspense";
    }
    if lanes
        & (SELECTIVE_HYDRATION_LANE
            | IDLE_HYDRATION_LANE
            | IDLE_LANE
            | OFFSCREEN_LANE
            | DEFERRED_LANE)
        != 0
    {
        return "Idle";
    }
    "Other"
}
