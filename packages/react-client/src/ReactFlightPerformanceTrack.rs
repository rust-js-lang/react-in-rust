// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

/* eslint-disable react-internal/no-production-logging */

use js::{Kind, Unknown, function};
use shared::environment::__DEV__;
use shared::react_feature_flags::ENABLE_PROFILER_TIMER;
use shared::react_io_description::get_io_description;
use shared::react_performance_track_properties::{
    add_object_to_properties, add_value_to_properties,
};
use shared::react_types::{ReactAsyncInfo, ReactComponentInfo, ReactIOInfo};
use webapi::{
    IntoStrOrNumber, IntoStrOrPerformanceMeasureOptions, Performance, PerformanceMeasureOptions,
    console, performance,
};

/// What DevTools' performance panel reads of a measure's `detail.devtools`:
/// the entry's track, its color, and what its tooltip shows.
pub struct DevToolsDetails<'a> {
    pub color: &'a str,
    pub track: Option<&'a str>,
    pub track_group: Option<&'a str>,
    pub properties: &'a [(String, String)],
    pub tooltip_text: Option<&'a str>,
}

/// A measure's `detail`, which DevTools reads.
pub struct MeasureDetail<'a> {
    pub devtools: DevToolsDetails<'a>,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "supportsUserTiming")]
    static SUPPORTS_USER_TIMING: bool = ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && js::is_defined(console)
        && js::is_function(console::time_stamp)
        && js::is_defined(performance)
        && js::is_function(Performance::measure);
}

const IO_TRACK: &str = "Server Requests ⚛";
const COMPONENTS_TRACK: &str = "Server Components ⚛";

pub fn mark_all_tracks_in_order() {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        // Ensure we create the Server Component track groups earlier than the Client Scheduler
        // and Client Components. We can always add the 0 time slot even if it's in the past.
        // That's still considered for ordering.
        console::time_stamp(
            "Server Requests Track",
            Some(&0.001),
            Some(&0.001),
            Some(IO_TRACK),
            None,
            Some("primary-light"),
        );
        console::time_stamp(
            "Server Components Track",
            Some(&0.001),
            Some(&0.001),
            Some("Primary"),
            Some(COMPONENTS_TRACK),
            Some("primary-light"),
        );
    }
}

#[cfg_attr(rust_js, rust_js::name = "trackNames")]
static TRACK_NAMES: [&str; 10] = [
    "Primary",
    "Parallel",
    "Parallel\u{200b}", // Padded with zero-width space to give each track a unique name.
    "Parallel\u{200b}\u{200b}",
    "Parallel\u{200b}\u{200b}\u{200b}",
    "Parallel\u{200b}\u{200b}\u{200b}\u{200b}",
    "Parallel\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}",
    "Parallel\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}",
    "Parallel\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}",
    "Parallel\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}",
];

