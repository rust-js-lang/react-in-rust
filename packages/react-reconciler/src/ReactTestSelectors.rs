// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::rc::Rc;

use js::{JsObject, Kind, SymbolFor, Unknown};
use shared::get_component_name_from_type::get_component_name_from_type;

use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_work_tags::{HOST_COMPONENT, HOST_HOISTABLE, HOST_SINGLETON, HOST_TEXT};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsTestSelectors"]
    safe static SUPPORTS_TEST_SELECTORS: bool;
}

// ReactFiberConfig's test selector operations, on its `Instance`. The root
// findFiberRoot finds is a HostRoot fiber, as getInstanceFromNode gives it.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#findFiberRoot",
    rust_js::nullable
)]
fn find_fiber_root(node: &'static Unknown) -> Option<Rc<Fiber>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getBoundingRect",
    rust_js::gives_plain_data
)]
fn get_bounding_rect(node: &'static Unknown) -> BoundingRect {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getInstanceFromNode",
    rust_js::nullable
)]
fn get_instance_from_node(node: &Unknown) -> Option<Rc<Fiber>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getTextContent",
    rust_js::nullable
)]
fn get_text_content(fiber: &Fiber) -> Option<String> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isHiddenSubtree"
)]
fn is_hidden_subtree(fiber: &Fiber) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#matchAccessibilityRole"
)]
fn match_accessibility_role(node: Option<&'static Unknown>, role: &str) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#setFocusIfFocusable"
)]
fn set_focus_if_focusable(node: Option<&'static Unknown>) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#setupIntersectionObserver",
    rust_js::gives_plain_data
)]
fn setup_intersection_observer(
    targets: &[&'static Unknown],
    callback: &'static ObserveVisibleRectsCallback,
    options: Option<&'static IntersectionObserverOptions>,
) -> IntersectionObserverHandle {
    unreachable!()
}

/// What setupIntersectionObserver gives: the host observer, and what
/// changes what it observes.
struct IntersectionObserverHandle {
    disconnect: &'static dyn Fn(),
    observe: &'static dyn Fn(&'static Unknown),
    unobserve: &'static dyn Fn(&'static Unknown),
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "COMPONENT_TYPE")]
    static COMPONENT_TYPE: Cell<&'static Unknown> = Cell::new(js::unknown_of(0b000));
    #[cfg_attr(rust_js, rust_js::name = "HAS_PSEUDO_CLASS_TYPE")]
    static HAS_PSEUDO_CLASS_TYPE: Cell<&'static Unknown> = Cell::new(js::unknown_of(0b001));
    #[cfg_attr(rust_js, rust_js::name = "ROLE_TYPE")]
    static ROLE_TYPE: Cell<&'static Unknown> = Cell::new(js::unknown_of(0b010));
    #[cfg_attr(rust_js, rust_js::name = "TEST_NAME_TYPE")]
    static TEST_NAME_TYPE: Cell<&'static Unknown> = Cell::new(js::unknown_of(0b011));
    #[cfg_attr(rust_js, rust_js::name = "TEXT_TYPE")]
    static TEXT_TYPE: Cell<&'static Unknown> = Cell::new(js::unknown_of(0b100));
}

js::on_load! {
    if js::is_function(js::symbol::new) && js::is_function(SymbolFor) {
        let symbol_for = SymbolFor;
        COMPONENT_TYPE.set(js::unknown_of(symbol_for("selector.component")));
        HAS_PSEUDO_CLASS_TYPE.set(js::unknown_of(symbol_for("selector.has_pseudo_class")));
        ROLE_TYPE.set(js::unknown_of(symbol_for("selector.role")));
        TEST_NAME_TYPE.set(js::unknown_of(symbol_for("selector.test_id")));
        TEXT_TYPE.set(js::unknown_of(symbol_for("selector.text")));
    }
}

/// A test selector: what its `$$typeof` says it matches, and its value of
/// that, a component, the selectors a fiber has, a role, a text or a test
/// name.
pub struct Selector {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: &'static Unknown,
    pub value: &'static Unknown,
}

