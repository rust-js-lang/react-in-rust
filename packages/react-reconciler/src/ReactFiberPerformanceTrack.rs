// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// eslint-disable react-internal/no-production-logging

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Kind, Unknown, function};
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    ENABLE_GESTURE_TRANSITION, ENABLE_PERFORMANCE_ISSUE_REPORTING, ENABLE_PROFILER_TIMER,
};
use shared::react_performance_track_properties::{
    add_object_diff_to_properties, add_object_to_properties, add_value_to_properties,
};
use webapi::{
    ConsoleTask, IntoStrOrNumber, IntoStrOrPerformanceMeasureOptions, Performance,
    PerformanceMeasureOptions, console, performance,
};

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_captured_value::CapturedValue;
use crate::react_fiber_lane::{
    Lanes, get_group_name_of_highest_priority_lane, includes_only_hydration_lanes,
    includes_only_hydration_or_offscreen_lanes, includes_only_offscreen_lanes, includes_some_lane,
};
use crate::react_internal_types::Fiber;
use crate::react_work_tags::SUSPENSE_COMPONENT;

/// What a phase captured: shared arrays, as upstream's live ones.
type CapturedValues = RefCell<Vec<Rc<CapturedValue<Option<&'static Unknown>>>>>;

/// What DevTools' performance panel reads of a measure's `detail.devtools`:
/// the entry's track, its color, and what its tooltip shows.
pub struct DevToolsDetails<'a> {
    pub color: &'a str,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub properties: Option<&'a [(String, String)]>,
    pub tooltip_text: Option<&'a str>,
    pub track: &'a str,
    pub track_group: Option<&'a str>,
    pub performance_issue: Option<&'a PerformanceIssue>,
}

/// A measure's `detail`, which DevTools reads.
pub struct MeasureDetail<'a> {
    pub devtools: DevToolsDetails<'a>,
}

/// A performance issue DevTools lists for an entry.
pub struct PerformanceIssue {
    pub name: &'static str,
    pub severity: &'static str,
    pub description: &'static str,
    pub learn_more_url: &'static str,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "supportsUserTiming")]
    static SUPPORTS_USER_TIMING: bool = ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && js::is_defined(console)
        && js::is_function(console::time_stamp)
        && (!__DEV__
            // In DEV we also rely on performance.measure
            || (js::is_defined(performance) && js::is_function(Performance::measure)));
}

const COMPONENTS_TRACK: &str = "Components ⚛";
const LANES_TRACK_GROUP: &str = "Scheduler ⚛";

thread_local! {
    static CURRENT_TRACK: Cell<&'static str> = const { Cell::new("Blocking") }; // Lane
}

pub fn set_current_track_from_lanes(lanes: Lanes) {
    CURRENT_TRACK.set(get_group_name_of_highest_priority_lane(lanes));
}

pub fn mark_all_lanes_in_order() {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        // Ensure we create all tracks in priority order. Currently performance.mark() are in
        // first insertion order but performance.measure() are in the reverse order. We can
        // always add the 0 time slot even if it's in the past. That's still considered for
        // ordering.
        console::time_stamp(
            "Blocking Track",
            Some(&0.003),
            Some(&0.003),
            Some("Blocking"),
            Some(LANES_TRACK_GROUP),
            Some("primary-light"),
        );
        if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
            console::time_stamp(
                "Gesture Track",
                Some(&0.003),
                Some(&0.003),
                Some("Gesture"),
                Some(LANES_TRACK_GROUP),
                Some("primary-light"),
            );
        }
        console::time_stamp(
            "Transition Track",
            Some(&0.003),
            Some(&0.003),
            Some("Transition"),
            Some(LANES_TRACK_GROUP),
            Some("primary-light"),
        );
        console::time_stamp(
            "Suspense Track",
            Some(&0.003),
            Some(&0.003),
            Some("Suspense"),
            Some(LANES_TRACK_GROUP),
            Some("primary-light"),
        );
        console::time_stamp(
            "Idle Track",
            Some(&0.003),
            Some(&0.003),
            Some("Idle"),
            Some(LANES_TRACK_GROUP),
            Some("primary-light"),
        );
    }
}

fn log_component_trigger(fiber: &Fiber, start_time: f64, end_time: f64, trigger: &str) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let options = PerformanceMeasureOptions {
            start: Some(start_time.into()),
            end: Some(end_time.into()),
            detail: Some(&MeasureDetail {
                devtools: DevToolsDetails {
                    color: "warning",
                    properties: None,
                    tooltip_text: Some(trigger),
                    track: COMPONENTS_TRACK,
                    track_group: None,
                    performance_issue: None,
                },
            }),
            ..Default::default()
        };
        let debug_task = fiber.debug_task.get();
        if __DEV__ && let Some(debug_task) = debug_task {
            debug_task.run(function::bind(
                Performance::measure,
                Some(&performance),
                (
                    performance,
                    trigger,
                    Some(&options as &dyn IntoStrOrPerformanceMeasureOptions),
                    None,
                ),
            ));
        } else {
            performance.measure(trigger, Some(&options), None);
        }
        performance.clear_measures(Some(trigger));
    }
}

pub fn log_component_mount(fiber: &Fiber, start_time: f64, end_time: f64) {
    log_component_trigger(fiber, start_time, end_time, "Mount");
}

pub fn log_component_unmount(fiber: &Fiber, start_time: f64, end_time: f64) {
    log_component_trigger(fiber, start_time, end_time, "Unmount");
}

