// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_reconciler::react_internal_types::Fiber;
use webapi::Node;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/client/ReactDOMComponentTree.js#getInstanceFromNode",
    rust_js::nullable
)]
fn get_instance_from_node(node: &Node) -> Option<Rc<Fiber>> {
    let _ = node;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/client/ReactDOMComponentTree.js#getFiberCurrentPropsFromNode",
    rust_js::nullable
)]
fn get_fiber_current_props_from_node(node: &Unknown) -> Option<&'static Unknown> {
    let _ = node;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/client/ReactDOMComponent.js#restoreControlledState",
    rust_js::nullable(props)
)]
fn restore_controlled_state(
    dom_element: &Unknown,
    tag: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) {
    let _ = (dom_element, tag, props);
    unreachable!()
}

// Use to restore controlled state after a change event has fired.

thread_local! {
    static RESTORE_TARGET: Cell<Option<&'static Node>> = const { Cell::new(js::null()) };
    static RESTORE_QUEUE: RefCell<Option<Vec<&'static Node>>> = const { RefCell::new(js::null()) };
}

fn restore_state_of_target(target: &Node) {
    // We perform this translation at the end of the event loop so that we
    // always receive the correct fiber here
    let Some(internal_instance) = get_instance_from_node(target) else {
        // Unmounted
        return;
    };

    let state_node = internal_instance.state_node.get();
    // Guard against Fiber being unmounted.
    if let Some(state_node) = state_node {
        let props = get_fiber_current_props_from_node(state_node);
        restore_controlled_state(state_node, internal_instance.r#type.get(), props);
    }
}

pub fn enqueue_state_restore(target: &'static Node) {
    if RESTORE_TARGET.get().is_some() {
        RESTORE_QUEUE.with_borrow_mut(|restore_queue| match restore_queue {
            Some(restore_queue) => restore_queue.push(target),
            None => *restore_queue = Some(vec![target]),
        });
    } else {
        RESTORE_TARGET.set(Some(target));
    }
}

pub fn needs_state_restore() -> bool {
    RESTORE_TARGET.get().is_some() || RESTORE_QUEUE.with_borrow(Option::is_some)
}

pub fn restore_state_if_needed() {
    let Some(target) = RESTORE_TARGET.take() else {
        return;
    };
    let queued_targets = RESTORE_QUEUE.take();

    restore_state_of_target(target);
    if let Some(queued_targets) = queued_targets {
        for target in queued_targets {
            restore_state_of_target(target);
        }
    }
}