pub fn create_component_selector(component: &'static Unknown) -> Selector {
    Selector {
        r#typeof: COMPONENT_TYPE.get(),
        value: component,
    }
}

pub fn create_has_pseudo_class_selector(selectors: Vec<Selector>) -> Selector {
    Selector {
        r#typeof: HAS_PSEUDO_CLASS_TYPE.get(),
        value: js::unknown_of(selectors),
    }
}

pub fn create_role_selector(role: String) -> Selector {
    Selector {
        r#typeof: ROLE_TYPE.get(),
        value: js::unknown_of(role),
    }
}

pub fn create_text_selector(text: String) -> Selector {
    Selector {
        r#typeof: TEXT_TYPE.get(),
        value: js::unknown_of(text),
    }
}

pub fn create_test_name_selector(id: String) -> Selector {
    Selector {
        r#typeof: TEST_NAME_TYPE.get(),
        value: js::unknown_of(id),
    }
}

fn find_fiber_root_for_host_root(host_root: &'static Unknown) -> Rc<Fiber> {
    let maybe_fiber = get_instance_from_node(host_root);
    if let Some(maybe_fiber) = maybe_fiber {
        if js::type_of(js::get(maybe_fiber.memoized_props.get(), "data-testname")) != "string" {
            panic!(
                "Invalid host root specified. Should be either a React container or a node with a testname attribute."
            );
        }

        maybe_fiber
    } else {
        let fiber_root = find_fiber_root(host_root);

        let Some(fiber_root) = fiber_root else {
            panic!("Could not find React container within specified host subtree.");
        };

        // The Flow type for FiberRoot is a little funky.
        // createFiberRoot() cheats this by treating the root as :any and adding stateNode lazily.
        unsafe { js::cast::<&FiberRoot>(fiber_root.state_node.get()) }
            .current
            .borrow()
            .clone()
    }
}

fn match_selector(fiber: &Rc<Fiber>, selector: &Selector) -> bool {
    let tag = fiber.tag.get();
    if js::strict_equals(selector.r#typeof, COMPONENT_TYPE.get()) {
        if js::strict_equals(fiber.r#type.get(), selector.value) {
            return true;
        }
    } else if js::strict_equals(selector.r#typeof, HAS_PSEUDO_CLASS_TYPE.get()) {
        return has_matching_paths(fiber, unsafe { js::cast(Some(selector.value)) });
    } else if js::strict_equals(selector.r#typeof, ROLE_TYPE.get()) {
        if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON {
            let node = fiber.state_node.get();
            if match_accessibility_role(node, unsafe { js::cast(Some(selector.value)) }) {
                return true;
            }
        }
    } else if js::strict_equals(selector.r#typeof, TEXT_TYPE.get()) {
        if tag == HOST_COMPONENT
            || tag == HOST_TEXT
            || tag == HOST_HOISTABLE
            || tag == HOST_SINGLETON
        {
            let text_content = get_text_content(fiber);
            if let Some(text_content) = text_content
                && text_content.contains(unsafe { js::cast::<&str>(Some(selector.value)) })
            {
                return true;
            }
        }
    } else if js::strict_equals(selector.r#typeof, TEST_NAME_TYPE.get()) {
        if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON {
            let data_test_id = js::get(fiber.memoized_props.get(), "data-testname");
            if let Kind::String(data_test_id) = js::classify(data_test_id)
                && data_test_id.to_lowercase()
                    == unsafe { js::cast::<&str>(Some(selector.value)) }.to_lowercase()
            {
                return true;
            }
        }
    } else {
        panic!("Invalid selector type specified.");
    }

    false
}