pub fn log_component_reappeared(fiber: &Fiber, start_time: f64, end_time: f64) {
    log_component_trigger(fiber, start_time, end_time, "Reconnect");
}

pub fn log_component_disappeared(fiber: &Fiber, start_time: f64, end_time: f64) {
    log_component_trigger(fiber, start_time, end_time, "Disconnect");
}

thread_local! {
    static ALREADY_WARNED_FOR_DEEP_EQUALITY: Cell<bool> = const { Cell::new(false) };
}

pub fn push_deep_equality() -> bool {
    if __DEV__ {
        // If this is true then we don't reset it to false because we're tracking if any
        // parent already warned about having deep equality props in this subtree.
        return ALREADY_WARNED_FOR_DEEP_EQUALITY.get();
    }
    false
}

pub fn pop_deep_equality(prev: bool) {
    if __DEV__ {
        ALREADY_WARNED_FOR_DEEP_EQUALITY.set(prev);
    }
}

#[cfg_attr(rust_js, rust_js::name = "reusableCascadingUpdateIssue")]
const REUSABLE_CASCADING_UPDATE_ISSUE: PerformanceIssue = PerformanceIssue {
    name: "React: Cascading Update",
    severity: "warning",
    description: "A cascading update is an update that is triggered during an ongoing render. This can lead to performance issues.",
    learn_more_url: "https://react.dev/reference/dev-tools/react-performance-tracks#cascading-updates",
};

const DEEP_EQUALITY_WARNING: &str = "This component received deeply equal props. It might benefit from useMemo or the React Compiler in its owner.";

pub fn log_component_render(
    fiber: &Fiber,
    start_time: f64,
    end_time: f64,
    was_hydrated: bool,
    committed_lanes: Lanes,
) {
    let Some(name) = get_component_name_from_fiber(fiber) else {
        // Skip
        return;
    };
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let alternate = fiber.alternate.borrow().clone();
        let mut self_time = fiber.actual_duration.get().unwrap();
        if alternate
            .as_ref()
            .is_none_or(|alternate| *alternate.child.borrow() != *fiber.child.borrow())
        {
            let mut child = fiber.child.borrow().clone();
            while let Some(current) = child {
                self_time -= current.actual_duration.get().unwrap();
                child = current.sibling.borrow().clone();
            }
        }
        let color = if self_time < 0.5 {
            if was_hydrated {
                "tertiary-light"
            } else {
                "primary-light"
            }
        } else if self_time < 10.0 {
            if was_hydrated { "tertiary" } else { "primary" }
        } else if self_time < 100.0 {
            if was_hydrated {
                "tertiary-dark"
            } else {
                "primary-dark"
            }
        } else {
            "error"
        };

        if !__DEV__ {
            console::time_stamp(
                &name,
                Some(&start_time),
                Some(&end_time),
                Some(COMPONENTS_TRACK),
                None,
                Some(color),
            );
        } else {
            let props = fiber.memoized_props.get();
            let debug_task = fiber.debug_task.get();

            if let Some(props) = props
                && let Some(alternate) = &alternate
                && let Some(alternate_props) = alternate.memoized_props.get()
                && !std::ptr::eq(alternate_props, props)
            {
                // If this is an update, we'll diff the props and emit which ones changed.
                let mut properties = vec![("Changed Props".to_string(), String::new())];
                let is_deeply_equal =
                    add_object_diff_to_properties(alternate_props, props, &mut properties, 0);
                if properties.len() > 1 {
                    let mut detail = MeasureDetail {
                        devtools: DevToolsDetails {
                            color,
                            properties: None,
                            tooltip_text: Some(&name),
                            track: COMPONENTS_TRACK,
                            track_group: None,
                            performance_issue: None,
                        },
                    };
                    if is_deeply_equal
                        && !ALREADY_WARNED_FOR_DEEP_EQUALITY.get()
                        && !includes_some_lane(alternate.lanes.get(), committed_lanes)
                        && fiber.actual_duration.get().unwrap() > 100.0
                    {
                        ALREADY_WARNED_FOR_DEEP_EQUALITY.set(true);
                        // This is the first component in a subtree which rerendered with deeply equal props
                        // and didn't have its own work scheduled and took a non-trivial amount of time.
                        // We highlight this for further inspection.
                        // Note that we only consider this case if properties.length > 1 which it will only
                        // be if we have emitted any diffs. We'd only emit diffs if there were any nested
                        // equal objects. Therefore, we don't warn for simple shallow equality.
                        properties[0] = (
                            "Changed Props".to_string(),
                            DEEP_EQUALITY_WARNING.to_string(),
                        );
                        detail.devtools.color = "warning";
                        detail.devtools.tooltip_text = Some(DEEP_EQUALITY_WARNING);
                    }
                    detail.devtools.properties = Some(&properties);
                    let options = PerformanceMeasureOptions {
                        start: Some(start_time.into()),
                        end: Some(end_time.into()),
                        detail: Some(&detail),
                        ..Default::default()
                    };

                    let measure_name = "\u{200b}".to_string() + &name;
                    if let Some(debug_task) = debug_task {
                        debug_task.run(function::bind(
                            Performance::measure,
                            Some(&performance),
                            (
                                performance,
                                measure_name.as_str(),
                                Some(&options as &dyn IntoStrOrPerformanceMeasureOptions),
                                None,
                            ),
                        ));
                    } else {
                        performance.measure(&measure_name, Some(&options), None);
                    }
                    performance.clear_measures(Some(&measure_name));
                } else if let Some(debug_task) = debug_task {
                    let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                        (&start_time, &end_time);
                    debug_task.run(function::bind(
                        console::time_stamp,
                        Some(&console),
                        (
                            name.as_str(),
                            Some(start_time),
                            Some(end_time),
                            Some(COMPONENTS_TRACK),
                            None,
                            Some(color),
                        ),
                    ));
                } else {
                    console::time_stamp(
                        &name,
                        Some(&start_time),
                        Some(&end_time),
                        Some(COMPONENTS_TRACK),
                        None,
                        Some(color),
                    );
                }
            } else if let Some(debug_task) = debug_task {
                let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                    (&start_time, &end_time);
                debug_task.run(function::bind(
                    console::time_stamp,
                    Some(&console),
                    (
                        name.as_str(),
                        Some(start_time),
                        Some(end_time),
                        Some(COMPONENTS_TRACK),
                        None,
                        Some(color),
                    ),
                ));
            } else {
                console::time_stamp(
                    &name,
                    Some(&start_time),
                    Some(&end_time),
                    Some(COMPONENTS_TRACK),
                    None,
                    Some(color),
                );
            }
        }
    }
}