pub fn log_component_render(
    component_info: &ReactComponentInfo,
    track_idx: usize,
    start_time: f64,
    end_time: f64,
    children_end_time: f64,
    root_env: &str,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) && children_end_time >= 0.0 && track_idx < 10
    {
        let env = &component_info.env;
        let name = &component_info.name;
        let is_primary_env = env.as_deref() == Some(root_env);
        let self_time = end_time - start_time;
        let color = if self_time < 0.5 {
            if is_primary_env {
                "primary-light"
            } else {
                "secondary-light"
            }
        } else if self_time < 50.0 {
            if is_primary_env {
                "primary"
            } else {
                "secondary"
            }
        } else if self_time < 500.0 {
            if is_primary_env {
                "primary-dark"
            } else {
                "secondary-dark"
            }
        } else {
            "error"
        };
        let entry_name = match env {
            Some(env) if !is_primary_env => name.clone() + " [" + env + "]",
            _ => name.clone(),
        };
        let debug_task = component_info.debug_task.get();
        let measure_name = "\u{200b}".to_owned() + &entry_name;
        if __DEV__ && let Some(debug_task) = debug_task {
            let mut properties: Vec<(String, String)> = Vec::new();
            if let Some(key) = component_info.key.clone() {
                add_value_to_properties("key", Some(js::unknown_of(key)), &mut properties, 0, "");
            }
            if let Some(props) = component_info.props {
                add_object_to_properties(js::unknown_of(props), &mut properties, 0, "");
            }

            debug_task.run(function::bind(
                Performance::measure,
                Some(&performance),
                (
                    performance,
                    measure_name.as_str(),
                    Some(&PerformanceMeasureOptions {
                        start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                        end: Some(children_end_time.into()),
                        detail: Some(&MeasureDetail {
                            devtools: DevToolsDetails {
                                color,
                                track: TRACK_NAMES.get(track_idx).copied(),
                                track_group: Some(COMPONENTS_TRACK),
                                properties: &properties,
                                tooltip_text: None,
                            },
                        }),
                        ..Default::default()
                    } as &dyn IntoStrOrPerformanceMeasureOptions),
                    None,
                ),
            ));
            performance.clear_measures(Some(&measure_name));
        } else {
            console::time_stamp(
                &measure_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&children_end_time),
                TRACK_NAMES.get(track_idx).copied(),
                Some(COMPONENTS_TRACK),
                Some(color),
            );
        }
    }
}

pub fn log_component_aborted(
    component_info: &ReactComponentInfo,
    track_idx: usize,
    start_time: f64,
    _end_time: f64,
    children_end_time: f64,
    root_env: &str,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let env = &component_info.env;
        let name = &component_info.name;
        let is_primary_env = env.as_deref() == Some(root_env);
        let entry_name = match env {
            Some(env) if !is_primary_env => name.clone() + " [" + env + "]",
            _ => name.clone(),
        };
        let measure_name = "\u{200b}".to_owned() + &entry_name;
        if __DEV__ {
            let mut properties: Vec<(String, String)> = vec![(
                "Aborted".to_owned(),
                "The stream was aborted before this Component finished rendering.".to_owned(),
            )];
            if let Some(key) = component_info.key.clone() {
                add_value_to_properties("key", Some(js::unknown_of(key)), &mut properties, 0, "");
            }
            if let Some(props) = component_info.props {
                add_object_to_properties(js::unknown_of(props), &mut properties, 0, "");
            }

            performance.measure(
                &measure_name,
                Some(&PerformanceMeasureOptions {
                    start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                    end: Some(children_end_time.into()),
                    detail: Some(&MeasureDetail {
                        devtools: DevToolsDetails {
                            color: "warning",
                            track: TRACK_NAMES.get(track_idx).copied(),
                            track_group: Some(COMPONENTS_TRACK),
                            tooltip_text: Some(&(entry_name.clone() + " Aborted")),
                            properties: &properties,
                        },
                    }),
                    ..Default::default()
                }),
                None,
            );
            performance.clear_measures(Some(&measure_name));
        } else {
            console::time_stamp(
                &measure_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&children_end_time),
                TRACK_NAMES.get(track_idx).copied(),
                Some(COMPONENTS_TRACK),
                Some("warning"),
            );
        }
    }
}