fn selector_to_string(selector: &Selector) -> String {
    if js::strict_equals(selector.r#typeof, COMPONENT_TYPE.get()) {
        let display_name = get_component_name_from_type(Some(selector.value))
            .filter(|name| js::truthy(Some(name.as_str())))
            .unwrap_or_else(|| "Unknown".to_string());
        format!("<{display_name}>")
    } else if js::strict_equals(selector.r#typeof, HAS_PSEUDO_CLASS_TYPE.get()) {
        format!(":has({})", selector_to_string(selector))
    } else if js::strict_equals(selector.r#typeof, ROLE_TYPE.get()) {
        format!("[role=\"{}\"]", unsafe {
            js::cast::<&str>(Some(selector.value))
        })
    } else if js::strict_equals(selector.r#typeof, TEXT_TYPE.get()) {
        format!("\"{}\"", unsafe { js::cast::<&str>(Some(selector.value)) })
    } else if js::strict_equals(selector.r#typeof, TEST_NAME_TYPE.get()) {
        format!("[data-testname=\"{}\"]", unsafe {
            js::cast::<&str>(Some(selector.value))
        })
    } else {
        panic!("Invalid selector type specified.");
    }
}

fn find_paths(root: &Rc<Fiber>, selectors: &[Selector]) -> Vec<Rc<Fiber>> {
    let mut matching_fibers = Vec::new();

    let mut stack = vec![(Rc::clone(root), 0)];
    let mut index = 0;
    while index < stack.len() {
        let (fiber, mut selector_index) = stack[index].clone();
        index += 1;
        let tag = fiber.tag.get();
        let mut selector = selectors.get(selector_index);

        if (tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON)
            && is_hidden_subtree(&fiber)
        {
            continue;
        } else {
            while let Some(current) = selector
                && match_selector(&fiber, current)
            {
                selector_index += 1;
                selector = selectors.get(selector_index);
            }
        }

        if selector_index == selectors.len() {
            matching_fibers.push(fiber);
        } else {
            let mut child = fiber.child.borrow().clone();
            while let Some(current) = child {
                stack.push((Rc::clone(&current), selector_index));
                child = current.sibling.borrow().clone();
            }
        }
    }

    matching_fibers
}

// Same as findPaths but with eager bailout on first match
fn has_matching_paths(root: &Rc<Fiber>, selectors: &[Selector]) -> bool {
    let mut stack = vec![(Rc::clone(root), 0)];
    let mut index = 0;
    while index < stack.len() {
        let (fiber, mut selector_index) = stack[index].clone();
        index += 1;
        let tag = fiber.tag.get();
        let mut selector = selectors.get(selector_index);

        if (tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON)
            && is_hidden_subtree(&fiber)
        {
            continue;
        } else {
            while let Some(current) = selector
                && match_selector(&fiber, current)
            {
                selector_index += 1;
                selector = selectors.get(selector_index);
            }
        }

        if selector_index == selectors.len() {
            return true;
        } else {
            let mut child = fiber.child.borrow().clone();
            while let Some(current) = child {
                stack.push((Rc::clone(&current), selector_index));
                child = current.sibling.borrow().clone();
            }
        }
    }

    false
}