pub fn log_component_errored(
    fiber: &Fiber,
    start_time: f64,
    end_time: f64,
    errors: &CapturedValues,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let Some(name) = get_component_name_from_fiber(fiber) else {
            // Skip
            return;
        };
        if __DEV__ {
            let mut debug_task: Option<&ConsoleTask> = js::null();
            let mut properties: Vec<(String, String)> = Vec::new();
            let mut i = 0;
            while i < errors.borrow().len() {
                let captured_value = Rc::clone(&errors.borrow()[i]);
                if debug_task.is_none()
                    && let Some(source) = &captured_value.source
                {
                    // If the captured value has a source Fiber, use its debugTask for
                    // the stack instead of the error boundary's stack. So you can find
                    // which component errored since we don't show the errored render tree.
                    // TODO: Ideally we should instead, store the failed fibers and log the
                    // whole subtree including the component that errored.
                    debug_task = source.debug_task.get();
                }
                let error = captured_value.value;
                let message = if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
                    && js::type_of(js::get(error, "message")) == "string"
                {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(js::get(error, "message"))
                } else {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(error)
                };
                properties.push(("Error".to_string(), message));
                i += 1;
            }
            let key = fiber.key.borrow().clone();
            if let Some(key) = key {
                add_value_to_properties("key", Some(js::unknown_of(key)), &mut properties, 0, "");
            }
            if let Some(memoized_props) = fiber.memoized_props.get() {
                add_object_to_properties(memoized_props, &mut properties, 0, "");
            }
            if debug_task.is_none() {
                // If the captured values don't have a debug task, fallback to the
                // error boundary itself.
                debug_task = fiber.debug_task.get();
            }
            let options = PerformanceMeasureOptions {
                start: Some(start_time.into()),
                end: Some(end_time.into()),
                detail: Some(&MeasureDetail {
                    devtools: DevToolsDetails {
                        color: "error",
                        track: COMPONENTS_TRACK,
                        tooltip_text: Some(if fiber.tag.get() == SUSPENSE_COMPONENT {
                            "Hydration failed"
                        } else {
                            "Error boundary caught an error"
                        }),
                        properties: Some(&properties),
                        track_group: None,
                        performance_issue: None,
                    },
                }),
                ..Default::default()
            };

            let measure_name = "\u{200b}".to_string() + &name;
            if __DEV__ && let Some(debug_task) = debug_task {
                debug_task.run(function::bind(
                    Performance::measure,
                    Some(&performance),
                    (
                        performance,
                        measure_name.as_str(),
                        Some(&options as &dyn IntoStrOrPerformanceMeasureOptions),
                        None,
                    ),
                ));
            } else {
                performance.measure(&measure_name, Some(&options), None);
            }
            performance.clear_measures(Some(&measure_name));
        } else {
            console::time_stamp(
                &name,
                Some(&start_time),
                Some(&end_time),
                Some(COMPONENTS_TRACK),
                None,
                Some("error"),
            );
        }
    }
}

