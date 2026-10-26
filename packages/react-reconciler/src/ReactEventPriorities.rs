// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::react_fiber_lane::{
    DEFAULT_LANE, IDLE_LANE, INPUT_CONTINUOUS_LANE, Lane, Lanes, NO_LANE, SYNC_LANE,
    get_highest_priority_lane, includes_non_idle_work,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct EventPriority(Lane);

#[cfg_attr(rust_js, rust_js::name = "NoEventPriority")]
pub const NO_EVENT_PRIORITY: EventPriority = EventPriority(NO_LANE);
#[cfg_attr(rust_js, rust_js::name = "DiscreteEventPriority")]
pub const DISCRETE_EVENT_PRIORITY: EventPriority = EventPriority(SYNC_LANE);
#[cfg_attr(rust_js, rust_js::name = "ContinuousEventPriority")]
pub const CONTINUOUS_EVENT_PRIORITY: EventPriority = EventPriority(INPUT_CONTINUOUS_LANE);
#[cfg_attr(rust_js, rust_js::name = "DefaultEventPriority")]
pub const DEFAULT_EVENT_PRIORITY: EventPriority = EventPriority(DEFAULT_LANE);
#[cfg_attr(rust_js, rust_js::name = "IdleEventPriority")]
pub const IDLE_EVENT_PRIORITY: EventPriority = EventPriority(IDLE_LANE);

pub fn higher_event_priority(a: EventPriority, b: EventPriority) -> EventPriority {
    if a.0 != 0 && a < b { a } else { b }
}

pub fn lower_event_priority(a: EventPriority, b: EventPriority) -> EventPriority {
    if a.0 == 0 || a > b { a } else { b }
}

pub fn is_higher_event_priority(a: EventPriority, b: EventPriority) -> bool {
    a.0 != 0 && a < b
}

pub fn event_priority_to_lane(update_priority: EventPriority) -> Lane {
    update_priority.0
}

pub fn lanes_to_event_priority(lanes: Lanes) -> EventPriority {
    let lane = EventPriority(get_highest_priority_lane(lanes));
    if !is_higher_event_priority(DISCRETE_EVENT_PRIORITY, lane) {
        return DISCRETE_EVENT_PRIORITY;
    }
    if !is_higher_event_priority(CONTINUOUS_EVENT_PRIORITY, lane) {
        return CONTINUOUS_EVENT_PRIORITY;
    }
    if includes_non_idle_work(lane.0) {
        return DEFAULT_EVENT_PRIORITY;
    }
    IDLE_EVENT_PRIORITY
}