pub fn log_component_errored(
    component_info: &ReactComponentInfo,
    track_idx: usize,
    start_time: f64,
    _end_time: f64,
    children_end_time: f64,
    root_env: &str,
    error: Option<&Unknown>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) {
        let env = &component_info.env;
        let name = &component_info.name;
        let is_primary_env = env.as_deref() == Some(root_env);
        let entry_name = match env {
            Some(env) if !is_primary_env => name.clone() + " [" + env + "]",
            _ => name.clone(),
        };
        let measure_name = "\u{200b}".to_owned() + &entry_name;
        if __DEV__ {
            let message = if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
                && js::type_of(js::get(error, "message")) == "string"
            {
                // eslint-disable-next-line react-internal/safe-string-coercion
                js::string(js::get(error, "message"))
            } else {
                // eslint-disable-next-line react-internal/safe-string-coercion
                js::string(error)
            };
            let mut properties: Vec<(String, String)> = vec![("Error".to_owned(), message)];
            if let Some(key) = component_info.key.clone() {
                add_value_to_properties("key", Some(js::unknown_of(key)), &mut properties, 0, "");
            }
            if let Some(props) = component_info.props {
                add_object_to_properties(js::unknown_of(props), &mut properties, 0, "");
            }

            performance.measure(
                &measure_name,
                Some(&PerformanceMeasureOptions {
                    start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                    end: Some(children_end_time.into()),
                    detail: Some(&MeasureDetail {
                        devtools: DevToolsDetails {
                            color: "error",
                            track: TRACK_NAMES.get(track_idx).copied(),
                            track_group: Some(COMPONENTS_TRACK),
                            tooltip_text: Some(&(entry_name.clone() + " Errored")),
                            properties: &properties,
                        },
                    }),
                    ..Default::default()
                }),
                None,
            );
            performance.clear_measures(Some(&measure_name));
        } else {
            console::time_stamp(
                &measure_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&children_end_time),
                TRACK_NAMES.get(track_idx).copied(),
                Some(COMPONENTS_TRACK),
                Some("error"),
            );
        }
    }
}

pub fn log_deduped_component_render(
    component_info: &ReactComponentInfo,
    track_idx: usize,
    start_time: f64,
    end_time: f64,
    root_env: &str,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) && end_time >= 0.0 && track_idx < 10 {
        let env = &component_info.env;
        let name = &component_info.name;
        let is_primary_env = env.as_deref() == Some(root_env);
        let color = if is_primary_env {
            "primary-light"
        } else {
            "secondary-light"
        };
        let entry_name = name.clone() + " [deduped]";
        let debug_task = component_info.debug_task.get();
        if __DEV__ && let Some(debug_task) = debug_task {
            debug_task.run(function::bind(
                console::time_stamp,
                Some(&console),
                (
                    entry_name.as_str(),
                    Some(
                        &(if start_time < 0.0 { 0.0 } else { start_time }) as &dyn IntoStrOrNumber,
                    ),
                    Some(&end_time as &dyn IntoStrOrNumber),
                    TRACK_NAMES.get(track_idx).copied(),
                    Some(COMPONENTS_TRACK),
                    Some(color),
                ),
            ));
        } else {
            console::time_stamp(
                &entry_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&end_time),
                TRACK_NAMES.get(track_idx).copied(),
                Some(COMPONENTS_TRACK),
                Some(color),
            );
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "getIOColor")]
fn get_io_color(function_name: &str) -> &'static str {
    // Add some color variation to be able to distinguish various sources.
    match js::string::char_code_at(function_name, 0) % 3.0 {
        0.0 => "tertiary-light",
        1.0 => "tertiary",
        _ => "tertiary-dark",
    }
}

#[cfg_attr(rust_js, rust_js::name = "getIOLongName")]
fn get_io_long_name(
    io_info: &ReactIOInfo,
    description: &str,
    env: Option<&str>,
    root_env: &str,
) -> String {
    let name = io_info.name.get();
    let long_name = if description.is_empty() {
        name.to_owned()
    } else {
        name.to_owned() + " (" + description + ")"
    };
    let is_primary_env = env == Some(root_env);
    match env {
        Some(env) if !is_primary_env => long_name + " [" + env + "]",
        _ => long_name,
    }
}