fn log_component_effect_errored(
    fiber: &Fiber,
    start_time: f64,
    end_time: f64,
    errors: &CapturedValues,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let Some(name) = get_component_name_from_fiber(fiber) else {
            // Skip
            return;
        };
        if __DEV__ {
            let mut properties: Vec<(String, String)> = Vec::new();
            let mut i = 0;
            while i < errors.borrow().len() {
                let captured_value = Rc::clone(&errors.borrow()[i]);
                let error = captured_value.value;
                let message = if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
                    && js::type_of(js::get(error, "message")) == "string"
                {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(js::get(error, "message"))
                } else {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(error)
                };
                properties.push(("Error".to_string(), message));
                i += 1;
            }
            let key = fiber.key.borrow().clone();
            if let Some(key) = key {
                add_value_to_properties("key", Some(js::unknown_of(key)), &mut properties, 0, "");
            }
            if let Some(memoized_props) = fiber.memoized_props.get() {
                add_object_to_properties(memoized_props, &mut properties, 0, "");
            }
            let options = PerformanceMeasureOptions {
                start: Some(start_time.into()),
                end: Some(end_time.into()),
                detail: Some(&MeasureDetail {
                    devtools: DevToolsDetails {
                        color: "error",
                        track: COMPONENTS_TRACK,
                        tooltip_text: Some("A lifecycle or effect errored"),
                        properties: Some(&properties),
                        track_group: None,
                        performance_issue: None,
                    },
                }),
                ..Default::default()
            };
            let debug_task = fiber.debug_task.get();
            let measure_name = "\u{200b}".to_string() + &name;
            if let Some(debug_task) = debug_task {
                debug_task.run(function::bind(
                    Performance::measure,
                    Some(&performance),
                    (
                        performance,
                        measure_name.as_str(),
                        Some(&options as &dyn IntoStrOrPerformanceMeasureOptions),
                        None,
                    ),
                ));
            } else {
                performance.measure(&measure_name, Some(&options), None);
            }
            performance.clear_measures(Some(&measure_name));
        } else {
            console::time_stamp(
                &name,
                Some(&start_time),
                Some(&end_time),
                Some(COMPONENTS_TRACK),
                None,
                Some("error"),
            );
        }
    }
}

pub fn log_component_effect(
    fiber: &Fiber,
    start_time: f64,
    end_time: f64,
    self_time: f64,
    errors: Option<&CapturedValues>,
) {
    if let Some(errors) = errors {
        log_component_effect_errored(fiber, start_time, end_time, errors);
        return;
    }
    let Some(name) = get_component_name_from_fiber(fiber) else {
        // Skip
        return;
    };
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let color = if self_time < 1.0 {
            "secondary-light"
        } else if self_time < 100.0 {
            "secondary"
        } else if self_time < 500.0 {
            "secondary-dark"
        } else {
            "error"
        };
        let debug_task = fiber.debug_task.get();
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    name.as_str(),
                    Some(start_time),
                    Some(end_time),
                    Some(COMPONENTS_TRACK),
                    None,
                    Some(color),
                ),
            ));
        } else {
            console::time_stamp(
                &name,
                Some(&start_time),
                Some(&end_time),
                Some(COMPONENTS_TRACK),
                None,
                Some(color),
            );
        }
    }
}

pub fn log_yield_time(start_time: f64, end_time: f64) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let yield_duration = end_time - start_time;
        if yield_duration < 3.0 {
            // Skip sub-millisecond yields. This happens all the time and is not interesting.
            return;
        }
        // Being blocked on CPU is potentially bad so we color it by how long it took.
        let color = if yield_duration < 5.0 {
            "primary-light"
        } else if yield_duration < 10.0 {
            "primary"
        } else if yield_duration < 100.0 {
            "primary-dark"
        } else {
            "error"
        };
        // This get logged in the components track if we don't commit which leaves them
        // hanging by themselves without context. It's a useful indicator for why something
        // might be starving this render though.
        // TODO: Considering adding these to a queue and only logging them if we commit.
        console::time_stamp(
            "Blocked",
            Some(&start_time),
            Some(&end_time),
            Some(COMPONENTS_TRACK),
            None,
            Some(color),
        );
    }
}

pub fn log_suspended_yield_time(start_time: f64, end_time: f64, suspended_fiber: &Fiber) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let debug_task = suspended_fiber.debug_task.get();
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Suspended",
                    Some(start_time),
                    Some(end_time),
                    Some(COMPONENTS_TRACK),
                    None,
                    Some("primary-light"),
                ),
            ));
        } else {
            console::time_stamp(
                "Suspended",
                Some(&start_time),
                Some(&end_time),
                Some(COMPONENTS_TRACK),
                None,
                Some("primary-light"),
            );
        }
    }
}

pub fn log_action_yield_time(start_time: f64, end_time: f64, suspended_fiber: &Fiber) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let debug_task = suspended_fiber.debug_task.get();
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Action",
                    Some(start_time),
                    Some(end_time),
                    Some(COMPONENTS_TRACK),
                    None,
                    Some("primary-light"),
                ),
            ));
        } else {
            console::time_stamp(
                "Action",
                Some(&start_time),
                Some(&end_time),
                Some(COMPONENTS_TRACK),
                None,
                Some("primary-light"),
            );
        }
    }
}

