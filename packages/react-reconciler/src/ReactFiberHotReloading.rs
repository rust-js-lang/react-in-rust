// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{JsObject, Set, SymbolFor, Unknown, WeakSet};
use shared::environment::__DEV__;
use shared::react_symbols::{REACT_FORWARD_REF_TYPE, REACT_LAZY_TYPE, REACT_MEMO_TYPE};

use crate::react_fiber_concurrent_updates::enqueue_concurrent_render_for_lane;
use crate::react_fiber_lane::{Lane, SYNC_LANE};
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_work_tags::{
    CLASS_COMPONENT, FORWARD_REF, FUNCTION_COMPONENT, MEMO_COMPONENT, SIMPLE_MEMO_COMPONENT,
};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#flushSyncWork"
)]
fn flush_sync_work() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#scheduleUpdateOnFiber"
)]
fn schedule_update_on_fiber(root: &FiberRoot, fiber: &Fiber, lane: Lane) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#flushPendingEffects"
)]
fn flush_pending_effects() -> bool {
    unreachable!()
}

// shared/ReactTypes' `ReactNodeList`, and a class component.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberReconciler.js#updateContainerSync"
)]
#[cfg_attr(rust_js, rust_js::nullable(parent_component, callback))]
fn update_container_sync(
    element: Option<&'static Unknown>,
    container: &FiberRoot,
    parent_component: Option<&'static Unknown>,
    callback: Option<&'static Unknown>,
) -> Lane {
    unreachable!()
}

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberLegacyContext.js#emptyContextObject"]
    safe static EMPTY_CONTEXT_OBJECT: &'static JsObject;
}

pub struct Family {
    pub current: Option<&'static Unknown>,
}

pub struct RefreshUpdate {
    pub stale_families: &'static Set<Family>,
    pub updated_families: &'static Set<Family>,
}

// Resolves type to a family.
pub type RefreshHandler = dyn Fn(Option<&'static Unknown>) -> Option<&'static Family>;

// Used by React Refresh runtime through DevTools Global Hook.
pub type SetRefreshHandler = fn(Option<&'static RefreshHandler>);
pub type ScheduleRefresh = fn(&FiberRoot, &RefreshUpdate);
// shared/ReactTypes' `ReactNodeList`.
pub type ScheduleRoot = fn(&FiberRoot, Option<&'static Unknown>);

// A fiber is an object, held weakly where hot reloading marks an error
// boundary failed.
unsafe impl js::WeakKey for Fiber {}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static RESOLVE_FAMILY: Cell<Option<&'static RefreshHandler>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static FAILED_BOUNDARIES: RefCell<Option<&'static WeakSet<Fiber>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable(handler))]
pub fn set_refresh_handler(handler: Option<&'static RefreshHandler>) {
    if __DEV__ {
        RESOLVE_FAMILY.set(handler);
    }
}

pub fn resolve_type_for_hot_reloading(
    r#type: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if __DEV__ {
        let Some(resolve_family) = RESOLVE_FAMILY.get() else {
            // Hot reloading is disabled.
            return r#type;
        };
        let Some(family) = resolve_family(r#type) else {
            return r#type;
        };
        // Use the latest known implementation.
        family.current
    } else {
        r#type
    }
}

// shared/ReactElementType's `ReactElement`.
pub fn is_compatible_family_for_hot_reloading(fiber: &Fiber, element: &Unknown) -> bool {
    if __DEV__ {
        let Some(resolve) = RESOLVE_FAMILY.get() else {
            // Hot reloading is disabled.
            return false;
        };

        let prev_type = fiber.element_type.get();
        let next_type = js::get(element, "type");

        // If we got here, we know types aren't === equal.
        let mut needs_compare_families = false;

        let typeof_next_type = if let Some(next_type) = js::classify(next_type).object() {
            js::get(next_type, "$$typeof")
        } else {
            None
        };

        match fiber.tag.get() {
            CLASS_COMPONENT => {
                if js::type_of(next_type) == "function" {
                    needs_compare_families = true;
                }
            }
            FUNCTION_COMPONENT => {
                if js::type_of(next_type) == "function" {
                    needs_compare_families = true;
                } else if matches!(
                    SymbolFor::from_value(typeof_next_type),
                    Some(REACT_LAZY_TYPE)
                ) {
                    // We don't know the inner type yet.
                    // We're going to assume that the lazy inner type is stable,
                    // and so it is sufficient to avoid reconciling it away.
                    // We're not going to unwrap or actually use the new lazy type.
                    needs_compare_families = true;
                }
            }
            FORWARD_REF => {
                if matches!(
                    SymbolFor::from_value(typeof_next_type),
                    Some(REACT_FORWARD_REF_TYPE | REACT_LAZY_TYPE)
                ) {
                    needs_compare_families = true;
                }
            }
            MEMO_COMPONENT | SIMPLE_MEMO_COMPONENT => {
                if matches!(
                    SymbolFor::from_value(typeof_next_type),
                    Some(REACT_MEMO_TYPE | REACT_LAZY_TYPE)
                ) {
                    needs_compare_families = true;
                }
            }
            _ => return false,
        }

        // Check if both types have a family and it's the same one.
        if needs_compare_families {
            // Note: memo() and forwardRef() we'll compare outer rather than inner type.
            // This means both of them need to be registered to preserve state.
            // If we unwrapped and compared the inner types for wrappers instead,
            // then we would risk falsely saying two separate memo(Foo)
            // calls are equivalent because they wrap the same Foo function.
            let prev_family = resolve(prev_type);
            if let Some(prev_family) = prev_family
                && resolve(next_type)
                    .is_some_and(|next_family| std::ptr::eq(prev_family, next_family))
            {
                return true;
            }
        }
        false
    } else {
        false
    }
}