#[cfg_attr(rust_js, rust_js::name = "getIOShortName")]
fn get_io_short_name(
    io_info: &ReactIOInfo,
    description: &str,
    env: Option<&str>,
    root_env: &str,
) -> String {
    let name = io_info.name.get();
    let is_primary_env = env == Some(root_env);
    let env_suffix = match env {
        Some(env) if !is_primary_env => " [".to_owned() + env + "]",
        _ => String::new(),
    };
    let mut desc = String::new();
    let desc_max_length =
        30.0 - f64::from(js::string::length(name)) - f64::from(js::string::length(&env_suffix));
    if desc_max_length > 1.0 {
        let l = f64::from(js::string::length(description));
        if l > 0.0 && l <= desc_max_length {
            // We can fit the full description
            desc = " (".to_owned() + description + ")";
        } else if description.starts_with("http://")
            || description.starts_with("https://")
            || description.starts_with('/')
        {
            // Looks like a URL. Let's see if we can extract something shorter.
            // We don't have to do a full parse so let's try something cheaper.
            let mut query_idx = js::string::index_of(description, "?", None);
            if query_idx == -1.0 {
                query_idx = f64::from(js::string::length(description));
            }
            if js::string::char_code_at(description, query_idx - 1.0) == 47.0
            /* "/" */
            {
                // Ends with slash. Look before that.
                query_idx -= 1.0;
            }
            let slash_idx = js::string::last_index_of(description, "/", Some(query_idx - 1.0));
            if query_idx - slash_idx < desc_max_length {
                // This may now be either the file name or the host.
                // Include the slash to make it more obvious what we trimmed.
                desc = " (…".to_owned()
                    + &js::string::slice(description, slash_idx, Some(query_idx))
                    + ")";
            } else {
                // cut out the middle to not exceed the max length
                let start = js::string::slice(
                    description,
                    slash_idx,
                    Some(slash_idx + desc_max_length / 2.0),
                );
                let end = js::string::slice(
                    description,
                    query_idx - desc_max_length / 2.0,
                    Some(query_idx),
                );
                desc = " (".to_owned()
                    + (if slash_idx > 0.0 { "…" } else { "" })
                    + &start
                    + "…"
                    + &end
                    + ")";
            }
        }
    }
    name.to_owned() + &desc + &env_suffix
}

pub fn log_component_await_aborted(
    async_info: &ReactAsyncInfo,
    track_idx: usize,
    start_time: f64,
    end_time: f64,
    root_env: &str,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) && end_time > 0.0 {
        let entry_name = "await ".to_owned()
            + &get_io_short_name(&async_info.awaited, "", async_info.env.as_deref(), root_env);
        let debug_task = async_info
            .debug_task
            .get()
            .or(async_info.awaited.debug_task.get());
        if __DEV__ && let Some(debug_task) = debug_task {
            let properties = [(
                "Aborted".to_owned(),
                "The stream was aborted before this Promise resolved.".to_owned(),
            )];
            let tooltip_text =
                get_io_long_name(&async_info.awaited, "", async_info.env.as_deref(), root_env)
                    + " Aborted";
            debug_task.run(function::bind(
                Performance::measure,
                Some(&performance),
                (
                    performance,
                    entry_name.as_str(),
                    Some(&PerformanceMeasureOptions {
                        start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                        end: Some(end_time.into()),
                        detail: Some(&MeasureDetail {
                            devtools: DevToolsDetails {
                                color: "warning",
                                track: TRACK_NAMES.get(track_idx).copied(),
                                track_group: Some(COMPONENTS_TRACK),
                                properties: &properties,
                                tooltip_text: Some(&tooltip_text),
                            },
                        }),
                        ..Default::default()
                    } as &dyn IntoStrOrPerformanceMeasureOptions),
                    None,
                ),
            ));
            performance.clear_measures(Some(&entry_name));
        } else {
            console::time_stamp(
                &entry_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&end_time),
                TRACK_NAMES.get(track_idx).copied(),
                Some(COMPONENTS_TRACK),
                Some("warning"),
            );
        }
    }
}