pub fn log_blocking_start(
    mut update_time: f64,
    mut event_time: f64,
    event_type: Option<&str>,
    event_is_repeat: bool,
    is_spawned_update: bool,
    is_pinged_update: bool,
    render_start_time: f64,
    lanes: Lanes,
    debug_task: Option<&ConsoleTask>, // DEV-only
    update_method_name: Option<&str>,
    update_component_name: Option<&str>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        CURRENT_TRACK.set("Blocking");
        // Clamp start times
        if update_time > 0.0 {
            if update_time > render_start_time {
                update_time = render_start_time;
            }
        } else {
            update_time = render_start_time;
        }
        if event_time > 0.0 {
            if event_time > update_time {
                event_time = update_time;
            }
        } else {
            event_time = update_time;
        }
        // If a blocking update was spawned within render or an effect, that's considered a cascading render.
        // If you have a second blocking update within the same event, that suggests multiple flushSync or
        // setState in a microtask which is also considered a cascade.
        if let Some(event_type) = event_type
            && update_time > event_time
        {
            // Log the time from the event timeStamp until we called setState.
            let color = if event_is_repeat {
                "secondary-light"
            } else {
                "warning"
            };
            if __DEV__ && let Some(debug_task) = debug_task {
                let (event_time, update_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                    (&event_time, &update_time);
                debug_task.run(function::bind(
                    console::time_stamp,
                    Some(&console),
                    (
                        if event_is_repeat {
                            "Consecutive".to_string()
                        } else {
                            "Event: ".to_string() + event_type
                        }
                        .as_str(),
                        Some(event_time),
                        Some(update_time),
                        Some(CURRENT_TRACK.get()),
                        Some(LANES_TRACK_GROUP),
                        Some(color),
                    ),
                ));
            } else {
                console::time_stamp(
                    &if event_is_repeat {
                        "Consecutive".to_string()
                    } else {
                        "Event: ".to_string() + event_type
                    },
                    Some(&event_time),
                    Some(&update_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                );
            }
        }
        if render_start_time > update_time {
            // Log the time from when we called setState until we started rendering.
            let color = if is_spawned_update {
                "error"
            } else if includes_only_hydration_or_offscreen_lanes(lanes) {
                "tertiary-light"
            } else {
                "primary-light"
            };
            let label = if is_pinged_update {
                "Promise Resolved"
            } else if is_spawned_update {
                "Cascading Update"
            } else if render_start_time - update_time > 5.0 {
                "Update Blocked"
            } else {
                "Update"
            };

            if __DEV__ {
                let mut properties = Vec::new();
                if let Some(update_component_name) = update_component_name {
                    properties.push((
                        "Component name".to_string(),
                        update_component_name.to_string(),
                    ));
                }
                if let Some(update_method_name) = update_method_name {
                    properties.push(("Method name".to_string(), update_method_name.to_string()));
                }
                let mut detail = MeasureDetail {
                    devtools: DevToolsDetails {
                        properties: Some(&properties),
                        track: CURRENT_TRACK.get(),
                        track_group: Some(LANES_TRACK_GROUP),
                        color,
                        tooltip_text: None,
                        performance_issue: None,
                    },
                };
                if ENABLE_PERFORMANCE_ISSUE_REPORTING && is_spawned_update {
                    detail.devtools.performance_issue = Some(&REUSABLE_CASCADING_UPDATE_ISSUE);
                }
                let measure_options = PerformanceMeasureOptions {
                    start: Some(update_time.into()),
                    end: Some(render_start_time.into()),
                    detail: Some(&detail),
                    ..Default::default()
                };

                if let Some(debug_task) = debug_task {
                    debug_task.run(function::bind(
                        Performance::measure,
                        Some(&performance),
                        (
                            performance,
                            label,
                            Some(&measure_options as &dyn IntoStrOrPerformanceMeasureOptions),
                            None,
                        ),
                    ));
                } else {
                    performance.measure(label, Some(&measure_options), None);
                }
                performance.clear_measures(Some(label));
            } else {
                console::time_stamp(
                    label,
                    Some(&update_time),
                    Some(&render_start_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                );
            }
        }
    }
}

pub fn log_gesture_start(
    mut update_time: f64,
    mut event_time: f64,
    event_type: Option<&str>,
    event_is_repeat: bool,
    is_pinged_update: bool,
    render_start_time: f64,
    debug_task: Option<&ConsoleTask>, // DEV-only
    update_method_name: Option<&str>,
    update_component_name: Option<&str>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        CURRENT_TRACK.set("Gesture");
        // Clamp start times
        if update_time > 0.0 {
            if update_time > render_start_time {
                update_time = render_start_time;
            }
        } else {
            update_time = render_start_time;
        }
        if event_time > 0.0 {
            if event_time > update_time {
                event_time = update_time;
            }
        } else {
            event_time = update_time;
        }

        if update_time > event_time
            && let Some(event_type) = event_type
        {
            // Log the time from the event timeStamp until we started a gesture.
            let color = if event_is_repeat {
                "secondary-light"
            } else {
                "warning"
            };
            if __DEV__ && let Some(debug_task) = debug_task {
                let (event_time, update_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                    (&event_time, &update_time);
                debug_task.run(function::bind(
                    console::time_stamp,
                    Some(&console),
                    (
                        if event_is_repeat {
                            "Consecutive".to_string()
                        } else {
                            "Event: ".to_string() + event_type
                        }
                        .as_str(),
                        Some(event_time),
                        Some(update_time),
                        Some(CURRENT_TRACK.get()),
                        Some(LANES_TRACK_GROUP),
                        Some(color),
                    ),
                ));
            } else {
                console::time_stamp(
                    &if event_is_repeat {
                        "Consecutive".to_string()
                    } else {
                        "Event: ".to_string() + event_type
                    },
                    Some(&event_time),
                    Some(&update_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                );
            }
        }
        if render_start_time > update_time {
            // Log the time from when we called setState until we started rendering.
            let label = if is_pinged_update {
                "Promise Resolved"
            } else if render_start_time - update_time > 5.0 {
                "Gesture Blocked"
            } else {
                "Gesture"
            };
            if __DEV__ {
                let mut properties = Vec::new();
                if let Some(update_component_name) = update_component_name {
                    properties.push((
                        "Component name".to_string(),
                        update_component_name.to_string(),
                    ));
                }
                if let Some(update_method_name) = update_method_name {
                    properties.push(("Method name".to_string(), update_method_name.to_string()));
                }
                let measure_options = PerformanceMeasureOptions {
                    start: Some(update_time.into()),
                    end: Some(render_start_time.into()),
                    detail: Some(&MeasureDetail {
                        devtools: DevToolsDetails {
                            properties: Some(&properties),
                            track: CURRENT_TRACK.get(),
                            track_group: Some(LANES_TRACK_GROUP),
                            color: "primary-light",
                            tooltip_text: None,
                            performance_issue: None,
                        },
                    }),
                    ..Default::default()
                };

                if let Some(debug_task) = debug_task {
                    debug_task.run(function::bind(
                        Performance::measure,
                        Some(&performance),
                        (
                            performance,
                            label,
                            Some(&measure_options as &dyn IntoStrOrPerformanceMeasureOptions),
                            None,
                        ),
                    ));
                } else {
                    performance.measure(label, Some(&measure_options), None);
                }
                performance.clear_measures(Some(label));
            } else {
                console::time_stamp(
                    label,
                    Some(&update_time),
                    Some(&render_start_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("primary-light"),
                );
            }
        }
    }
}

pub fn log_transition_start(
    mut start_time: f64,
    mut update_time: f64,
    mut event_time: f64,
    event_type: Option<&str>,
    event_is_repeat: bool,
    is_pinged_update: bool,
    render_start_time: f64,
    debug_task: Option<&ConsoleTask>, // DEV-only
    update_method_name: Option<&str>,
    update_component_name: Option<&str>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        CURRENT_TRACK.set("Transition");
        // Clamp start times
        if update_time > 0.0 {
            if update_time > render_start_time {
                update_time = render_start_time;
            }
        } else {
            update_time = render_start_time;
        }
        if start_time > 0.0 {
            if start_time > update_time {
                start_time = update_time;
            }
        } else {
            start_time = update_time;
        }
        if event_time > 0.0 {
            if event_time > start_time {
                event_time = start_time;
            }
        } else {
            event_time = start_time;
        }

        if start_time > event_time
            && let Some(event_type) = event_type
        {
            // Log the time from the event timeStamp until we started a transition.
            let color = if event_is_repeat {
                "secondary-light"
            } else {
                "warning"
            };
            if __DEV__ && let Some(debug_task) = debug_task {
                let (event_time, start_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                    (&event_time, &start_time);
                debug_task.run(function::bind(
                    console::time_stamp,
                    Some(&console),
                    (
                        if event_is_repeat {
                            "Consecutive".to_string()
                        } else {
                            "Event: ".to_string() + event_type
                        }
                        .as_str(),
                        Some(event_time),
                        Some(start_time),
                        Some(CURRENT_TRACK.get()),
                        Some(LANES_TRACK_GROUP),
                        Some(color),
                    ),
                ));
            } else {
                console::time_stamp(
                    &if event_is_repeat {
                        "Consecutive".to_string()
                    } else {
                        "Event: ".to_string() + event_type
                    },
                    Some(&event_time),
                    Some(&start_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                );
            }
        }
        if update_time > start_time {
            // Log the time from when we started an async transition until we called setState or started rendering.
            // TODO: Ideally this would use the debugTask of the startTransition call perhaps.
            if __DEV__ && let Some(debug_task) = debug_task {
                let (start_time, update_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                    (&start_time, &update_time);
                debug_task.run(function::bind(
                    console::time_stamp,
                    Some(&console),
                    (
                        "Action",
                        Some(start_time),
                        Some(update_time),
                        Some(CURRENT_TRACK.get()),
                        Some(LANES_TRACK_GROUP),
                        Some("primary-dark"),
                    ),
                ));
            } else {
                console::time_stamp(
                    "Action",
                    Some(&start_time),
                    Some(&update_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("primary-dark"),
                );
            }
        }
        if render_start_time > update_time {
            // Log the time from when we called setState until we started rendering.
            let label = if is_pinged_update {
                "Promise Resolved"
            } else if render_start_time - update_time > 5.0 {
                "Update Blocked"
            } else {
                "Update"
            };
            if __DEV__ {
                let mut properties = Vec::new();
                if let Some(update_component_name) = update_component_name {
                    properties.push((
                        "Component name".to_string(),
                        update_component_name.to_string(),
                    ));
                }
                if let Some(update_method_name) = update_method_name {
                    properties.push(("Method name".to_string(), update_method_name.to_string()));
                }
                let measure_options = PerformanceMeasureOptions {
                    start: Some(update_time.into()),
                    end: Some(render_start_time.into()),
                    detail: Some(&MeasureDetail {
                        devtools: DevToolsDetails {
                            properties: Some(&properties),
                            track: CURRENT_TRACK.get(),
                            track_group: Some(LANES_TRACK_GROUP),
                            color: "primary-light",
                            tooltip_text: None,
                            performance_issue: None,
                        },
                    }),
                    ..Default::default()
                };

                if let Some(debug_task) = debug_task {
                    debug_task.run(function::bind(
                        Performance::measure,
                        Some(&performance),
                        (
                            performance,
                            label,
                            Some(&measure_options as &dyn IntoStrOrPerformanceMeasureOptions),
                            None,
                        ),
                    ));
                } else {
                    performance.measure(label, Some(&measure_options), None);
                }
                performance.clear_measures(Some(label));
            } else {
                console::time_stamp(
                    label,
                    Some(&update_time),
                    Some(&render_start_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("primary-light"),
                );
            }
        }
    }
}

pub fn log_render_phase(
    start_time: f64,
    end_time: f64,
    lanes: Lanes,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        let color = if includes_only_hydration_or_offscreen_lanes(lanes) {
            "tertiary-dark"
        } else {
            "primary-dark"
        };
        let label = if includes_only_offscreen_lanes(lanes) {
            "Prepared"
        } else if includes_only_hydration_lanes(lanes) {
            "Hydrated"
        } else {
            "Render"
        };
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    label,
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                ),
            ));
        } else {
            console::time_stamp(
                label,
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some(color),
            );
        }
    }
}

pub fn log_interrupted_render_phase(
    start_time: f64,
    end_time: f64,
    lanes: Lanes,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        let color = if includes_only_hydration_or_offscreen_lanes(lanes) {
            "tertiary-dark"
        } else {
            "primary-dark"
        };
        let label = if includes_only_offscreen_lanes(lanes) {
            "Prewarm"
        } else if includes_only_hydration_lanes(lanes) {
            "Interrupted Hydration"
        } else {
            "Interrupted Render"
        };
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    label,
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                ),
            ));
        } else {
            console::time_stamp(
                label,
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some(color),
            );
        }
    }
}