pub fn mark_failed_error_boundary_for_hot_reloading(fiber: &Fiber) {
    if __DEV__ {
        if RESOLVE_FAMILY.get().is_none() {
            // Hot reloading is disabled.
            return;
        }
        if !js::is_function(js::weak_set::new::<Fiber>) {
            return;
        }
        FAILED_BOUNDARIES.with_borrow_mut(|failed_boundaries| {
            failed_boundaries
                .get_or_insert_with(js::weak_set::new)
                .add(fiber);
        });
    }
}

pub fn schedule_refresh(root: &FiberRoot, update: &RefreshUpdate) {
    if __DEV__ {
        if RESOLVE_FAMILY.get().is_none() {
            // Hot reloading is disabled.
            return;
        }
        let RefreshUpdate {
            stale_families,
            updated_families,
        } = update;
        flush_pending_effects();
        let current = root.current.borrow().clone();
        schedule_fibers_with_families_recursively(current, updated_families, stale_families);
        flush_sync_work();
    }
}

// shared/ReactTypes' `ReactNodeList`.
pub fn schedule_root(root: &FiberRoot, element: Option<&'static Unknown>) {
    if __DEV__ {
        if !js::strict_equals(root.context.get(), EMPTY_CONTEXT_OBJECT) {
            // Super edge case: root has a legacy _renderSubtree context
            // but we don't know the parentComponent so we can't pass it.
            // Just ignore. We'll delete this with _renderSubtree code path later.
            return;
        }
        update_container_sync(element, root, None, None);
        flush_sync_work();
    }
}

fn schedule_fibers_with_families_recursively(
    mut fiber: Rc<Fiber>,
    updated_families: &Set<Family>,
    stale_families: &Set<Family>,
) {
    if __DEV__ {
        loop {
            let alternate = fiber.alternate.borrow().clone();
            let child = fiber.child.borrow().clone();
            let sibling = fiber.sibling.borrow().clone();
            let tag = fiber.tag.get();
            let r#type = fiber.r#type.get();
            let element_type = fiber.element_type.get();

            let mut candidate_type = None;
            // Wrapper fibers (memo, forwardRef) resolve their family through the
            // inner implementation, but an edit that changes the kind of the type
            // (e.g. memo to a plain function) is only recorded on the family of the
            // outer type. Check the outer type too so such edits trigger a remount.
            let mut outer_candidate_type = None;
            match tag {
                FUNCTION_COMPONENT | CLASS_COMPONENT => {
                    candidate_type = r#type;
                }
                SIMPLE_MEMO_COMPONENT => {
                    candidate_type = r#type;
                    outer_candidate_type = element_type;
                }
                MEMO_COMPONENT => {
                    // Edits to the inner implementation are handled by the inner fiber.
                    outer_candidate_type = element_type;
                }
                FORWARD_REF => {
                    candidate_type = js::get(r#type, "render");
                    outer_candidate_type = element_type;
                }
                _ => {}
            }

            let Some(resolve) = RESOLVE_FAMILY.get() else {
                panic!("Expected resolveFamily to be set during hot reload.");
            };

            let mut needs_render = false;
            let mut needs_remount = false;
            if candidate_type.is_some() {
                let family = resolve(candidate_type);
                if let Some(family) = family {
                    if stale_families.has(family) {
                        needs_remount = true;
                    } else if updated_families.has(family) {
                        if tag == CLASS_COMPONENT {
                            needs_remount = true;
                        } else {
                            needs_render = true;
                        }
                    }
                }
            }
            if !needs_remount && outer_candidate_type.is_some() {
                let outer_family = resolve(outer_candidate_type);
                if outer_family.is_some_and(|outer_family| stale_families.has(outer_family)) {
                    needs_remount = true;
                } else if js::type_of(outer_candidate_type) == "object"
                    && matches!(
                        SymbolFor::from_value(js::get(outer_candidate_type, "$$typeof")),
                        Some(REACT_LAZY_TYPE)
                    )
                {
                    let payload = js::get(outer_candidate_type, "_payload");
                    if js::strict_equals(
                        js::get(payload, "_status"),
                        1, /* Resolved; see ReactLazy */
                    ) {
                        let middle_family =
                            resolve(js::get(js::get(payload, "_result"), "default"));
                        if middle_family
                            .is_some_and(|middle_family| stale_families.has(middle_family))
                        {
                            needs_remount = true;
                        }
                    }
                }
            }
            if let Some(failed_boundaries) =
                FAILED_BOUNDARIES.with_borrow(|failed_boundaries| *failed_boundaries)
                && (failed_boundaries.has(&fiber)
                    || alternate
                        .as_ref()
                        .is_some_and(|alternate| failed_boundaries.has(alternate)))
            {
                needs_remount = true;
            }

            if needs_remount {
                fiber.debug_needs_remount.set(Some(true));
            }
            if needs_remount || needs_render {
                let root = enqueue_concurrent_render_for_lane(&fiber, SYNC_LANE);
                if let Some(root) = root {
                    schedule_update_on_fiber(&root, &fiber, SYNC_LANE);
                }
            }
            if let Some(child) = child
                && !needs_remount
            {
                schedule_fibers_with_families_recursively(child, updated_families, stale_families);
            }

            let Some(sibling) = sibling else {
                break;
            };
            fiber = sibling;
        }
    }
}