pub fn log_component_await_errored(
    async_info: &ReactAsyncInfo,
    track_idx: usize,
    start_time: f64,
    end_time: f64,
    root_env: &str,
    error: Option<&Unknown>,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) && end_time > 0.0 {
        let description = get_io_description(error);
        let entry_name = "await ".to_owned()
            + &get_io_short_name(
                &async_info.awaited,
                &description,
                async_info.env.as_deref(),
                root_env,
            );
        let debug_task = async_info
            .debug_task
            .get()
            .or(async_info.awaited.debug_task.get());
        if __DEV__ && let Some(debug_task) = debug_task {
            let message = if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
                && js::type_of(js::get(error, "message")) == "string"
            {
                // eslint-disable-next-line react-internal/safe-string-coercion
                js::string(js::get(error, "message"))
            } else {
                // eslint-disable-next-line react-internal/safe-string-coercion
                js::string(error)
            };
            let properties = [("Rejected".to_owned(), message)];
            let tooltip_text = get_io_long_name(
                &async_info.awaited,
                &description,
                async_info.env.as_deref(),
                root_env,
            ) + " Rejected";
            debug_task.run(function::bind(
                Performance::measure,
                Some(&performance),
                (
                    performance,
                    entry_name.as_str(),
                    Some(&PerformanceMeasureOptions {
                        start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                        end: Some(end_time.into()),
                        detail: Some(&MeasureDetail {
                            devtools: DevToolsDetails {
                                color: "error",
                                track: TRACK_NAMES.get(track_idx).copied(),
                                track_group: Some(COMPONENTS_TRACK),
                                properties: &properties,
                                tooltip_text: Some(&tooltip_text),
                            },
                        }),
                        ..Default::default()
                    } as &dyn IntoStrOrPerformanceMeasureOptions),
                    None,
                ),
            ));
            performance.clear_measures(Some(&entry_name));
        } else {
            console::time_stamp(
                &entry_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&end_time),
                TRACK_NAMES.get(track_idx).copied(),
                Some(COMPONENTS_TRACK),
                Some("error"),
            );
        }
    }
}