pub fn log_suspended_render_phase(
    start_time: f64,
    end_time: f64,
    lanes: Lanes,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        let color = if includes_only_hydration_or_offscreen_lanes(lanes) {
            "tertiary-dark"
        } else {
            "primary-dark"
        };
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Prewarm",
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                ),
            ));
        } else {
            console::time_stamp(
                "Prewarm",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some(color),
            );
        }
    }
}

pub fn log_suspended_with_delay_phase(
    start_time: f64,
    end_time: f64,
    lanes: Lanes,
    debug_task: Option<&ConsoleTask>,
) {
    // This means the render was suspended and cannot commit until it gets unblocked.
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        let color = if includes_only_hydration_or_offscreen_lanes(lanes) {
            "tertiary-dark"
        } else {
            "primary-dark"
        };
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Suspended",
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(color),
                ),
            ));
        } else {
            console::time_stamp(
                "Suspended",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some(color),
            );
        }
    }
}

pub fn log_recovered_render_phase(
    start_time: f64,
    end_time: f64,
    _lanes: Lanes,
    recoverable_errors: &CapturedValues,
    hydration_failed: bool,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ {
            let mut properties: Vec<(String, String)> = Vec::new();
            let mut i = 0;
            while i < recoverable_errors.borrow().len() {
                let captured_value = Rc::clone(&recoverable_errors.borrow()[i]);
                let error = captured_value.value;
                let message = if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
                    && js::type_of(js::get(error, "message")) == "string"
                {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(js::get(error, "message"))
                } else {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(error)
                };
                properties.push(("Recoverable Error".to_string(), message));
                i += 1;
            }
            let options = PerformanceMeasureOptions {
                start: Some(start_time.into()),
                end: Some(end_time.into()),
                detail: Some(&MeasureDetail {
                    devtools: DevToolsDetails {
                        color: "primary-dark",
                        track: CURRENT_TRACK.get(),
                        track_group: Some(LANES_TRACK_GROUP),
                        tooltip_text: Some(if hydration_failed {
                            "Hydration Failed"
                        } else {
                            "Recovered after Error"
                        }),
                        properties: Some(&properties),
                        performance_issue: None,
                    },
                }),
                ..Default::default()
            };
            if let Some(debug_task) = debug_task {
                debug_task.run(function::bind(
                    Performance::measure,
                    Some(&performance),
                    (
                        performance,
                        "Recovered",
                        Some(&options as &dyn IntoStrOrPerformanceMeasureOptions),
                        None,
                    ),
                ));
            } else {
                performance.measure("Recovered", Some(&options), None);
            }
            performance.clear_measures(Some("Recovered"));
        } else {
            console::time_stamp(
                "Recovered",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("error"),
            );
        }
    }
}

