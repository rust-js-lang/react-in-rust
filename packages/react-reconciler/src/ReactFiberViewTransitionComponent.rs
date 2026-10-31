// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use shared::react_types::{ViewTransitionClass, ViewTransitionProps};

pub use crate::react_fiber::ViewTransitionState;
use crate::react_fiber_work_loop::{get_committing_root, get_pending_transition_types};
use crate::react_internal_types::FiberRoot;

thread_local! {
    static GLOBAL_CLIENT_ID_COUNTER: Cell<f64> = const { Cell::new(0.0) };
}

pub fn get_view_transition_name(
    props: &ViewTransitionProps,
    instance: &ViewTransitionState,
) -> String {
    if let Some(name) = &props.name
        && name != "auto"
    {
        return name.clone();
    }
    if let Some(auto_name) = &*instance.auto_name.borrow() {
        return auto_name.clone();
    }

    // We assume we always call this in the commit phase.
    let root: &FiberRoot = unsafe { js::cast(get_committing_root().map(js::unknown_of)) };
    let identifier_prefix = &root.identifier_prefix;
    let global_client_id = GLOBAL_CLIENT_ID_COUNTER.get();
    GLOBAL_CLIENT_ID_COUNTER.set(global_client_id + 1.0);
    let name = "_".to_string()
        + identifier_prefix
        + "t_"
        + &js::number::to_string(global_client_id, Some(32))
        + "_";
    instance.auto_name.replace(Some(name.clone()));
    name
}

fn get_class_name_by_type(class_by_type: Option<ViewTransitionClass>) -> Option<String> {
    let class_by_type = match class_by_type {
        None => return None,
        Some(ViewTransitionClass::Name(class_name)) => return Some(class_name.to_string()),
        Some(ViewTransitionClass::PerType(class_by_type)) => class_by_type,
    };
    let mut class_name: Option<String> = None;
    let active_types = get_pending_transition_types();
    if let Some(active_types) = active_types {
        for active_type in active_types.borrow().iter() {
            let r#match = js::dict::get(class_by_type, active_type);
            if let Some(r#match) = r#match {
                if r#match == "none" {
                    // If anything matches "none" that takes precedence over any other
                    // type that also matches.
                    return Some("none".to_string());
                }
                match &mut class_name {
                    None => class_name = Some(r#match.clone()),
                    Some(class_name) => *class_name += &(" ".to_string() + r#match),
                }
            }
        }
    }
    if class_name.is_none() {
        // We had no other matches. Match the default for this configuration.
        return js::dict::get(class_by_type, "default").cloned();
    }
    class_name
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_view_transition_class_name(
    default_class: Option<ViewTransitionClass>,
    event_class: Option<ViewTransitionClass>,
) -> Option<String> {
    let class_name = get_class_name_by_type(default_class);
    let event_class_name = get_class_name_by_type(event_class);
    let Some(event_class_name) = event_class_name else {
        return if class_name.as_deref() == Some("auto") {
            None
        } else {
            class_name
        };
    };
    if event_class_name == "auto" {
        return None;
    }
    Some(event_class_name)
}
