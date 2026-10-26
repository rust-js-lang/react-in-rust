// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Dict, Unknown};

pub use crate::react_fiber::ViewTransitionState;
use crate::react_internal_types::FiberRoot;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getCommittingRoot"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_committing_root() -> Option<Rc<FiberRoot>> {
    unreachable!()
}

// react/src/ReactTransitionType's `TransitionTypes`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getPendingTransitionTypes"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_pending_transition_types() -> Option<Rc<RefCell<Vec<String>>>> {
    unreachable!()
}

/// shared/ReactTypes' `ViewTransitionClass`: a class name, `'none'` or
/// `'auto'` among them, or one for each transition type.
#[derive(Clone, Copy)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ViewTransitionClass {
    Name(&'static str),
    // shared/ReactTypes' `ViewTransitionClassPerType`, whose `default` is
    // the class of a type it has none for.
    #[cfg_attr(rust_js, rust_js::otherwise)]
    PerType(&'static Dict<String>),
}

/// shared/ReactTypes' `GestureOptionsRequired`.
pub struct GestureOptionsRequired {
    pub range_start: f64,
    pub range_end: f64,
}

/// What a `<ViewTransition>` calls as it animates, given its
/// ReactFiberConfig's `ViewTransitionInstance` and the transition's types:
/// what cleans up after it, if anything.
pub type ViewTransitionCallback = dyn Fn(&'static Unknown, &[String]) -> Option<&'static dyn Fn()>;

/// What a `<ViewTransition>` calls as a gesture animates it, given the
/// ReactFiberConfig's `GestureTimeline` first.
pub type ViewTransitionGestureCallback = dyn Fn(
    &'static Unknown,
    &GestureOptionsRequired,
    &'static Unknown,
    &[String],
) -> Option<&'static dyn Fn()>;

/// shared/ReactTypes' `ViewTransitionProps`.
pub struct ViewTransitionProps {
    pub name: Option<String>,
    // shared/ReactTypes' `ReactNodeList`.
    pub children: Option<&'static Unknown>,
    pub default: Option<ViewTransitionClass>,
    pub enter: Option<ViewTransitionClass>,
    pub exit: Option<ViewTransitionClass>,
    pub share: Option<ViewTransitionClass>,
    pub update: Option<ViewTransitionClass>,
    pub parent_enter: Option<ViewTransitionClass>,
    pub parent_exit: Option<ViewTransitionClass>,
    pub on_enter: Option<&'static ViewTransitionCallback>,
    pub on_exit: Option<&'static ViewTransitionCallback>,
    pub on_parent_enter: Option<&'static ViewTransitionCallback>,
    pub on_parent_exit: Option<&'static ViewTransitionCallback>,
    pub on_share: Option<&'static ViewTransitionCallback>,
    pub on_update: Option<&'static ViewTransitionCallback>,
    pub on_gesture_enter: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_exit: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_parent_enter: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_parent_exit: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_share: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_update: Option<&'static ViewTransitionGestureCallback>,
}

unsafe impl js::Defined for ViewTransitionProps {}

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