pub fn log_errored_render_phase(
    start_time: f64,
    end_time: f64,
    _lanes: Lanes,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Errored",
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("error"),
                ),
            ));
        } else {
            console::time_stamp(
                "Errored",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("error"),
            );
        }
    }
}

pub fn log_inconsistent_render(start_time: f64, end_time: f64, debug_task: Option<&ConsoleTask>) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Teared Render",
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("error"),
                ),
            ));
        } else {
            console::time_stamp(
                "Teared Render",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("error"),
            );
        }
    }
}

pub fn log_suspended_commit_phase(
    start_time: f64,
    end_time: f64,
    reason: &str,
    debug_task: Option<&ConsoleTask>,
) {
    // This means the commit was suspended on CSS or images.
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        // TODO: Include the exact reason and URLs of what resources suspended.
        // TODO: This might also be Suspended while waiting on a View Transition.
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    reason,
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("secondary-light"),
                ),
            ));
        } else {
            console::time_stamp(
                reason,
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("secondary-light"),
            );
        }
    }
}

pub fn log_suspended_view_transition_phase(
    start_time: f64,
    end_time: f64,
    reason: &str,
    debug_task: Option<&ConsoleTask>,
) {
    // This means the commit was suspended on CSS or images.
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        // TODO: Include the exact reason and URLs of what resources suspended.
        // TODO: This might also be Suspended while waiting on a View Transition.
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    reason,
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("secondary-light"),
                ),
            ));
        } else {
            console::time_stamp(
                reason,
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("secondary-light"),
            );
        }
    }
}