pub fn log_component_await(
    async_info: &ReactAsyncInfo,
    track_idx: usize,
    start_time: f64,
    end_time: f64,
    root_env: &str,
    value: &Unknown,
) {
    if SUPPORTS_USER_TIMING.with(|&supports| supports) && end_time > 0.0 {
        let description = get_io_description(Some(value));
        let name = get_io_short_name(
            &async_info.awaited,
            &description,
            async_info.env.as_deref(),
            root_env,
        );
        let entry_name = "await ".to_owned() + &name;
        let color = get_io_color(&name);
        let debug_task = async_info
            .debug_task
            .get()
            .or(async_info.awaited.debug_task.get());
        if __DEV__ && let Some(debug_task) = debug_task {
            let mut properties: Vec<(String, String)> = Vec::new();
            if let Some(object) = js::classify(Some(value)).object() {
                add_object_to_properties(object, &mut properties, 0, "");
            } else if !matches!(js::classify(Some(value)), Kind::Undefined) {
                add_value_to_properties("awaited value", Some(value), &mut properties, 0, "");
            }
            let tooltip_text = get_io_long_name(
                &async_info.awaited,
                &description,
                async_info.env.as_deref(),
                root_env,
            );
            debug_task.run(function::bind(
                Performance::measure,
                Some(&performance),
                (
                    performance,
                    entry_name.as_str(),
                    Some(&PerformanceMeasureOptions {
                        start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                        end: Some(end_time.into()),
                        detail: Some(&MeasureDetail {
                            devtools: DevToolsDetails {
                                color,
                                track: TRACK_NAMES.get(track_idx).copied(),
                                track_group: Some(COMPONENTS_TRACK),
                                properties: &properties,
                                tooltip_text: Some(&tooltip_text),
                            },
                        }),
                        ..Default::default()
                    } as &dyn IntoStrOrPerformanceMeasureOptions),
                    None,
                ),
            ));
            performance.clear_measures(Some(&entry_name));
        } else {
            console::time_stamp(
                &entry_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&end_time),
                TRACK_NAMES.get(track_idx).copied(),
                Some(COMPONENTS_TRACK),
                Some(color),
            );
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "logIOInfoErrored")]
pub fn log_io_info_errored(io_info: &ReactIOInfo, root_env: &str, error: Option<&Unknown>) {
    let start_time = io_info.start.get();
    let end_time = io_info.end.get();
    if SUPPORTS_USER_TIMING.with(|&supports| supports) && end_time >= 0.0 {
        let description = get_io_description(error);
        let entry_name = get_io_short_name(io_info, &description, io_info.env.as_deref(), root_env);
        let debug_task = io_info.debug_task.get();
        let measure_name = "\u{200b}".to_owned() + &entry_name;
        if __DEV__ && let Some(debug_task) = debug_task {
            let message = if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
                && js::type_of(js::get(error, "message")) == "string"
            {
                // eslint-disable-next-line react-internal/safe-string-coercion
                js::string(js::get(error, "message"))
            } else {
                // eslint-disable-next-line react-internal/safe-string-coercion
                js::string(error)
            };
            let properties = [("rejected with".to_owned(), message)];
            let tooltip_text =
                get_io_long_name(io_info, &description, io_info.env.as_deref(), root_env)
                    + " Rejected";

            debug_task.run(function::bind(
                Performance::measure,
                Some(&performance),
                (
                    performance,
                    measure_name.as_str(),
                    Some(&PerformanceMeasureOptions {
                        start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                        end: Some(end_time.into()),
                        detail: Some(&MeasureDetail {
                            devtools: DevToolsDetails {
                                color: "error",
                                track: Some(IO_TRACK),
                                track_group: None,
                                properties: &properties,
                                tooltip_text: Some(&tooltip_text),
                            },
                        }),
                        ..Default::default()
                    } as &dyn IntoStrOrPerformanceMeasureOptions),
                    None,
                ),
            ));
            performance.clear_measures(Some(&measure_name));
        } else {
            console::time_stamp(
                &measure_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&end_time),
                Some(IO_TRACK),
                None,
                Some("error"),
            );
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "logIOInfo")]
pub fn log_io_info(io_info: &ReactIOInfo, root_env: &str, value: &Unknown) {
    let start_time = io_info.start.get();
    let end_time = io_info.end.get();
    if SUPPORTS_USER_TIMING.with(|&supports| supports) && end_time >= 0.0 {
        let description = get_io_description(Some(value));
        let entry_name = get_io_short_name(io_info, &description, io_info.env.as_deref(), root_env);
        let color = get_io_color(&entry_name);
        let debug_task = io_info.debug_task.get();
        let measure_name = "\u{200b}".to_owned() + &entry_name;
        if __DEV__ && let Some(debug_task) = debug_task {
            let mut properties: Vec<(String, String)> = Vec::new();
            if let Some(object) = js::classify(Some(value)).object() {
                add_object_to_properties(object, &mut properties, 0, "");
            } else if !matches!(js::classify(Some(value)), Kind::Undefined) {
                add_value_to_properties("Resolved", Some(value), &mut properties, 0, "");
            }
            let tooltip_text =
                get_io_long_name(io_info, &description, io_info.env.as_deref(), root_env);
            debug_task.run(function::bind(
                Performance::measure,
                Some(&performance),
                (
                    performance,
                    measure_name.as_str(),
                    Some(&PerformanceMeasureOptions {
                        start: Some((if start_time < 0.0 { 0.0 } else { start_time }).into()),
                        end: Some(end_time.into()),
                        detail: Some(&MeasureDetail {
                            devtools: DevToolsDetails {
                                color,
                                track: Some(IO_TRACK),
                                track_group: None,
                                properties: &properties,
                                tooltip_text: Some(&tooltip_text),
                            },
                        }),
                        ..Default::default()
                    } as &dyn IntoStrOrPerformanceMeasureOptions),
                    None,
                ),
            ));
            performance.clear_measures(Some(&measure_name));
        } else {
            console::time_stamp(
                &measure_name,
                Some(&if start_time < 0.0 { 0.0 } else { start_time }),
                Some(&end_time),
                Some(IO_TRACK),
                None,
                Some(color),
            );
        }
    }
}
