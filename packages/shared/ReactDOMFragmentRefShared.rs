// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::Unknown;
use react_reconciler::react_fiber_tree_reflection::get_fragment_instance_or_text_instance_siblings;
use react_reconciler::react_internal_types::Fiber;
use webapi::{Node, node};

pub fn compare_document_position_for_empty_fragment(
    fragment_fiber: &Rc<Fiber>,
    parent_host_instance: &Node,
    other_node: &Node,
    get_public_instance: &dyn Fn(&Fiber) -> &'static Unknown,
) -> f64 {
    // If the fragment has no children, we can use the parent and
    // siblings to determine a position.
    let parent_result = parent_host_instance.compare_document_position(other_node) as u16;
    let mut result = parent_result;
    if parent_host_instance == other_node {
        result = node::DOCUMENT_POSITION_CONTAINS as u16;
    } else if parent_result & node::DOCUMENT_POSITION_CONTAINED_BY as u16 != 0 {
        // otherNode is one of the fragment's siblings. Use the next
        // host sibling (via the parent tree, not fiber.sibling) to
        // determine if its preceding or following.
        let [_, next_sibling_fiber] =
            get_fragment_instance_or_text_instance_siblings(fragment_fiber);
        match next_sibling_fiber {
            None => result = node::DOCUMENT_POSITION_PRECEDING as u16,
            Some(next_sibling_fiber) => {
                let next_sibling_instance: &Node =
                    unsafe { js::cast(Some(get_public_instance(&next_sibling_fiber))) };
                let next_sibling_result =
                    next_sibling_instance.compare_document_position(other_node) as u16;
                result = if next_sibling_result == 0
                    || next_sibling_result & node::DOCUMENT_POSITION_FOLLOWING as u16 != 0
                {
                    node::DOCUMENT_POSITION_FOLLOWING as u16
                } else {
                    node::DOCUMENT_POSITION_PRECEDING as u16
                };
            }
        }
    }

    result |= node::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC as u16;
    f64::from(result)
}