pub fn find_all_nodes(
    host_root: &'static Unknown,
    selectors: &[Selector],
) -> Vec<&'static Unknown> {
    if !SUPPORTS_TEST_SELECTORS {
        panic!("Test selector API is not supported by this renderer.");
    }

    let root = find_fiber_root_for_host_root(host_root);
    let matching_fibers = find_paths(&root, selectors);

    let mut instance_roots = Vec::new();

    let mut stack = matching_fibers;
    let mut index = 0;
    while index < stack.len() {
        let node = Rc::clone(&stack[index]);
        index += 1;
        let tag = node.tag.get();
        if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON {
            if is_hidden_subtree(&node) {
                continue;
            }
            instance_roots.push(unsafe { js::cast(node.state_node.get()) });
        } else {
            let mut child = node.child.borrow().clone();
            while let Some(current) = child {
                stack.push(Rc::clone(&current));
                child = current.sibling.borrow().clone();
            }
        }
    }

    instance_roots
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_find_all_nodes_failure_description(
    host_root: &'static Unknown,
    selectors: &[Selector],
) -> Option<String> {
    if !SUPPORTS_TEST_SELECTORS {
        panic!("Test selector API is not supported by this renderer.");
    }

    let root = find_fiber_root_for_host_root(host_root);

    let mut max_selector_index = 0;
    let mut matched_names = Vec::new();

    // The logic of this loop should be kept in sync with findPaths()
    let mut stack = vec![(root, 0)];
    let mut index = 0;
    while index < stack.len() {
        let (fiber, mut selector_index) = stack[index].clone();
        index += 1;
        let tag = fiber.tag.get();
        let selector = &selectors[selector_index];

        if (tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON)
            && is_hidden_subtree(&fiber)
        {
            continue;
        } else if match_selector(&fiber, selector) {
            matched_names.push(selector_to_string(selector));
            selector_index += 1;

            if selector_index > max_selector_index {
                max_selector_index = selector_index;
            }
        }

        if selector_index < selectors.len() {
            let mut child = fiber.child.borrow().clone();
            while let Some(current) = child {
                stack.push((Rc::clone(&current), selector_index));
                child = current.sibling.borrow().clone();
            }
        }
    }

    if max_selector_index < selectors.len() {
        let mut unmatched_names = Vec::new();
        for selector in &selectors[max_selector_index..] {
            unmatched_names.push(selector_to_string(selector));
        }

        return Some(format!(
            concat!(
                "findAllNodes was able to match part of the selector:\n",
                "  {}\n\n",
                "No matching component was found for:\n",
                "  {}"
            ),
            matched_names.join(" > "),
            unmatched_names.join(" > "),
        ));
    }

    None
}

pub struct BoundingRect {
    pub x: Cell<f64>,
    pub y: Cell<f64>,
    pub width: Cell<f64>,
    pub height: Cell<f64>,
}

pub fn find_bounding_rects(
    host_root: &'static Unknown,
    selectors: &[Selector],
) -> Vec<BoundingRect> {
    if !SUPPORTS_TEST_SELECTORS {
        panic!("Test selector API is not supported by this renderer.");
    }

    let instance_roots = find_all_nodes(host_root, selectors);

    let mut bounding_rects = Vec::new();
    for instance_root in instance_roots {
        bounding_rects.push(get_bounding_rect(instance_root));
    }

    for i in (1..bounding_rects.len()).rev() {
        let target_rect = &bounding_rects[i];
        let target_left = target_rect.x.get();
        let target_right = target_left + target_rect.width.get();
        let target_top = target_rect.y.get();
        let target_bottom = target_top + target_rect.height.get();

        for j in (0..i).rev() {
            if i != j {
                let other_rect = &bounding_rects[j];
                let other_left = other_rect.x.get();
                let other_right = other_left + other_rect.width.get();
                let other_top = other_rect.y.get();
                let other_bottom = other_top + other_rect.height.get();

                // Merging all rects to the minimums set would be complicated,
                // but we can handle the most common cases:
                // 1. completely overlapping rects
                // 2. adjacent rects that are the same width or height (e.g. items in a list)
                //
                // Even given the above constraints,
                // we still won't end up with the fewest possible rects without doing multiple passes,
                // but it's good enough for this purpose.

                if target_left >= other_left
                    && target_top >= other_top
                    && target_right <= other_right
                    && target_bottom <= other_bottom
                {
                    // Complete overlapping rects; remove the inner one.
                    bounding_rects.remove(i);
                    break;
                } else if target_left == other_left
                    && target_rect.width.get() == other_rect.width.get()
                    && other_bottom.partial_cmp(&target_top) != Some(Ordering::Less)
                    && other_top.partial_cmp(&target_bottom) != Some(Ordering::Greater)
                {
                    // Adjacent vertical rects; merge them.
                    if other_top > target_top {
                        other_rect
                            .height
                            .set(other_rect.height.get() + (other_top - target_top));
                        other_rect.y.set(target_top);
                    }
                    if other_bottom < target_bottom {
                        other_rect.height.set(target_bottom - other_top);
                    }

                    bounding_rects.remove(i);
                    break;
                } else if target_top == other_top
                    && target_rect.height.get() == other_rect.height.get()
                    && other_right.partial_cmp(&target_left) != Some(Ordering::Less)
                    && other_left.partial_cmp(&target_right) != Some(Ordering::Greater)
                {
                    // Adjacent horizontal rects; merge them.
                    if other_left > target_left {
                        other_rect
                            .width
                            .set(other_rect.width.get() + (other_left - target_left));
                        other_rect.x.set(target_left);
                    }
                    if other_right < target_right {
                        other_rect.width.set(target_right - other_left);
                    }

                    bounding_rects.remove(i);
                    break;
                }
            }
        }
    }

    bounding_rects
}