pub fn log_commit_errored(
    start_time: f64,
    end_time: f64,
    errors: &CapturedValues,
    passive: bool,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ {
            let mut properties: Vec<(String, String)> = Vec::new();
            let mut i = 0;
            while i < errors.borrow().len() {
                let captured_value = Rc::clone(&errors.borrow()[i]);
                let error = captured_value.value;
                let message = if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
                    && js::type_of(js::get(error, "message")) == "string"
                {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(js::get(error, "message"))
                } else {
                    // eslint-disable-next-line react-internal/safe-string-coercion
                    js::string(error)
                };
                properties.push(("Error".to_string(), message));
                i += 1;
            }
            let options = PerformanceMeasureOptions {
                start: Some(start_time.into()),
                end: Some(end_time.into()),
                detail: Some(&MeasureDetail {
                    devtools: DevToolsDetails {
                        color: "error",
                        track: CURRENT_TRACK.get(),
                        track_group: Some(LANES_TRACK_GROUP),
                        tooltip_text: Some(if passive {
                            "Remaining Effects Errored"
                        } else {
                            "Commit Errored"
                        }),
                        properties: Some(&properties),
                        performance_issue: None,
                    },
                }),
                ..Default::default()
            };
            if let Some(debug_task) = debug_task {
                debug_task.run(function::bind(
                    Performance::measure,
                    Some(&performance),
                    (
                        performance,
                        "Errored",
                        Some(&options as &dyn IntoStrOrPerformanceMeasureOptions),
                        None,
                    ),
                ));
            } else {
                performance.measure("Errored", Some(&options), None);
            }
            performance.clear_measures(Some("Errored"));
        } else {
            console::time_stamp(
                "Errored",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("error"),
            );
        }
    }
}

pub fn log_commit_phase(
    start_time: f64,
    end_time: f64,
    errors: Option<&CapturedValues>,
    aborted_view_transition: bool,
    debug_task: Option<&ConsoleTask>,
) {
    if let Some(errors) = errors {
        log_commit_errored(start_time, end_time, errors, false, debug_task);
        return;
    }
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    if aborted_view_transition {
                        "Commit Interrupted View Transition"
                    } else {
                        "Commit"
                    },
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(if aborted_view_transition {
                        "error"
                    } else {
                        "secondary-dark"
                    }),
                ),
            ));
        } else {
            console::time_stamp(
                if aborted_view_transition {
                    "Commit Interrupted View Transition"
                } else {
                    "Commit"
                },
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some(if aborted_view_transition {
                    "error"
                } else {
                    "secondary-dark"
                }),
            );
        }
    }
}

pub fn log_paint_yield_phase(
    start_time: f64,
    end_time: f64,
    delayed_until_paint: bool,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    if delayed_until_paint {
                        "Waiting for Paint"
                    } else {
                        "Waiting"
                    },
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("secondary-light"),
                ),
            ));
        } else {
            console::time_stamp(
                if delayed_until_paint {
                    "Waiting for Paint"
                } else {
                    "Waiting"
                },
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("secondary-light"),
            );
        }
    }
}

pub fn log_apply_gesture_phase(start_time: f64, end_time: f64, debug_task: Option<&ConsoleTask>) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Create Ghost Tree",
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("secondary-dark"),
                ),
            ));
        } else {
            console::time_stamp(
                "Create Ghost Tree",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("secondary-dark"),
            );
        }
    }
}

pub fn log_start_view_transition_yield_phase(
    start_time: f64,
    end_time: f64,
    aborted_view_transition: bool,
    debug_task: Option<&ConsoleTask>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    if aborted_view_transition {
                        "Interrupted View Transition"
                    } else {
                        "Starting Animation"
                    },
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some(if aborted_view_transition {
                        "error"
                    } else {
                        "secondary-light"
                    }),
                ),
            ));
        } else {
            console::time_stamp(
                if aborted_view_transition {
                    "Interrupted View Transition"
                } else {
                    "Starting Animation"
                },
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some(if aborted_view_transition {
                    " error"
                } else {
                    "secondary-light"
                }),
            );
        }
    }
}

pub fn log_animating_phase(start_time: f64, end_time: f64, debug_task: Option<&ConsoleTask>) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Animating",
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("secondary-dark"),
                ),
            ));
        } else {
            console::time_stamp(
                "Animating",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("secondary-dark"),
            );
        }
    }
}

pub fn log_passive_commit_phase(
    start_time: f64,
    end_time: f64,
    errors: Option<&CapturedValues>,
    debug_task: Option<&ConsoleTask>,
) {
    if let Some(errors) = errors {
        log_commit_errored(start_time, end_time, errors, true, debug_task);
        return;
    }
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        if end_time <= start_time {
            return;
        }
        if __DEV__ && let Some(debug_task) = debug_task {
            let (start_time, end_time): (&dyn IntoStrOrNumber, &dyn IntoStrOrNumber) =
                (&start_time, &end_time);
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    "Remaining Effects",
                    Some(start_time),
                    Some(end_time),
                    Some(CURRENT_TRACK.get()),
                    Some(LANES_TRACK_GROUP),
                    Some("secondary-dark"),
                ),
            ));
        } else {
            console::time_stamp(
                "Remaining Effects",
                Some(&start_time),
                Some(&end_time),
                Some(CURRENT_TRACK.get()),
                Some(LANES_TRACK_GROUP),
                Some("secondary-dark"),
            );
        }
    }
}
