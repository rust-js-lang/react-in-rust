// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

use js::JsObject;
use shared::react_types::{ViewTransitionClass, ViewTransitionProps};

use crate::react_fizz_tree_context::{TreeContext, get_tree_id};

/// The renderer's resumable state, its Fizz config's.
pub struct ResumableState(PhantomData<JsObject>);

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFizzConfig.js#makeId")]
fn make_id(resumable_state: &ResumableState, tree_id: &str, local_id: f64) -> String {
    let _ = (resumable_state, tree_id, local_id);
    unreachable!()
}

pub fn get_view_transition_name(
    props: &ViewTransitionProps,
    tree_context: &TreeContext,
    resumable_state: &ResumableState,
) -> String {
    if let Some(name) = &props.name
        && name != "auto"
    {
        return name.clone();
    }
    let tree_id = get_tree_id(tree_context);
    make_id(resumable_state, &tree_id, 0.0)
}

fn get_class_name_by_type(class_by_type: Option<ViewTransitionClass>) -> Option<String> {
    let class_by_type = match class_by_type {
        None => return None,
        Some(ViewTransitionClass::Name(class_name)) => return Some(class_name.to_string()),
        Some(ViewTransitionClass::PerType(class_by_type)) => class_by_type,
    };
    let mut class_name: Option<String> = None;
    let active_types: Option<&[String]> = None; // TODO: Support passing active types.
    if let Some(active_types) = active_types {
        for active_type in active_types {
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