pub fn focus_within(host_root: &'static Unknown, selectors: &[Selector]) -> bool {
    if !SUPPORTS_TEST_SELECTORS {
        panic!("Test selector API is not supported by this renderer.");
    }

    let root = find_fiber_root_for_host_root(host_root);
    let matching_fibers = find_paths(&root, selectors);

    let mut stack = matching_fibers;
    let mut index = 0;
    while index < stack.len() {
        let fiber = Rc::clone(&stack[index]);
        index += 1;
        let tag = fiber.tag.get();
        if is_hidden_subtree(&fiber) {
            continue;
        }
        if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON {
            let node = fiber.state_node.get();
            if set_focus_if_focusable(node) {
                return true;
            }
        }
        let mut child = fiber.child.borrow().clone();
        while let Some(current) = child {
            stack.push(Rc::clone(&current));
            child = current.sibling.borrow().clone();
        }
    }

    false
}

thread_local! {
    static COMMIT_HOOKS: RefCell<Vec<Rc<dyn Fn()>>> = const { RefCell::new(Vec::new()) };
}

pub fn on_commit_root() {
    if SUPPORTS_TEST_SELECTORS {
        COMMIT_HOOKS.with_borrow(|commit_hooks| {
            commit_hooks.iter().for_each(|commit_hook| commit_hook());
        });
    }
}

pub type IntersectionObserverOptions = JsObject;

/// A target's bounding rect, and how much of it is visible.
pub struct Intersection {
    pub ratio: f64,
    pub rect: BoundingRect,
}

pub type ObserveVisibleRectsCallback = dyn Fn(Vec<Intersection>);

/// What observeVisibleRects gives: what stops it.
pub struct VisibleRectsObserver {
    pub disconnect: Box<dyn Fn()>,
}

pub fn observe_visible_rects(
    host_root: &'static Unknown,
    selectors: Vec<Selector>,
    callback: &'static ObserveVisibleRectsCallback,
    options: Option<&'static IntersectionObserverOptions>,
) -> VisibleRectsObserver {
    if !SUPPORTS_TEST_SELECTORS {
        panic!("Test selector API is not supported by this renderer.");
    }

    let instance_roots = find_all_nodes(host_root, &selectors);

    let IntersectionObserverHandle {
        disconnect,
        observe,
        unobserve,
    } = setup_intersection_observer(&instance_roots, callback, options);

    // When React mutates the host environment, we may need to change what we're listening to.
    let commit_hook: Rc<dyn Fn()> = Rc::new(move || {
        let next_instance_roots = find_all_nodes(host_root, &selectors);

        instance_roots.iter().for_each(|target| {
            if !next_instance_roots.contains(target) {
                unobserve(target);
            }
        });

        next_instance_roots.iter().for_each(|target| {
            if !instance_roots.contains(target) {
                observe(target);
            }
        });
    });

    COMMIT_HOOKS.with_borrow_mut(|commit_hooks| commit_hooks.push(Rc::clone(&commit_hook)));

    VisibleRectsObserver {
        disconnect: Box::new(move || {
            // Stop listening for React mutations:
            COMMIT_HOOKS.with_borrow_mut(|commit_hooks| {
                let index = commit_hooks
                    .iter()
                    .position(|hook| Rc::ptr_eq(hook, &commit_hook));
                if let Some(index) = index {
                    commit_hooks.remove(index);
                }
            });

            // Disconnect the host observer:
            disconnect();
        }),
    }
}
