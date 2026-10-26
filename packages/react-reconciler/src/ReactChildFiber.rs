// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{Dict, Iterator, JsError, Kind, Map, Set, SymbolFor, Unknown};
use react_isomorphic::react_context::ReactContext;
use react_isomorphic::react_jsx_element::{ReactElement, ReactKey};
use react_isomorphic::react_lazy::{LazyComponent, Thenable};
use shared::environment::__DEV__;
use shared::is_array::is_array;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, ENABLE_ASYNC_ITERABLE_CHILDREN, ENABLE_OPTIMISTIC_KEY,
};
use shared::react_symbols::{
    ASYNC_ITERATOR, REACT_CONTEXT_TYPE, REACT_ELEMENT_TYPE, REACT_FRAGMENT_TYPE, REACT_LAZY_TYPE,
    REACT_LEGACY_ELEMENT_TYPE, REACT_OPTIMISTIC_KEY, REACT_PORTAL_TYPE, get_iterator_fn,
};
use webapi::{ConsoleTask, console};

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_fiber::{
    PortalStateNode, create_fiber_from_element, create_fiber_from_fragment,
    create_fiber_from_portal, create_fiber_from_text, create_fiber_from_throw,
    create_work_in_progress, reset_work_in_progress,
};
use crate::react_fiber_flags::{CHILD_DELETION, FORKED, PLACEMENT, PLACEMENT_DEV};
use crate::react_fiber_hot_reloading::is_compatible_family_for_hot_reloading;
use crate::react_fiber_hydration_context::get_is_hydrating;
use crate::react_fiber_lane::Lanes;
use crate::react_fiber_new_context::read_context_during_reconciliation;
use crate::react_fiber_thenable::{
    SUSPENSE_ACTION_EXCEPTION, SUSPENSE_EXCEPTION, ThenableState, create_thenable_state,
    resolve_lazy, track_used_thenable,
};
use crate::react_fiber_tree_context::push_tree_fork;
use crate::react_internal_types::Fiber;
use crate::react_portal::ReactPortal;
use crate::react_type_of_mode::{CONCURRENT_MODE, NO_MODE};
use crate::react_work_tags::{FRAGMENT, FUNCTION_COMPONENT, HOST_PORTAL, HOST_ROOT, HOST_TEXT};

thread_local! {
    // This tracks the thenables that are unwrapped during reconcilation.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static THENABLE_STATE: RefCell<Option<Rc<ThenableState>>> = const { RefCell::new(None) };
    static THENABLE_INDEX_COUNTER: Cell<usize> = const { Cell::new(0) };

    // Server Components Meta Data
    // shared/ReactTypes' `ReactDebugInfo`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_DEBUG_INFO: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
}

// shared/ReactTypes' `ReactDebugInfo`s.
#[cfg_attr(rust_js, rust_js::nullable)]
fn push_debug_info(debug_info: Option<&'static Unknown>) -> Option<&'static Unknown> {
    if !__DEV__ {
        return None;
    }
    let previous_debug_info = CURRENT_DEBUG_INFO.get();
    if debug_info.is_none() {
        // Leave inplace
    } else if previous_debug_info.is_none() {
        CURRENT_DEBUG_INFO.set(debug_info);
    } else {
        // If we have two debugInfo, we need to create a new one. This makes the array no longer
        // live so we'll miss any future updates if we received more so ideally we should always
        // do this after both have fully resolved/unsuspended.
        let previous_debug_info: &[&'static Unknown] = unsafe { js::cast(previous_debug_info) };
        let debug_info: &[&'static Unknown] = unsafe { js::cast(debug_info) };
        CURRENT_DEBUG_INFO.set(Some(js::unknown_of(
            [previous_debug_info, debug_info].concat(),
        )));
    }
    previous_debug_info
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_current_debug_task() -> Option<&'static ConsoleTask> {
    // Get the debug task of the parent Server Component if there is one.
    if __DEV__ {
        let debug_info = CURRENT_DEBUG_INFO.get();
        if let Some(debug_info) = debug_info {
            let debug_info: &[&'static Unknown] = unsafe { js::cast(Some(debug_info)) };
            for &component_info in debug_info.iter().rev() {
                if js::get(component_info, "name").is_some() {
                    let debug_task: Option<&'static ConsoleTask> =
                        unsafe { js::cast(js::get(component_info, "debugTask")) };
                    if let Some(debug_task) = debug_task {
                        return Some(debug_task);
                    }
                }
            }
        }
    }
    None
}

/// What warns that a child in a list has no key.
type WarnForMissingKey = fn(&Rc<Fiber>, &Rc<Fiber>, Option<&'static Unknown>);

thread_local! {
    static DID_WARN_ABOUT_MAPS: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_ABOUT_GENERATORS: Cell<bool> = const { Cell::new(false) };
    static OWNER_HAS_KEY_USE_WARNING: &'static Dict<bool> = js::dict::new();
    static OWNER_HAS_FUNCTION_TYPE_WARNING: &'static Dict<bool> = js::dict::new();
    static OWNER_HAS_SYMBOL_TYPE_WARNING: &'static Dict<bool> = js::dict::new();
    static WARN_FOR_MISSING_KEY: Cell<WarnForMissingKey> =
        Cell::new(|_return_fiber, _work_in_progress, _child| {});
}

js::on_load! {
    if __DEV__ {
        DID_WARN_ABOUT_MAPS.set(false);
        DID_WARN_ABOUT_GENERATORS.set(false);

        /*
         * Warn if there's no key explicitly set on dynamic arrays of children or
         * object keys are not valid. This allows us to keep track of children between
         * updates.
         */
        WARN_FOR_MISSING_KEY.set(|return_fiber, work_in_progress, child| {
            if child.is_none() || js::type_of(child) != "object" {
                return;
            }
            let store = js::get(child, "_store");
            if !js::truthy(store)
                || ((js::truthy(js::get(store, "validated")) || js::get(child, "key").is_some())
                    && !js::strict_equals(js::get(store, "validated"), 2))
            {
                return;
            }

            if js::type_of(store) != "object" {
                panic!(concat!(
                    "React Component in warnForMissingKey should have a _store. ",
                    "This error is likely caused by a bug in React. Please file an issue."
                ));
            }

            // $FlowFixMe[cannot-write] unable to narrow type from mixed to writable object
            js::set(store, "validated", 1);

            let component_name = get_component_name_from_fiber(return_fiber);

            let component_key = component_name
                .as_deref()
                .filter(|name| js::truthy(Some(*name)))
                .unwrap_or("null");
            if OWNER_HAS_KEY_USE_WARNING.with(|owner_has_key_use_warning| {
                js::dict::get(owner_has_key_use_warning, component_key).is_some_and(|&warned| warned)
            }) {
                return;
            }
            OWNER_HAS_KEY_USE_WARNING.with(|owner_has_key_use_warning| {
                js::dict::set(owner_has_key_use_warning, component_key, true);
            });

            let child_owner = js::get(child, "_owner");
            let parent_owner = return_fiber.debug_owner.get();

            let mut current_component_error_info = String::new();
            if js::truthy(parent_owner) && js::type_of(js::get(parent_owner, "tag")) == "number" {
                let name = get_component_name_from_fiber(unsafe { js::cast(parent_owner) });
                if let Some(name) = name.filter(|name| js::truthy(Some(name.as_str()))) {
                    current_component_error_info =
                        "\n\nCheck the render method of `".to_string() + &name + "`.";
                }
            }
            if current_component_error_info.is_empty()
                && let Some(component_name) =
                    component_name.filter(|name| js::truthy(Some(name.as_str())))
            {
                current_component_error_info =
                    format!("\n\nCheck the top-level render call using <{component_name}>.");
            }

            // Usually the current owner is the offender, but if it accepts children as a
            // property, it may be the creator of the child that's responsible for
            // assigning it a key.
            let mut child_owner_appendix = String::new();
            if child_owner.is_some() && !js::strict_equals(parent_owner, child_owner) {
                let mut owner_name = None;
                if js::type_of(js::get(child_owner, "tag")) == "number" {
                    owner_name = get_component_name_from_fiber(unsafe { js::cast(child_owner) });
                } else if js::type_of(js::get(child_owner, "name")) == "string" {
                    owner_name = Some(unsafe { js::cast(js::get(child_owner, "name")) });
                }
                if let Some(owner_name) =
                    owner_name.filter(|name: &String| js::truthy(Some(name.as_str())))
                {
                    // Give the component that originally created this child.
                    child_owner_appendix = format!(" It was passed a child from {owner_name}.");
                }
            }

            run_with_fiber_in_dev(
                Some(Rc::clone(work_in_progress)),
                |_: (), _: (), _: (), _: (), _: ()| {
                    console::error((
                        concat!(
                            "Each child in a list should have a unique \"key\" prop.",
                            "%s%s See https://react.dev/link/warning-keys for more information."
                        ),
                        &current_component_error_info,
                        &child_owner_appendix,
                    ));
                },
                (),
                (),
                (),
                (),
                (),
            );
        });
    }
}

// Given a fragment, validate that it can only be provided with fragment props
// We do this here instead of BeginWork because the Fragment fiber doesn't have
// the whole props object, only the children and is shared with arrays.
fn validate_fragment_props(
    element: &ReactElement,
    fiber: Option<&Rc<Fiber>>,
    return_fiber: &Rc<Fiber>,
) {
    if __DEV__ {
        let keys = js::object::keys(element.props);
        for key in &keys {
            if key != "children" && key != "key" && key != "ref" {
                let fiber = match fiber {
                    Some(fiber) => Rc::clone(fiber),
                    None => {
                        // For unkeyed root fragments without refs, there's no Fiber.
                        // We create a fake one just for error stack handling.
                        let fiber = create_fiber_from_element(element, return_fiber.mode.get(), 0);
                        if __DEV__ {
                            fiber.debug_info.set(CURRENT_DEBUG_INFO.get());
                        }
                        fiber.r#return.replace(Some(Rc::clone(return_fiber)));
                        fiber
                    }
                };
                run_with_fiber_in_dev(
                    Some(fiber),
                    |errored_key: &str, _: (), _: (), _: (), _: ()| {
                        console::error((
                            concat!(
                                "Invalid prop `%s` supplied to `React.Fragment`. ",
                                "React.Fragment can only have `key`, `ref`, and `children` props."
                            ),
                            errored_key,
                        ));
                    },
                    key.as_str(),
                    (),
                    (),
                    (),
                    (),
                );
                break;
            }
        }
    }
}

fn unwrap_thenable(thenable: &'static Thenable) -> Option<&'static Unknown> {
    let index = THENABLE_INDEX_COUNTER.get();
    THENABLE_INDEX_COUNTER.set(index + 1);
    let thenable_state = THENABLE_STATE.with_borrow_mut(|thenable_state| {
        Rc::clone(thenable_state.get_or_insert_with(|| Rc::new(create_thenable_state())))
    });
    track_used_thenable(&thenable_state, thenable, index, None)
}

fn coerce_ref(work_in_progress: &Fiber, element: &ReactElement) {
    // TODO: This is a temporary, intermediate step. Now that enableRefAsProp is on,
    // we should resolve the `ref` prop during the begin phase of the component
    // it's attached to (HostComponent, ClassComponent, etc).
    let ref_prop = js::get(element.props, "ref");
    // TODO: With enableRefAsProp now rolled out, we shouldn't use the `ref` field. We
    // should always read the ref from the prop.
    work_in_progress.r#ref.set(unsafe { js::cast(ref_prop) });
}

fn throw_on_invalid_object_type_impl(_return_fiber: &Fiber, new_child: &Unknown) {
    if SymbolFor::from_value(js::get(new_child, "$$typeof")) == Some(REACT_LEGACY_ELEMENT_TYPE) {
        panic!(concat!(
            "A React Element from an older version of React was rendered. ",
            "This is not supported. It can happen if:\n",
            "- Multiple copies of the \"react\" package is used.\n",
            "- A library pre-bundled an old copy of \"react\" or \"react/jsx-runtime\".\n",
            "- A compiler tries to \"inline\" JSX instead of using the runtime."
        ));
    }

    // $FlowFixMe[method-unbinding]
    let child_string = js::object::to_string(Some(new_child)).unwrap();

    panic!(
        "Objects are not valid as a React child (found: {}). If you meant to render a collection of children, use an array instead.",
        if child_string == "[object Object]" {
            "object with keys {".to_string() + &js::object::keys(new_child).join(", ") + "}"
        } else {
            child_string
        }
    );
}

fn throw_on_invalid_object_type(return_fiber: &Fiber, new_child: &Unknown) {
    let debug_task = get_current_debug_task();
    if __DEV__ && let Some(debug_task) = debug_task {
        debug_task.run(|| throw_on_invalid_object_type_impl(return_fiber, new_child));
    } else {
        throw_on_invalid_object_type_impl(return_fiber, new_child);
    }
}

fn warn_on_function_type_impl(return_fiber: &Fiber, invalid_child: &Unknown) {
    if __DEV__ {
        let parent_name = get_component_name_from_fiber(return_fiber)
            .filter(|name| js::truthy(Some(name.as_str())))
            .unwrap_or_else(|| "Component".to_string());

        if OWNER_HAS_FUNCTION_TYPE_WARNING.with(|owner_has_function_type_warning| {
            js::dict::get(owner_has_function_type_warning, &parent_name)
                .is_some_and(|&warned| warned)
        }) {
            return;
        }
        OWNER_HAS_FUNCTION_TYPE_WARNING.with(|owner_has_function_type_warning| {
            js::dict::set(owner_has_function_type_warning, &parent_name, true);
        });

        let name = unsafe {
            js::cast::<Option<String>>(
                js::get(invalid_child, "displayName")
                    .filter(|name| js::truthy(Some(*name)))
                    .or_else(|| js::get(invalid_child, "name"))
                    .filter(|name| js::truthy(Some(*name))),
            )
        }
        .unwrap_or_else(|| "Component".to_string());

        if return_fiber.tag.get() == HOST_ROOT {
            console::error((
                concat!(
                    "Functions are not valid as a React child. This may happen if ",
                    "you return %s instead of <%s /> from render. ",
                    "Or maybe you meant to call this function rather than return it.\n",
                    "  root.render(%s)"
                ),
                &name,
                &name,
                &name,
            ));
        } else {
            console::error((
                concat!(
                    "Functions are not valid as a React child. This may happen if ",
                    "you return %s instead of <%s /> from render. ",
                    "Or maybe you meant to call this function rather than return it.\n",
                    "  <%s>{%s}</%s>"
                ),
                &name,
                &name,
                &parent_name,
                &name,
                &parent_name,
            ));
        }
    }
}

fn warn_on_function_type(return_fiber: &Fiber, invalid_child: &Unknown) {
    let debug_task = get_current_debug_task();
    if __DEV__ && let Some(debug_task) = debug_task {
        debug_task.run(|| warn_on_function_type_impl(return_fiber, invalid_child));
    } else {
        warn_on_function_type_impl(return_fiber, invalid_child);
    }
}

fn warn_on_symbol_type_impl(return_fiber: &Fiber, invalid_child: &Unknown) {
    if __DEV__ {
        let parent_name = get_component_name_from_fiber(return_fiber)
            .filter(|name| js::truthy(Some(name.as_str())))
            .unwrap_or_else(|| "Component".to_string());

        if OWNER_HAS_SYMBOL_TYPE_WARNING.with(|owner_has_symbol_type_warning| {
            js::dict::get(owner_has_symbol_type_warning, &parent_name).is_some_and(|&warned| warned)
        }) {
            return;
        }
        OWNER_HAS_SYMBOL_TYPE_WARNING.with(|owner_has_symbol_type_warning| {
            js::dict::set(owner_has_symbol_type_warning, &parent_name, true);
        });

        // eslint-disable-next-line react-internal/safe-string-coercion
        let name = js::string(Some(invalid_child));

        if return_fiber.tag.get() == HOST_ROOT {
            console::error((
                concat!(
                    "Symbols are not valid as a React child.\n",
                    "  root.render(%s)"
                ),
                &name,
            ));
        } else {
            console::error((
                concat!("Symbols are not valid as a React child.\n", "  <%s>%s</%s>"),
                &parent_name,
                &name,
                &parent_name,
            ));
        }
    }
}

fn warn_on_symbol_type(return_fiber: &Fiber, invalid_child: &Unknown) {
    let debug_task = get_current_debug_task();
    if __DEV__ && let Some(debug_task) = debug_task {
        debug_task.run(|| warn_on_symbol_type_impl(return_fiber, invalid_child));
    } else {
        warn_on_symbol_type_impl(return_fiber, invalid_child);
    }
}

/// What reconciles a fiber's children: given the fiber, its current first
/// child, its new children and the lanes, the first child of the new ones.
pub type ChildReconciler =
    dyn Fn(&Rc<Fiber>, Option<Rc<Fiber>>, Option<&'static Unknown>, Lanes) -> Option<Rc<Fiber>>;

struct RestoreDebugInfo(Option<&'static Unknown>);

impl Drop for RestoreDebugInfo {
    fn drop(&mut self) {
        CURRENT_DEBUG_INFO.set(self.0);
    }
}

// This wrapper function exists because I expect to clone the code in each path
// to be able to optimize each path individually by branching early. This needs
// a compiler or we can do it manually. Helpers that don't need this branching
// live outside of this function.
fn create_child_reconciler(should_track_side_effects: bool) -> Box<ChildReconciler> {
    fn delete_child(
        should_track_side_effects: bool,
        return_fiber: &Fiber,
        child_to_delete: &Rc<Fiber>,
    ) {
        if !should_track_side_effects {
            // Noop.
            return;
        }
        let mut deletions = return_fiber.deletions.borrow_mut();
        match &mut *deletions {
            None => {
                *deletions = Some(vec![Rc::clone(child_to_delete)]);
                return_fiber
                    .flags
                    .set(return_fiber.flags.get() | CHILD_DELETION);
            }
            Some(deletions) => deletions.push(Rc::clone(child_to_delete)),
        }
    }

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn delete_remaining_children(
        should_track_side_effects: bool,
        return_fiber: &Fiber,
        current_first_child: Option<Rc<Fiber>>,
    ) -> Option<Rc<Fiber>> {
        if !should_track_side_effects {
            // Noop.
            return None;
        }

        // TODO: For the shouldClone case, this could be micro-optimized a bit by
        // assuming that after the first child we've already added everything.
        let mut child_to_delete = current_first_child;
        while let Some(child) = child_to_delete {
            delete_child(should_track_side_effects, return_fiber, &child);
            child_to_delete = child.sibling.borrow().clone();
        }
        None
    }

    fn map_remaining_children(current_first_child: Rc<Fiber>) -> &'static Map<Unknown, Rc<Fiber>> {
        // Add the remaining children to a temporary map so that we can find them by
        // keys quickly. Implicit (null) keys get added to this set with their index
        // instead.
        let existing_children = js::map::new(None);

        let mut existing_child = Some(current_first_child);
        while let Some(child) = existing_child {
            let key = child.key.borrow().clone();
            match key {
                None => existing_children.set(js::unknown(&child.index.get()), Rc::clone(&child)),
                Some(ReactKey::Optimistic(key))
                    if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                        && key == REACT_OPTIMISTIC_KEY =>
                {
                    // For optimistic keys, we store the negative index (minus one) to differentiate
                    // them from the regular indices. We'll look this up regardless of what the new
                    // key is, if there's no other match.
                    existing_children.set(
                        js::unknown(&(-(child.index.get() as f64) - 1.0)),
                        Rc::clone(&child),
                    );
                }
                Some(key) => existing_children.set(js::unknown(&key), Rc::clone(&child)),
            }
            existing_child = child.sibling.borrow().clone();
        }
        existing_children
    }

    fn use_fiber(fiber: &Rc<Fiber>, pending_props: Option<&'static Unknown>) -> Rc<Fiber> {
        // We currently set sibling to null and index to 0 here because it is easy
        // to forget to do before returning it. E.g. for the single child case.
        let clone = create_work_in_progress(fiber, pending_props);
        clone.index.set(0);
        clone.sibling.replace(None);
        clone
    }

    fn place_child(
        should_track_side_effects: bool,
        new_fiber: &Fiber,
        last_placed_index: usize,
        new_index: usize,
    ) -> usize {
        new_fiber.index.set(new_index);
        if !should_track_side_effects {
            // During hydration, the useId algorithm needs to know which fibers are
            // part of a list of children (arrays, iterators).
            new_fiber.flags.set(new_fiber.flags.get() | FORKED);
            return last_placed_index;
        }
        let current = new_fiber.alternate.borrow().clone();
        if let Some(current) = current {
            let old_index = current.index.get();
            if old_index < last_placed_index {
                // This is a move. The fiber already existed, so this is not a new
                // mount; don't set PlacementDEV, which would cause StrictMode to
                // re-run the effects in its subtree as if it had remounted.
                new_fiber.flags.set(new_fiber.flags.get() | PLACEMENT);
                last_placed_index
            } else {
                // This item can stay in place.
                old_index
            }
        } else {
            // This is an insertion.
            new_fiber
                .flags
                .set(new_fiber.flags.get() | PLACEMENT | PLACEMENT_DEV);
            last_placed_index
        }
    }

    fn place_single_child(should_track_side_effects: bool, new_fiber: Rc<Fiber>) -> Rc<Fiber> {
        // This is simpler for the single child case. We only need to do a
        // placement for inserting new children.
        if should_track_side_effects && new_fiber.alternate.borrow().is_none() {
            new_fiber
                .flags
                .set(new_fiber.flags.get() | PLACEMENT | PLACEMENT_DEV);
        }
        new_fiber
    }

    fn update_text_node(
        return_fiber: &Rc<Fiber>,
        current: Option<&Rc<Fiber>>,
        text_content: String,
        lanes: Lanes,
    ) -> Rc<Fiber> {
        let Some(current) = current.filter(|current| current.tag.get() == HOST_TEXT) else {
            // Insert
            let created = create_fiber_from_text(text_content, return_fiber.mode.get(), lanes);
            created.r#return.replace(Some(Rc::clone(return_fiber)));
            if __DEV__ {
                // We treat the parent as the owner for stack purposes.
                created
                    .debug_owner
                    .set(Some(js::unknown_of(Rc::clone(return_fiber))));
                created.debug_task.set(return_fiber.debug_task.get());
                created.debug_info.set(CURRENT_DEBUG_INFO.get());
            }
            return created;
        };
        // Update
        let existing = use_fiber(current, Some(js::unknown_of(text_content)));
        existing.r#return.replace(Some(Rc::clone(return_fiber)));
        if __DEV__ {
            existing.debug_info.set(CURRENT_DEBUG_INFO.get());
        }
        existing
    }

    fn update_element(
        return_fiber: &Rc<Fiber>,
        current: Option<&Rc<Fiber>>,
        element: &'static ReactElement,
        lanes: Lanes,
    ) -> Rc<Fiber> {
        let element_type = element.r#type;
        if SymbolFor::from_value(element_type) == Some(REACT_FRAGMENT_TYPE) {
            let updated = update_fragment(
                return_fiber,
                current,
                js::get(element.props, "children"),
                lanes,
                element.key.clone(),
            );
            coerce_ref(&updated, element);
            validate_fragment_props(element, Some(&updated), return_fiber);
            return updated;
        }
        if let Some(current) = current
            && (js::strict_equals(current.element_type.get(), element_type)
                // Keep this check inline so it only runs on the false path:
                || (__DEV__ && is_compatible_family_for_hot_reloading(current, js::unknown(element)))
                // Lazy types should reconcile their resolved type.
                // We need to do this after the Hot Reloading check above,
                // because hot reloading has different semantics than prod because
                // it doesn't resuspend. So we can't let the call below suspend.
                || (js::type_of(element_type) == "object"
                    && element_type.is_some()
                    && SymbolFor::from_value(js::get(element_type, "$$typeof"))
                        == Some(REACT_LAZY_TYPE)
                    && js::strict_equals(
                        resolve_lazy(unsafe { js::cast(element_type) }),
                        current.r#type.get(),
                    )))
        {
            // Move based on index
            let existing = use_fiber(current, Some(element.props));
            coerce_ref(&existing, element);
            existing.r#return.replace(Some(Rc::clone(return_fiber)));
            if __DEV__ {
                existing.debug_owner.set(element.owner);
                existing.debug_info.set(CURRENT_DEBUG_INFO.get());
            }
            return existing;
        }
        // Insert
        let created = create_fiber_from_element(element, return_fiber.mode.get(), lanes);
        coerce_ref(&created, element);
        created.r#return.replace(Some(Rc::clone(return_fiber)));
        if __DEV__ {
            created.debug_info.set(CURRENT_DEBUG_INFO.get());
        }
        created
    }

    fn update_portal(
        return_fiber: &Rc<Fiber>,
        current: Option<&Rc<Fiber>>,
        portal: &'static ReactPortal,
        lanes: Lanes,
    ) -> Rc<Fiber> {
        let Some(current) = current.filter(|current| {
            current.tag.get() == HOST_PORTAL
                && js::strict_equals(
                    unsafe { js::cast::<&PortalStateNode>(current.state_node.get()) }
                        .container_info,
                    portal.container_info,
                )
                && js::strict_equals(
                    unsafe { js::cast::<&PortalStateNode>(current.state_node.get()) }
                        .implementation,
                    portal.implementation,
                )
        }) else {
            // Insert
            let created = create_fiber_from_portal(portal, return_fiber.mode.get(), lanes);
            created.r#return.replace(Some(Rc::clone(return_fiber)));
            if __DEV__ {
                created.debug_info.set(CURRENT_DEBUG_INFO.get());
            }
            return created;
        };
        // Update
        let existing = use_fiber(
            current,
            portal
                .children
                .filter(|children| js::truthy(Some(*children)))
                .or_else(|| Some(js::unknown_of(Vec::<&Unknown>::new()))),
        );
        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
            // If the old key was optimistic we need to now save the real one.
            existing.key.replace(portal.key.clone());
        }
        existing.r#return.replace(Some(Rc::clone(return_fiber)));
        if __DEV__ {
            existing.debug_info.set(CURRENT_DEBUG_INFO.get());
        }
        existing
    }

    fn update_fragment(
        return_fiber: &Rc<Fiber>,
        current: Option<&Rc<Fiber>>,
        fragment: Option<&'static Unknown>,
        lanes: Lanes,
        key: Option<ReactKey>,
    ) -> Rc<Fiber> {
        let Some(current) = current.filter(|current| current.tag.get() == FRAGMENT) else {
            // Insert
            let created = create_fiber_from_fragment(fragment, return_fiber.mode.get(), lanes, key);
            created.r#return.replace(Some(Rc::clone(return_fiber)));
            if __DEV__ {
                // We treat the parent as the owner for stack purposes.
                created
                    .debug_owner
                    .set(Some(js::unknown_of(Rc::clone(return_fiber))));
                created.debug_task.set(return_fiber.debug_task.get());
                created.debug_info.set(CURRENT_DEBUG_INFO.get());
            }
            return created;
        };
        // Update
        let existing = use_fiber(current, fragment);
        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
            // If the old key was optimistic we need to now save the real one.
            existing.key.replace(key);
        }
        existing.r#return.replace(Some(Rc::clone(return_fiber)));
        if __DEV__ {
            existing.debug_info.set(CURRENT_DEBUG_INFO.get());
        }
        existing
    }

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn create_child(
        return_fiber: &Rc<Fiber>,
        new_child: Option<&'static Unknown>,
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        if (js::type_of(new_child) == "string" && !js::strict_equals(new_child, ""))
            || js::type_of(new_child) == "number"
            || js::type_of(new_child) == "bigint"
        {
            // Text nodes don't have keys. If the previous node is implicitly keyed
            // we can continue to replace it without aborting even if it is not a text
            // node.
            let created = create_fiber_from_text(
                // $FlowFixMe[unsafe-addition] Flow doesn't want us to use `+` operator with string and bigint
                js::concat_text(new_child).unwrap(),
                return_fiber.mode.get(),
                lanes,
            );
            created.r#return.replace(Some(Rc::clone(return_fiber)));
            if __DEV__ {
                // We treat the parent as the owner for stack purposes.
                created
                    .debug_owner
                    .set(Some(js::unknown_of(Rc::clone(return_fiber))));
                created.debug_task.set(return_fiber.debug_task.get());
                created.debug_info.set(CURRENT_DEBUG_INFO.get());
            }
            return Some(created);
        }

        if js::type_of(new_child) == "object"
            && let Some(child) = new_child
        {
            match SymbolFor::from_value(js::get(child, "$$typeof")) {
                Some(REACT_ELEMENT_TYPE) => {
                    let element: &ReactElement = unsafe { js::cast(new_child) };
                    let created =
                        create_fiber_from_element(element, return_fiber.mode.get(), lanes);
                    coerce_ref(&created, element);
                    created.r#return.replace(Some(Rc::clone(return_fiber)));
                    if __DEV__ {
                        let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                        created.debug_info.set(CURRENT_DEBUG_INFO.get());
                        CURRENT_DEBUG_INFO.set(prev_debug_info);
                    }
                    return Some(created);
                }
                Some(REACT_PORTAL_TYPE) => {
                    let created = create_fiber_from_portal(
                        unsafe { js::cast(new_child) },
                        return_fiber.mode.get(),
                        lanes,
                    );
                    created.r#return.replace(Some(Rc::clone(return_fiber)));
                    if __DEV__ {
                        created.debug_info.set(CURRENT_DEBUG_INFO.get());
                    }
                    return Some(created);
                }
                Some(REACT_LAZY_TYPE) => {
                    let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                    let resolved_child =
                        resolve_lazy(unsafe { js::cast::<&LazyComponent>(new_child) });
                    let created = create_child(return_fiber, resolved_child, lanes);
                    CURRENT_DEBUG_INFO.set(prev_debug_info);
                    return created;
                }
                _ => {}
            }

            if is_array(new_child)
                || get_iterator_fn(new_child).is_some()
                || (ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
                    && js::type_of(
                        ASYNC_ITERATOR.with(|&async_iterator| js::get(child, async_iterator)),
                    ) == "function")
            {
                let created = create_fiber_from_fragment(
                    new_child,
                    return_fiber.mode.get(),
                    lanes,
                    js::null(),
                );
                created.r#return.replace(Some(Rc::clone(return_fiber)));
                if __DEV__ {
                    // We treat the parent as the owner for stack purposes.
                    created
                        .debug_owner
                        .set(Some(js::unknown_of(Rc::clone(return_fiber))));
                    created.debug_task.set(return_fiber.debug_task.get());
                    // Make sure to not push again when handling the Fragment child.
                    let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                    created.debug_info.set(CURRENT_DEBUG_INFO.get());
                    CURRENT_DEBUG_INFO.set(prev_debug_info);
                }
                return Some(created);
            }

            // Usable node types
            //
            // Unwrap the inner value and recursively call this function again.
            if js::type_of(js::get(child, "then")) == "function" {
                let thenable: &'static Thenable = unsafe { js::cast(new_child) };
                let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                let created = create_child(return_fiber, unwrap_thenable(thenable), lanes);
                CURRENT_DEBUG_INFO.set(prev_debug_info);
                return created;
            }

            if SymbolFor::from_value(js::get(child, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
                let context: Rc<ReactContext<Option<&'static Unknown>>> =
                    unsafe { js::cast(new_child) };
                return create_child(
                    return_fiber,
                    read_context_during_reconciliation(return_fiber, &context, lanes),
                    lanes,
                );
            }

            throw_on_invalid_object_type(return_fiber, child);
        }

        if __DEV__ {
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "function"
            {
                warn_on_function_type(return_fiber, new_child);
            }
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "symbol"
            {
                warn_on_symbol_type(return_fiber, new_child);
            }
        }

        None
    }

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn update_slot(
        return_fiber: &Rc<Fiber>,
        old_fiber: Option<&Rc<Fiber>>,
        new_child: Option<&'static Unknown>,
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        // Update the fiber if the keys match, otherwise return null.
        let key = old_fiber.and_then(|old_fiber| old_fiber.key.borrow().clone());

        if (js::type_of(new_child) == "string" && !js::strict_equals(new_child, ""))
            || js::type_of(new_child) == "number"
            || js::type_of(new_child) == "bigint"
        {
            // Text nodes don't have keys. If the previous node is implicitly keyed
            // we can continue to replace it without aborting even if it is not a text
            // node.
            if key.is_some() {
                return None;
            }
            return Some(update_text_node(
                return_fiber,
                old_fiber,
                // $FlowFixMe[unsafe-addition] Flow doesn't want us to use `+` operator with string and bigint
                js::concat_text(new_child).unwrap(),
                lanes,
            ));
        }

        if js::type_of(new_child) == "object"
            && let Some(child) = new_child
        {
            match SymbolFor::from_value(js::get(child, "$$typeof")) {
                Some(REACT_ELEMENT_TYPE) => {
                    let element: &'static ReactElement = unsafe { js::cast(new_child) };
                    // If the old child was an optimisticKey, then we'd normally consider that a match,
                    // but instead, we'll bail to return null from the slot which will bail to slow path.
                    // That's to ensure that if the new key has a match elsewhere in the list, then that
                    // takes precedence over assuming the identity of an optimistic slot.
                    if element.key == key {
                        let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                        let updated = update_element(return_fiber, old_fiber, element, lanes);
                        CURRENT_DEBUG_INFO.set(prev_debug_info);
                        return Some(updated);
                    } else {
                        return None;
                    }
                }
                Some(REACT_PORTAL_TYPE) => {
                    let portal: &'static ReactPortal = unsafe { js::cast(new_child) };
                    // If the old child was an optimisticKey, then we'd normally consider that a match,
                    // but instead, we'll bail to return null from the slot which will bail to slow path.
                    // That's to ensure that if the new key has a match elsewhere in the list, then that
                    // takes precedence over assuming the identity of an optimistic slot.
                    if portal.key == key {
                        return Some(update_portal(return_fiber, old_fiber, portal, lanes));
                    } else {
                        return None;
                    }
                }
                Some(REACT_LAZY_TYPE) => {
                    let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                    let resolved_child =
                        resolve_lazy(unsafe { js::cast::<&LazyComponent>(new_child) });
                    let updated = update_slot(return_fiber, old_fiber, resolved_child, lanes);
                    CURRENT_DEBUG_INFO.set(prev_debug_info);
                    return updated;
                }
                _ => {}
            }

            if is_array(new_child)
                || get_iterator_fn(new_child).is_some()
                || (ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
                    && js::type_of(
                        ASYNC_ITERATOR.with(|&async_iterator| js::get(child, async_iterator)),
                    ) == "function")
            {
                if key.is_some() {
                    return None;
                }

                let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                let updated =
                    update_fragment(return_fiber, old_fiber, new_child, lanes, js::null());
                CURRENT_DEBUG_INFO.set(prev_debug_info);
                return Some(updated);
            }

            // Usable node types
            //
            // Unwrap the inner value and recursively call this function again.
            if js::type_of(js::get(child, "then")) == "function" {
                let thenable: &'static Thenable = unsafe { js::cast(new_child) };
                let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                let updated =
                    update_slot(return_fiber, old_fiber, unwrap_thenable(thenable), lanes);
                CURRENT_DEBUG_INFO.set(prev_debug_info);
                return updated;
            }

            if SymbolFor::from_value(js::get(child, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
                let context: Rc<ReactContext<Option<&'static Unknown>>> =
                    unsafe { js::cast(new_child) };
                return update_slot(
                    return_fiber,
                    old_fiber,
                    read_context_during_reconciliation(return_fiber, &context, lanes),
                    lanes,
                );
            }

            throw_on_invalid_object_type(return_fiber, child);
        }

        if __DEV__ {
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "function"
            {
                warn_on_function_type(return_fiber, new_child);
            }
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "symbol"
            {
                warn_on_symbol_type(return_fiber, new_child);
            }
        }

        None
    }

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn update_from_map(
        existing_children: &Map<Unknown, Rc<Fiber>>,
        return_fiber: &Rc<Fiber>,
        new_idx: usize,
        new_child: Option<&'static Unknown>,
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        if (js::type_of(new_child) == "string" && !js::strict_equals(new_child, ""))
            || js::type_of(new_child) == "number"
            || js::type_of(new_child) == "bigint"
        {
            // Text nodes don't have keys, so we neither have to check the old nor
            // new node for the key. If both are text nodes, they match.
            let matched_fiber = existing_children.get(js::unknown(&new_idx));
            return Some(update_text_node(
                return_fiber,
                matched_fiber.as_ref(),
                // $FlowFixMe[unsafe-addition] Flow doesn't want us to use `+` operator with string and bigint
                js::concat_text(new_child).unwrap(),
                lanes,
            ));
        }

        if js::type_of(new_child) == "object"
            && let Some(child) = new_child
        {
            match SymbolFor::from_value(js::get(child, "$$typeof")) {
                Some(REACT_ELEMENT_TYPE) => {
                    let element: &'static ReactElement = unsafe { js::cast(new_child) };
                    let matched_fiber = match &element.key {
                        None => existing_children.get(js::unknown(&new_idx)),
                        Some(key) => existing_children.get(js::unknown(key)),
                    }
                    .or_else(|| {
                        // If the existing child was an optimistic key, we may still match on the index.
                        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
                            existing_children.get(js::unknown(&(-(new_idx as f64) - 1.0)))
                        } else {
                            None
                        }
                    });
                    let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                    let updated =
                        update_element(return_fiber, matched_fiber.as_ref(), element, lanes);
                    CURRENT_DEBUG_INFO.set(prev_debug_info);
                    return Some(updated);
                }
                Some(REACT_PORTAL_TYPE) => {
                    let portal: &'static ReactPortal = unsafe { js::cast(new_child) };
                    let matched_fiber = match &portal.key {
                        None => existing_children.get(js::unknown(&new_idx)),
                        Some(key) => existing_children.get(js::unknown(key)),
                    }
                    .or_else(|| {
                        // If the existing child was an optimistic key, we may still match on the index.
                        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
                            existing_children.get(js::unknown(&(-(new_idx as f64) - 1.0)))
                        } else {
                            None
                        }
                    });
                    return Some(update_portal(
                        return_fiber,
                        matched_fiber.as_ref(),
                        portal,
                        lanes,
                    ));
                }
                Some(REACT_LAZY_TYPE) => {
                    let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                    let resolved_child =
                        resolve_lazy(unsafe { js::cast::<&LazyComponent>(new_child) });
                    let updated = update_from_map(
                        existing_children,
                        return_fiber,
                        new_idx,
                        resolved_child,
                        lanes,
                    );
                    CURRENT_DEBUG_INFO.set(prev_debug_info);
                    return updated;
                }
                _ => {}
            }

            if is_array(new_child)
                || get_iterator_fn(new_child).is_some()
                || (ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
                    && js::type_of(
                        ASYNC_ITERATOR.with(|&async_iterator| js::get(child, async_iterator)),
                    ) == "function")
            {
                let matched_fiber = existing_children.get(js::unknown(&new_idx));
                let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                let updated = update_fragment(
                    return_fiber,
                    matched_fiber.as_ref(),
                    new_child,
                    lanes,
                    js::null(),
                );
                CURRENT_DEBUG_INFO.set(prev_debug_info);
                return Some(updated);
            }

            // Usable node types
            //
            // Unwrap the inner value and recursively call this function again.
            if js::type_of(js::get(child, "then")) == "function" {
                let thenable: &'static Thenable = unsafe { js::cast(new_child) };
                let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                let updated = update_from_map(
                    existing_children,
                    return_fiber,
                    new_idx,
                    unwrap_thenable(thenable),
                    lanes,
                );
                CURRENT_DEBUG_INFO.set(prev_debug_info);
                return updated;
            }

            if SymbolFor::from_value(js::get(child, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
                let context: Rc<ReactContext<Option<&'static Unknown>>> =
                    unsafe { js::cast(new_child) };
                return update_from_map(
                    existing_children,
                    return_fiber,
                    new_idx,
                    read_context_during_reconciliation(return_fiber, &context, lanes),
                    lanes,
                );
            }

            throw_on_invalid_object_type(return_fiber, child);
        }

        if __DEV__ {
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "function"
            {
                warn_on_function_type(return_fiber, new_child);
            }
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "symbol"
            {
                warn_on_symbol_type(return_fiber, new_child);
            }
        }

        None
    }

    /**
     * Warns if there is a duplicate or missing key
     */
    #[cfg_attr(rust_js, rust_js::nullable)]
    fn warn_on_invalid_key(
        return_fiber: &Rc<Fiber>,
        work_in_progress: &Rc<Fiber>,
        child: Option<&'static Unknown>,
        known_keys: Option<&'static Set<str>>,
    ) -> Option<&'static Set<str>> {
        let mut known_keys = known_keys;
        if __DEV__ {
            if js::type_of(child) != "object" || child.is_none() {
                return known_keys;
            }
            match SymbolFor::from_value(js::get(child, "$$typeof")) {
                Some(REACT_ELEMENT_TYPE | REACT_PORTAL_TYPE) => {
                    WARN_FOR_MISSING_KEY.get()(return_fiber, work_in_progress, child);
                    let key = js::get(child, "key");
                    if let Kind::String(key) = js::classify(key) {
                        match known_keys {
                            None => {
                                let new_known_keys = js::set::new(None);
                                new_known_keys.add(key);
                                known_keys = Some(new_known_keys);
                            }
                            Some(known_keys) if !known_keys.has(key) => known_keys.add(key),
                            Some(_) => {
                                run_with_fiber_in_dev(
                                    Some(Rc::clone(work_in_progress)),
                                    |_: (), _: (), _: (), _: (), _: ()| {
                                        console::error((
                                            concat!(
                                                "Encountered two children with the same key, `%s`. ",
                                                "Keys should be unique so that components maintain their identity ",
                                                "across updates. Non-unique keys may cause children to be ",
                                                "duplicated and/or omitted — the behavior is unsupported and ",
                                                "could change in a future version."
                                            ),
                                            key,
                                        ));
                                    },
                                    (),
                                    (),
                                    (),
                                    (),
                                    (),
                                );
                            }
                        }
                    }
                }
                Some(REACT_LAZY_TYPE) => {
                    let resolved_child = resolve_lazy(unsafe { js::cast::<&LazyComponent>(child) });
                    warn_on_invalid_key(return_fiber, work_in_progress, resolved_child, known_keys);
                }
                _ => {}
            }
        }
        known_keys
    }

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn reconcile_children_array(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        new_children: &[Option<&'static Unknown>],
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        // This algorithm can't optimize by searching from both ends since we
        // don't have backpointers on fibers. I'm trying to see how far we can get
        // with that model. If it ends up not being worth the tradeoffs, we can
        // add it later.

        // Even with a two ended optimization, we'd want to optimize for the case
        // where there are few changes and brute force the comparison instead of
        // going for the Map. It'd like to explore hitting that path first in
        // forward-only mode and only go for the Map once we notice that we need
        // lots of look ahead. This doesn't handle reversal as well as two ended
        // search but that's unusual. Besides, for the two ended optimization to
        // work on Iterables, we'd need to copy the whole set.

        // In this first iteration, we'll just live with hitting the bad case
        // (adding everything to a Map) in for every insert/move.

        // If you change this code, also update reconcileChildrenIterator() which
        // uses the same algorithm.

        let mut known_keys = None;

        let mut resulting_first_child = None;
        let mut previous_new_fiber: Option<Rc<Fiber>> = None;

        let mut old_fiber = current_first_child;
        let mut last_placed_index = 0;
        let mut new_idx = 0;
        let mut next_old_fiber;
        while let Some(fiber) = &old_fiber
            && new_idx < new_children.len()
        {
            if fiber.index.get() > new_idx {
                next_old_fiber = old_fiber;
                old_fiber = None;
            } else {
                next_old_fiber = fiber.sibling.borrow().clone();
            }
            let new_fiber = update_slot(
                return_fiber,
                old_fiber.as_ref(),
                new_children[new_idx],
                lanes,
            );
            let Some(new_fiber) = new_fiber else {
                // TODO: This breaks on empty slots like null children. That's
                // unfortunate because it triggers the slow path all the time. We need
                // a better way to communicate whether this was a miss or null,
                // boolean, undefined, etc.
                if old_fiber.is_none() {
                    old_fiber = next_old_fiber;
                }
                break;
            };

            if __DEV__ {
                known_keys = warn_on_invalid_key(
                    return_fiber,
                    &new_fiber,
                    new_children[new_idx],
                    known_keys,
                );
            }

            if should_track_side_effects
                && let Some(old_fiber) = &old_fiber
                && new_fiber.alternate.borrow().is_none()
            {
                // We matched the slot, but we didn't reuse the existing fiber, so we
                // need to delete the existing child.
                delete_child(should_track_side_effects, return_fiber, old_fiber);
            }
            last_placed_index = place_child(
                should_track_side_effects,
                &new_fiber,
                last_placed_index,
                new_idx,
            );
            match &previous_new_fiber {
                None => {
                    // TODO: Move out of the loop. This only happens for the first run.
                    resulting_first_child = Some(Rc::clone(&new_fiber));
                }
                Some(previous_new_fiber) => {
                    // TODO: Defer siblings if we're not at the right index for this slot.
                    // I.e. if we had null values before, then we want to defer this
                    // for each null value. However, we also don't want to call updateSlot
                    // with the previous one.
                    previous_new_fiber
                        .sibling
                        .replace(Some(Rc::clone(&new_fiber)));
                }
            }
            previous_new_fiber = Some(new_fiber);
            old_fiber = next_old_fiber;
            new_idx += 1;
        }

        if new_idx == new_children.len() {
            // We've reached the end of the new children. We can delete the rest.
            delete_remaining_children(should_track_side_effects, return_fiber, old_fiber);
            if get_is_hydrating() {
                let number_of_forks = new_idx;
                push_tree_fork(return_fiber, number_of_forks as i32);
            }
            return resulting_first_child;
        }

        let Some(old_fiber) = old_fiber else {
            // If we don't have any more existing children we can choose a fast path
            // since the rest will all be insertions.
            while new_idx < new_children.len() {
                let new_fiber = create_child(return_fiber, new_children[new_idx], lanes);
                if let Some(new_fiber) = new_fiber {
                    if __DEV__ {
                        known_keys = warn_on_invalid_key(
                            return_fiber,
                            &new_fiber,
                            new_children[new_idx],
                            known_keys,
                        );
                    }
                    last_placed_index = place_child(
                        should_track_side_effects,
                        &new_fiber,
                        last_placed_index,
                        new_idx,
                    );
                    match &previous_new_fiber {
                        None => {
                            // TODO: Move out of the loop. This only happens for the first run.
                            resulting_first_child = Some(Rc::clone(&new_fiber));
                        }
                        Some(previous_new_fiber) => {
                            previous_new_fiber
                                .sibling
                                .replace(Some(Rc::clone(&new_fiber)));
                        }
                    }
                    previous_new_fiber = Some(new_fiber);
                }
                new_idx += 1;
            }
            if get_is_hydrating() {
                let number_of_forks = new_idx;
                push_tree_fork(return_fiber, number_of_forks as i32);
            }
            return resulting_first_child;
        };

        // Add all children to a key map for quick lookups.
        let existing_children = map_remaining_children(old_fiber);

        // Keep scanning and use the map to restore deleted items as moves.
        while new_idx < new_children.len() {
            let new_fiber = update_from_map(
                existing_children,
                return_fiber,
                new_idx,
                new_children[new_idx],
                lanes,
            );
            if let Some(new_fiber) = new_fiber {
                if __DEV__ {
                    known_keys = warn_on_invalid_key(
                        return_fiber,
                        &new_fiber,
                        new_children[new_idx],
                        known_keys,
                    );
                }
                if should_track_side_effects {
                    let current_fiber = new_fiber.alternate.borrow().clone();
                    if let Some(current_fiber) = current_fiber {
                        // The new fiber is a work in progress, but if there exists a
                        // current, that means that we reused the fiber. We need to delete
                        // it from the child list so that we don't add it to the deletion
                        // list.
                        let key = current_fiber.key.borrow().clone();
                        match key {
                            Some(ReactKey::Optimistic(key))
                                if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                                    && key == REACT_OPTIMISTIC_KEY =>
                            {
                                existing_children.delete(js::unknown(&(-(new_idx as f64) - 1.0)));
                            }
                            None => {
                                existing_children.delete(js::unknown(&new_idx));
                            }
                            Some(key) => {
                                existing_children.delete(js::unknown(&key));
                            }
                        }
                    }
                }
                last_placed_index = place_child(
                    should_track_side_effects,
                    &new_fiber,
                    last_placed_index,
                    new_idx,
                );
                match &previous_new_fiber {
                    None => resulting_first_child = Some(Rc::clone(&new_fiber)),
                    Some(previous_new_fiber) => {
                        previous_new_fiber
                            .sibling
                            .replace(Some(Rc::clone(&new_fiber)));
                    }
                }
                previous_new_fiber = Some(new_fiber);
            }
            new_idx += 1;
        }

        if should_track_side_effects {
            // Any existing children that weren't consumed above were deleted. We need
            // to add them to the deletion list.
            existing_children
                .for_each(|child, _| delete_child(should_track_side_effects, return_fiber, &child));
        }

        if get_is_hydrating() {
            let number_of_forks = new_idx;
            push_tree_fork(return_fiber, number_of_forks as i32);
        }
        resulting_first_child
    }

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn reconcile_children_iteratable(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        new_children_iterable: &'static Unknown,
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        // This is the same implementation as reconcileChildrenArray(),
        // but using the iterator instead.

        let iterator_fn = get_iterator_fn(Some(new_children_iterable));

        if js::type_of(iterator_fn) != "function" {
            panic!(concat!(
                "An object is not an iterable. This error is likely caused by a bug in ",
                "React. Please file an issue."
            ));
        }

        let new_children = js::function::call(
            unsafe { js::cast::<&dyn Fn() -> Option<&'static Unknown>>(iterator_fn) },
            Some(&new_children_iterable),
            (),
        );

        if __DEV__ {
            if js::strict_equals(new_children, new_children_iterable) {
                // We don't support rendering Generators as props because it's a mutation.
                // See https://github.com/react/react/issues/12995
                // We do support generators if they were created by a GeneratorFunction component
                // as its direct child since we can recreate those by rerendering the component
                // as needed.
                let is_generator_component = return_fiber.tag.get() == FUNCTION_COMPONENT
                    // $FlowFixMe[method-unbinding]
                    && js::object::to_string(return_fiber.r#type.get()).unwrap()
                        == "[object GeneratorFunction]"
                    // $FlowFixMe[method-unbinding]
                    && js::object::to_string(new_children).unwrap() == "[object Generator]";
                if !is_generator_component {
                    if !DID_WARN_ABOUT_GENERATORS.get() {
                        console::error((concat!(
                            "Using Iterators as children is unsupported and will likely yield ",
                            "unexpected results because enumerating a generator mutates it. ",
                            "You may convert it to an array with `Array.from()` or the ",
                            "`[...spread]` operator before rendering. You can also use an ",
                            "Iterable that can iterate multiple times over the same items."
                        ),));
                    }
                    DID_WARN_ABOUT_GENERATORS.set(true);
                }
            } else if js::strict_equals(js::get(new_children_iterable, "entries"), iterator_fn) {
                // Warn about using Maps as children
                if !DID_WARN_ABOUT_MAPS.get() {
                    console::error((concat!(
                        "Using Maps as children is not supported. ",
                        "Use an array of keyed ReactElements instead."
                    ),));
                    DID_WARN_ABOUT_MAPS.set(true);
                }
            }
        }

        reconcile_children_iterator(
            should_track_side_effects,
            return_fiber,
            current_first_child,
            unsafe { js::cast(new_children) },
            lanes,
        )
    }

    /// The iterator an async iterable's children are reconciled with: each
    /// step its async iterator's, unwrapped.
    struct UnwrappedIterator {
        next: Box<dyn Fn() -> Option<&'static Unknown>>,
    }

    unsafe impl js::Defined for UnwrappedIterator {}

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn reconcile_children_async_iteratable(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        new_children_iterable: &'static Unknown,
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        let new_children = js::function::call(
            unsafe {
                js::cast::<&dyn Fn() -> Option<&'static Unknown>>(
                    ASYNC_ITERATOR
                        .with(|&async_iterator| js::get(new_children_iterable, async_iterator)),
                )
            },
            Some(&new_children_iterable),
            (),
        );

        if __DEV__ && js::strict_equals(new_children, new_children_iterable) {
            // We don't support rendering AsyncGenerators as props because it's a mutation.
            // We do support generators if they were created by a AsyncGeneratorFunction component
            // as its direct child since we can recreate those by rerendering the component
            // as needed.
            let is_generator_component = return_fiber.tag.get() == FUNCTION_COMPONENT
                // $FlowFixMe[method-unbinding]
                && js::object::to_string(return_fiber.r#type.get()).unwrap()
                    == "[object AsyncGeneratorFunction]"
                // $FlowFixMe[method-unbinding]
                && js::object::to_string(new_children).unwrap() == "[object AsyncGenerator]";
            if !is_generator_component {
                if !DID_WARN_ABOUT_GENERATORS.get() {
                    console::error((concat!(
                        "Using AsyncIterators as children is unsupported and will likely yield ",
                        "unexpected results because enumerating a generator mutates it. ",
                        "You can use an AsyncIterable that can iterate multiple times over ",
                        "the same items."
                    ),));
                }
                DID_WARN_ABOUT_GENERATORS.set(true);
            }
        }

        let Some(new_children) = new_children else {
            panic!("An iterable object provided no iterator.");
        };

        // To save bytes, we reuse the logic by creating a synchronous Iterable and
        // reusing that code path.
        let iterator = UnwrappedIterator {
            next: Box::new(move || {
                unwrap_thenable(unsafe {
                    js::cast(js::function::call(
                        js::cast::<&dyn Fn() -> Option<&'static Unknown>>(js::get(
                            new_children,
                            "next",
                        )),
                        Some(&new_children),
                        (),
                    ))
                })
            }),
        };

        reconcile_children_iterator(
            should_track_side_effects,
            return_fiber,
            current_first_child,
            unsafe { js::cast(Some(js::unknown_of(iterator))) },
            lanes,
        )
    }

    #[cfg_attr(rust_js, rust_js::nullable)]
    fn reconcile_children_iterator(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        new_children: Option<&'static Iterator<Option<&'static Unknown>>>,
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        let Some(new_children) = new_children else {
            panic!("An iterable object provided no iterator.");
        };

        let mut resulting_first_child = None;
        let mut previous_new_fiber: Option<Rc<Fiber>> = None;

        let mut old_fiber = current_first_child;
        let mut last_placed_index = 0;
        let mut new_idx = 0;
        let mut next_old_fiber;

        let mut known_keys = None;

        let mut step = new_children.next();
        while let Some(fiber) = &old_fiber
            && step.done != Some(true)
        {
            if fiber.index.get() > new_idx {
                next_old_fiber = old_fiber;
                old_fiber = None;
            } else {
                next_old_fiber = fiber.sibling.borrow().clone();
            }
            let new_fiber = update_slot(return_fiber, old_fiber.as_ref(), step.value, lanes);
            let Some(new_fiber) = new_fiber else {
                // TODO: This breaks on empty slots like null children. That's
                // unfortunate because it triggers the slow path all the time. We need
                // a better way to communicate whether this was a miss or null,
                // boolean, undefined, etc.
                if old_fiber.is_none() {
                    old_fiber = next_old_fiber;
                }
                break;
            };

            if __DEV__ {
                known_keys = warn_on_invalid_key(return_fiber, &new_fiber, step.value, known_keys);
            }

            if should_track_side_effects
                && let Some(old_fiber) = &old_fiber
                && new_fiber.alternate.borrow().is_none()
            {
                // We matched the slot, but we didn't reuse the existing fiber, so we
                // need to delete the existing child.
                delete_child(should_track_side_effects, return_fiber, old_fiber);
            }
            last_placed_index = place_child(
                should_track_side_effects,
                &new_fiber,
                last_placed_index,
                new_idx,
            );
            match &previous_new_fiber {
                None => {
                    // TODO: Move out of the loop. This only happens for the first run.
                    resulting_first_child = Some(Rc::clone(&new_fiber));
                }
                Some(previous_new_fiber) => {
                    // TODO: Defer siblings if we're not at the right index for this slot.
                    // I.e. if we had null values before, then we want to defer this
                    // for each null value. However, we also don't want to call updateSlot
                    // with the previous one.
                    previous_new_fiber
                        .sibling
                        .replace(Some(Rc::clone(&new_fiber)));
                }
            }
            previous_new_fiber = Some(new_fiber);
            old_fiber = next_old_fiber;
            new_idx += 1;
            step = new_children.next();
        }

        if step.done == Some(true) {
            // We've reached the end of the new children. We can delete the rest.
            delete_remaining_children(should_track_side_effects, return_fiber, old_fiber);
            if get_is_hydrating() {
                let number_of_forks = new_idx;
                push_tree_fork(return_fiber, number_of_forks as i32);
            }
            return resulting_first_child;
        }

        let Some(old_fiber) = old_fiber else {
            // If we don't have any more existing children we can choose a fast path
            // since the rest will all be insertions.
            while step.done != Some(true) {
                let new_fiber = create_child(return_fiber, step.value, lanes);
                if let Some(new_fiber) = new_fiber {
                    if __DEV__ {
                        known_keys =
                            warn_on_invalid_key(return_fiber, &new_fiber, step.value, known_keys);
                    }
                    last_placed_index = place_child(
                        should_track_side_effects,
                        &new_fiber,
                        last_placed_index,
                        new_idx,
                    );
                    match &previous_new_fiber {
                        None => {
                            // TODO: Move out of the loop. This only happens for the first run.
                            resulting_first_child = Some(Rc::clone(&new_fiber));
                        }
                        Some(previous_new_fiber) => {
                            previous_new_fiber
                                .sibling
                                .replace(Some(Rc::clone(&new_fiber)));
                        }
                    }
                    previous_new_fiber = Some(new_fiber);
                }
                new_idx += 1;
                step = new_children.next();
            }
            if get_is_hydrating() {
                let number_of_forks = new_idx;
                push_tree_fork(return_fiber, number_of_forks as i32);
            }
            return resulting_first_child;
        };

        // Add all children to a key map for quick lookups.
        let existing_children = map_remaining_children(old_fiber);

        // Keep scanning and use the map to restore deleted items as moves.
        while step.done != Some(true) {
            let new_fiber =
                update_from_map(existing_children, return_fiber, new_idx, step.value, lanes);
            if let Some(new_fiber) = new_fiber {
                if __DEV__ {
                    known_keys =
                        warn_on_invalid_key(return_fiber, &new_fiber, step.value, known_keys);
                }
                if should_track_side_effects {
                    let current_fiber = new_fiber.alternate.borrow().clone();
                    if let Some(current_fiber) = current_fiber {
                        // The new fiber is a work in progress, but if there exists a
                        // current, that means that we reused the fiber. We need to delete
                        // it from the child list so that we don't add it to the deletion
                        // list.
                        let key = current_fiber.key.borrow().clone();
                        match key {
                            Some(ReactKey::Optimistic(key))
                                if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                                    && key == REACT_OPTIMISTIC_KEY =>
                            {
                                existing_children.delete(js::unknown(&(-(new_idx as f64) - 1.0)));
                            }
                            None => {
                                existing_children.delete(js::unknown(&new_idx));
                            }
                            Some(key) => {
                                existing_children.delete(js::unknown(&key));
                            }
                        }
                    }
                }
                last_placed_index = place_child(
                    should_track_side_effects,
                    &new_fiber,
                    last_placed_index,
                    new_idx,
                );
                match &previous_new_fiber {
                    None => resulting_first_child = Some(Rc::clone(&new_fiber)),
                    Some(previous_new_fiber) => {
                        previous_new_fiber
                            .sibling
                            .replace(Some(Rc::clone(&new_fiber)));
                    }
                }
                previous_new_fiber = Some(new_fiber);
            }
            new_idx += 1;
            step = new_children.next();
        }

        if should_track_side_effects {
            // Any existing children that weren't consumed above were deleted. We need
            // to add them to the deletion list.
            existing_children
                .for_each(|child, _| delete_child(should_track_side_effects, return_fiber, &child));
        }

        if get_is_hydrating() {
            let number_of_forks = new_idx;
            push_tree_fork(return_fiber, number_of_forks as i32);
        }
        resulting_first_child
    }

    fn reconcile_single_text_node(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        text_content: String,
        lanes: Lanes,
    ) -> Rc<Fiber> {
        // There's no need to check for keys on text nodes since we don't have a
        // way to define them.
        if let Some(current_first_child) = &current_first_child
            && current_first_child.tag.get() == HOST_TEXT
        {
            // We already have an existing node so let's just update it and delete
            // the rest.
            let sibling = current_first_child.sibling.borrow().clone();
            delete_remaining_children(should_track_side_effects, return_fiber, sibling);
            let existing = use_fiber(current_first_child, Some(js::unknown_of(text_content)));
            existing.r#return.replace(Some(Rc::clone(return_fiber)));
            return existing;
        }
        // The existing first child is not a text node so we need to create one
        // and delete the existing ones.
        delete_remaining_children(should_track_side_effects, return_fiber, current_first_child);
        let created = create_fiber_from_text(text_content, return_fiber.mode.get(), lanes);
        created.r#return.replace(Some(Rc::clone(return_fiber)));
        if __DEV__ {
            // We treat the parent as the owner for stack purposes.
            created
                .debug_owner
                .set(Some(js::unknown_of(Rc::clone(return_fiber))));
            created.debug_task.set(return_fiber.debug_task.get());
            created.debug_info.set(CURRENT_DEBUG_INFO.get());
        }
        created
    }

    fn reconcile_single_element(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        element: &'static ReactElement,
        lanes: Lanes,
    ) -> Rc<Fiber> {
        let key = &element.key;
        let mut child = current_first_child;
        while let Some(current) = child {
            // TODO: If key === null and child.key === null, then this only applies to
            // the first item in the list.
            let child_key = current.key.borrow().clone();
            if child_key == *key
                || (ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                    && matches!(child_key, Some(ReactKey::Optimistic(child_key)) if child_key == REACT_OPTIMISTIC_KEY))
            {
                let element_type = element.r#type;
                if SymbolFor::from_value(element_type) == Some(REACT_FRAGMENT_TYPE) {
                    if current.tag.get() == FRAGMENT {
                        let sibling = current.sibling.borrow().clone();
                        delete_remaining_children(should_track_side_effects, return_fiber, sibling);
                        let existing = use_fiber(&current, js::get(element.props, "children"));
                        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
                            // If the old key was optimistic we need to now save the real one.
                            existing.key.replace(key.clone());
                        }
                        coerce_ref(&existing, element);
                        existing.r#return.replace(Some(Rc::clone(return_fiber)));
                        if __DEV__ {
                            existing.debug_owner.set(element.owner);
                            existing.debug_info.set(CURRENT_DEBUG_INFO.get());
                        }
                        validate_fragment_props(element, Some(&existing), return_fiber);
                        return existing;
                    }
                } else if js::strict_equals(current.element_type.get(), element_type)
                    // Keep this check inline so it only runs on the false path:
                    || (__DEV__ && is_compatible_family_for_hot_reloading(&current, js::unknown(element)))
                    // Lazy types should reconcile their resolved type.
                    // We need to do this after the Hot Reloading check above,
                    // because hot reloading has different semantics than prod because
                    // it doesn't resuspend. So we can't let the call below suspend.
                    || (js::type_of(element_type) == "object"
                        && element_type.is_some()
                        && SymbolFor::from_value(js::get(element_type, "$$typeof"))
                            == Some(REACT_LAZY_TYPE)
                        && js::strict_equals(
                            resolve_lazy(unsafe { js::cast(element_type) }),
                            current.r#type.get(),
                        ))
                {
                    let sibling = current.sibling.borrow().clone();
                    delete_remaining_children(should_track_side_effects, return_fiber, sibling);
                    let existing = use_fiber(&current, Some(element.props));
                    if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
                        // If the old key was optimistic we need to now save the real one.
                        existing.key.replace(key.clone());
                    }
                    coerce_ref(&existing, element);
                    existing.r#return.replace(Some(Rc::clone(return_fiber)));
                    if __DEV__ {
                        existing.debug_owner.set(element.owner);
                        existing.debug_info.set(CURRENT_DEBUG_INFO.get());
                    }
                    return existing;
                }
                // Didn't match.
                delete_remaining_children(should_track_side_effects, return_fiber, Some(current));
                break;
            } else {
                delete_child(should_track_side_effects, return_fiber, &current);
            }
            child = current.sibling.borrow().clone();
        }

        if SymbolFor::from_value(element.r#type) == Some(REACT_FRAGMENT_TYPE) {
            let created = create_fiber_from_fragment(
                js::get(element.props, "children"),
                return_fiber.mode.get(),
                lanes,
                element.key.clone(),
            );
            coerce_ref(&created, element);
            created.r#return.replace(Some(Rc::clone(return_fiber)));
            if __DEV__ {
                // We treat the parent as the owner for stack purposes.
                created
                    .debug_owner
                    .set(Some(js::unknown_of(Rc::clone(return_fiber))));
                created.debug_task.set(return_fiber.debug_task.get());
                created.debug_info.set(CURRENT_DEBUG_INFO.get());
            }
            validate_fragment_props(element, Some(&created), return_fiber);
            created
        } else {
            let created = create_fiber_from_element(element, return_fiber.mode.get(), lanes);
            coerce_ref(&created, element);
            created.r#return.replace(Some(Rc::clone(return_fiber)));
            if __DEV__ {
                created.debug_info.set(CURRENT_DEBUG_INFO.get());
            }
            created
        }
    }

    fn reconcile_single_portal(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        portal: &'static ReactPortal,
        lanes: Lanes,
    ) -> Rc<Fiber> {
        let key = &portal.key;
        let mut child = current_first_child;
        while let Some(current) = child {
            // TODO: If key === null and child.key === null, then this only applies to
            // the first item in the list.
            let child_key = current.key.borrow().clone();
            if child_key == *key
                || (ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                    && matches!(child_key, Some(ReactKey::Optimistic(child_key)) if child_key == REACT_OPTIMISTIC_KEY))
            {
                if current.tag.get() == HOST_PORTAL
                    && js::strict_equals(
                        unsafe { js::cast::<&PortalStateNode>(current.state_node.get()) }
                            .container_info,
                        portal.container_info,
                    )
                    && js::strict_equals(
                        unsafe { js::cast::<&PortalStateNode>(current.state_node.get()) }
                            .implementation,
                        portal.implementation,
                    )
                {
                    let sibling = current.sibling.borrow().clone();
                    delete_remaining_children(should_track_side_effects, return_fiber, sibling);
                    let existing = use_fiber(
                        &current,
                        portal
                            .children
                            .filter(|children| js::truthy(Some(*children)))
                            .or_else(|| Some(js::unknown_of(Vec::<&Unknown>::new()))),
                    );
                    if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled) {
                        // If the old key was optimistic we need to now save the real one.
                        existing.key.replace(key.clone());
                    }
                    existing.r#return.replace(Some(Rc::clone(return_fiber)));
                    return existing;
                } else {
                    delete_remaining_children(
                        should_track_side_effects,
                        return_fiber,
                        Some(current),
                    );
                    break;
                }
            } else {
                delete_child(should_track_side_effects, return_fiber, &current);
            }
            child = current.sibling.borrow().clone();
        }

        let created = create_fiber_from_portal(portal, return_fiber.mode.get(), lanes);
        created.r#return.replace(Some(Rc::clone(return_fiber)));
        created
    }

    // This API will tag the children with the side-effect of the reconciliation
    // itself. They will be added to the side-effect list as we pass through the
    // children and the parent.
    #[cfg_attr(rust_js, rust_js::nullable)]
    fn reconcile_child_fibers_impl(
        should_track_side_effects: bool,
        return_fiber: &Rc<Fiber>,
        current_first_child: Option<Rc<Fiber>>,
        new_child: Option<&'static Unknown>,
        lanes: Lanes,
    ) -> Option<Rc<Fiber>> {
        let mut new_child = new_child;
        // This function is only recursive for Usables/Lazy and not nested arrays.
        // That's so that using a Lazy wrapper is unobservable to the Fragment
        // convention.
        // If the top level item is an array, we treat it as a set of children,
        // not as a fragment. Nested arrays on the other hand will be treated as
        // fragment nodes. Recursion happens at the normal flow.

        // Handle top level unkeyed fragments without refs
        // as if they were arrays. This leads to an ambiguity between <>{[...]}</> and <>...</>.
        // We treat the ambiguous cases above the same.
        // We don't use recursion here because a fragment inside a fragment
        // is no longer considered "top level" for these purposes.
        let is_unkeyed_unrefed_top_level_fragment = js::type_of(new_child) == "object"
            && new_child.is_some()
            && SymbolFor::from_value(js::get(new_child, "type")) == Some(REACT_FRAGMENT_TYPE)
            && matches!(js::classify(js::get(new_child, "key")), Kind::Null)
            && matches!(
                js::classify(js::get(js::get(new_child, "props"), "ref")),
                Kind::Undefined
            );

        if is_unkeyed_unrefed_top_level_fragment {
            validate_fragment_props(unsafe { js::cast(new_child) }, None, return_fiber);
            new_child = js::get(js::get(new_child, "props"), "children");
        }

        // Handle object types
        if js::type_of(new_child) == "object"
            && let Some(child) = new_child
        {
            match SymbolFor::from_value(js::get(child, "$$typeof")) {
                Some(REACT_ELEMENT_TYPE) => {
                    let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                    let first_child = place_single_child(
                        should_track_side_effects,
                        reconcile_single_element(
                            should_track_side_effects,
                            return_fiber,
                            current_first_child,
                            unsafe { js::cast(new_child) },
                            lanes,
                        ),
                    );
                    CURRENT_DEBUG_INFO.set(prev_debug_info);
                    return Some(first_child);
                }
                Some(REACT_PORTAL_TYPE) => {
                    return Some(place_single_child(
                        should_track_side_effects,
                        reconcile_single_portal(
                            should_track_side_effects,
                            return_fiber,
                            current_first_child,
                            unsafe { js::cast(new_child) },
                            lanes,
                        ),
                    ));
                }
                Some(REACT_LAZY_TYPE) => {
                    let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                    let result = resolve_lazy(unsafe { js::cast::<&LazyComponent>(new_child) });
                    let first_child = reconcile_child_fibers_impl(
                        should_track_side_effects,
                        return_fiber,
                        current_first_child,
                        result,
                        lanes,
                    );
                    CURRENT_DEBUG_INFO.set(prev_debug_info);
                    return first_child;
                }
                _ => {}
            }

            if is_array(new_child) {
                // We created a Fragment for this child with the debug info.
                // No need to push again.
                let first_child = reconcile_children_array(
                    should_track_side_effects,
                    return_fiber,
                    current_first_child,
                    unsafe { js::cast(new_child) },
                    lanes,
                );
                return first_child;
            }

            if get_iterator_fn(new_child).is_some() {
                // We created a Fragment for this child with the debug info.
                // No need to push again.
                let first_child = reconcile_children_iteratable(
                    should_track_side_effects,
                    return_fiber,
                    current_first_child,
                    child,
                    lanes,
                );
                return first_child;
            }

            if ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
                && js::type_of(
                    ASYNC_ITERATOR.with(|&async_iterator| js::get(child, async_iterator)),
                ) == "function"
            {
                // We created a Fragment for this child with the debug info.
                // No need to push again.
                let first_child = reconcile_children_async_iteratable(
                    should_track_side_effects,
                    return_fiber,
                    current_first_child,
                    child,
                    lanes,
                );
                return first_child;
            }

            // Usables are a valid React node type. When React encounters a Usable in
            // a child position, it unwraps it using the same algorithm as `use`. For
            // example, for promises, React will throw an exception to unwind the
            // stack, then replay the component once the promise resolves.
            //
            // A difference from `use` is that React will keep unwrapping the value
            // until it reaches a non-Usable type.
            //
            // e.g. Usable<Usable<Usable<T>>> should resolve to T
            //
            // The structure is a bit unfortunate. Ideally, we shouldn't need to
            // replay the entire begin phase of the parent fiber in order to reconcile
            // the children again. This would require a somewhat significant refactor,
            // because reconcilation happens deep within the begin phase, and
            // depending on the type of work, not always at the end. We should
            // consider as an future improvement.
            if js::type_of(js::get(child, "then")) == "function" {
                let thenable: &'static Thenable = unsafe { js::cast(new_child) };
                let prev_debug_info = push_debug_info(js::get(child, "_debugInfo"));
                let first_child = reconcile_child_fibers_impl(
                    should_track_side_effects,
                    return_fiber,
                    current_first_child,
                    unwrap_thenable(thenable),
                    lanes,
                );
                CURRENT_DEBUG_INFO.set(prev_debug_info);
                return first_child;
            }

            if SymbolFor::from_value(js::get(child, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
                let context: Rc<ReactContext<Option<&'static Unknown>>> =
                    unsafe { js::cast(new_child) };
                return reconcile_child_fibers_impl(
                    should_track_side_effects,
                    return_fiber,
                    current_first_child,
                    read_context_during_reconciliation(return_fiber, &context, lanes),
                    lanes,
                );
            }

            throw_on_invalid_object_type(return_fiber, child);
        }

        if (js::type_of(new_child) == "string" && !js::strict_equals(new_child, ""))
            || js::type_of(new_child) == "number"
            || js::type_of(new_child) == "bigint"
        {
            return Some(place_single_child(
                should_track_side_effects,
                reconcile_single_text_node(
                    should_track_side_effects,
                    return_fiber,
                    current_first_child,
                    // $FlowFixMe[unsafe-addition] Flow doesn't want us to use `+` operator with string and bigint
                    js::concat_text(new_child).unwrap(),
                    lanes,
                ),
            ));
        }

        if __DEV__ {
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "function"
            {
                warn_on_function_type(return_fiber, new_child);
            }
            if let Some(new_child) = new_child
                && js::type_of(Some(new_child)) == "symbol"
            {
                warn_on_symbol_type(return_fiber, new_child);
            }
        }

        // Remaining cases are all treated as empty.
        delete_remaining_children(should_track_side_effects, return_fiber, current_first_child)
    }

    Box::new(
        move |return_fiber: &Rc<Fiber>,
              current_first_child: Option<Rc<Fiber>>,
              new_child: Option<&'static Unknown>,
              lanes: Lanes| {
            let prev_debug_info = CURRENT_DEBUG_INFO.get();
            CURRENT_DEBUG_INFO.set(None);
            let _restore = RestoreDebugInfo(prev_debug_info);
            match catch_unwind(AssertUnwindSafe(|| {
                // This indirection only exists so we can reset `thenableState` at the end.
                // It should get inlined by Closure.
                THENABLE_INDEX_COUNTER.set(0);
                let first_child_fiber = reconcile_child_fibers_impl(
                    should_track_side_effects,
                    return_fiber,
                    current_first_child,
                    new_child,
                    lanes,
                );
                THENABLE_STATE.set(None);
                // Don't bother to reset `thenableIndexCounter` to 0 because it always gets
                // set at the beginning.
                first_child_fiber
            })) {
                Ok(first_child_fiber) => first_child_fiber,
                Err(x) => {
                    let thrown = x
                        .downcast_ref::<&'static JsError>()
                        .and_then(|x| x.as_unknown());
                    if SUSPENSE_EXCEPTION.with(|exception| js::strict_equals(thrown, *exception))
                        || SUSPENSE_ACTION_EXCEPTION
                            .with(|exception| js::strict_equals(thrown, *exception))
                        || (!DISABLE_LEGACY_MODE
                            && (return_fiber.mode.get() & CONCURRENT_MODE) == NO_MODE
                            && js::type_of(thrown) == "object"
                            && thrown.is_some()
                            && js::type_of(js::get(thrown, "then")) == "function")
                    {
                        // Suspense exceptions need to read the current suspended state before
                        // yielding and replay it using the same sequence so this trick doesn't
                        // work here.
                        // Suspending in legacy mode actually mounts so if we let the child
                        // mount then we delete its state in an update.
                        panic::resume_unwind(x);
                    }
                    // Something errored during reconciliation but it's conceptually a child that
                    // errored and not the current component itself so we create a virtual child
                    // that throws in its begin phase. That way the current component can handle
                    // the error or suspending if needed.
                    let throw_fiber =
                        create_fiber_from_throw(thrown, return_fiber.mode.get(), lanes);
                    throw_fiber.r#return.replace(Some(Rc::clone(return_fiber)));
                    if __DEV__ {
                        let debug_info = CURRENT_DEBUG_INFO.get();
                        throw_fiber.debug_info.set(debug_info);
                        // Conceptually the error's owner should ideally be captured when the
                        // Error constructor is called but we don't override them to capture our
                        // `owner`. So instead, we use the nearest parent as the owner/task of the
                        // error. This is usually the same thing when it's thrown from the same
                        // async component but not if you await a promise started from a different
                        // component/task.
                        // In newer Chrome, Error constructor does capture the Task which is what
                        // is logged by reportError. In that case this debugTask isn't used.
                        throw_fiber.debug_owner.set(return_fiber.debug_owner.get());
                        throw_fiber.debug_task.set(return_fiber.debug_task.get());
                        if let Some(debug_info) = debug_info {
                            let debug_info: &[&'static Unknown] =
                                unsafe { js::cast(Some(debug_info)) };
                            for &info in debug_info.iter().rev() {
                                if js::type_of(js::get(info, "stack")) == "string" {
                                    throw_fiber.debug_owner.set(Some(info));
                                    throw_fiber
                                        .debug_task
                                        .set(unsafe { js::cast(js::get(info, "debugTask")) });
                                    break;
                                }
                            }
                        }
                    }
                    Some(throw_fiber)
                }
            }
        },
    )
}

thread_local! {
    pub static RECONCILE_CHILD_FIBERS: Box<ChildReconciler> = create_child_reconciler(true);
    pub static MOUNT_CHILD_FIBERS: Box<ChildReconciler> = create_child_reconciler(false);
}

pub fn reset_child_reconciler_on_unwind() {
    // On unwind, clear any pending thenables that were used.
    THENABLE_STATE.set(None);
    THENABLE_INDEX_COUNTER.set(0);
}

pub fn clone_child_fibers(current: Option<&Fiber>, work_in_progress: &Rc<Fiber>) {
    if let Some(current) = current
        && *work_in_progress.child.borrow() != *current.child.borrow()
    {
        panic!("Resuming work not yet implemented.");
    }

    let Some(mut current_child) = work_in_progress.child.borrow().clone() else {
        return;
    };

    let mut new_child = create_work_in_progress(&current_child, current_child.pending_props.get());
    work_in_progress.child.replace(Some(Rc::clone(&new_child)));

    new_child
        .r#return
        .replace(Some(Rc::clone(work_in_progress)));
    loop {
        let sibling = current_child.sibling.borrow().clone();
        let Some(sibling) = sibling else {
            break;
        };
        current_child = sibling;
        let next = create_work_in_progress(&current_child, current_child.pending_props.get());
        new_child.sibling.replace(Some(Rc::clone(&next)));
        new_child = next;
        new_child
            .r#return
            .replace(Some(Rc::clone(work_in_progress)));
    }
    new_child.sibling.replace(None);
}

// Reset a workInProgress child set to prepare it for a second pass.
pub fn reset_child_fibers(work_in_progress: &Fiber, lanes: Lanes) {
    let mut child = work_in_progress.child.borrow().clone();
    while let Some(current) = child {
        reset_work_in_progress(&current, lanes);
        child = current.sibling.borrow().clone();
    }
}

fn validate_suspense_list_nested_child(child_slot: Option<&'static Unknown>, index: usize) -> bool {
    if __DEV__ {
        let is_an_array = is_array(child_slot);
        let is_iterable = !is_an_array && js::type_of(get_iterator_fn(child_slot)) == "function";
        let is_async_iterable = ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
            && js::type_of(child_slot) == "object"
            && child_slot.is_some()
            && js::type_of(
                ASYNC_ITERATOR.with(|&async_iterator| js::get(child_slot, async_iterator)),
            ) == "function";
        if is_an_array || is_iterable || is_async_iterable {
            let r#type = if is_an_array {
                "array"
            } else if is_async_iterable {
                "async iterable"
            } else {
                "iterable"
            };
            console::error((
                concat!(
                    "A nested %s was passed to row #%s in <SuspenseList />. Wrap it in ",
                    "an additional SuspenseList to configure its revealOrder: ",
                    "<SuspenseList revealOrder=...> ... ",
                    "<SuspenseList revealOrder=...>{%s}</SuspenseList> ... ",
                    "</SuspenseList>"
                ),
                r#type,
                index as f64,
                r#type,
            ));
            return false;
        }
    }
    true
}

// shared/ReactTypes' `SuspenseListRevealOrder`.
pub fn validate_suspense_list_children(
    children: Option<&'static Unknown>,
    reveal_order: Option<&str>,
) {
    if __DEV__
        && reveal_order.is_none_or(|reveal_order| {
            reveal_order == "forwards"
                || reveal_order == "backwards"
                || reveal_order == "unstable_legacy-backwards"
        })
        && children.is_some()
        && !js::strict_equals(children, false)
    {
        if is_array(children) {
            let children: &[Option<&'static Unknown>] = unsafe { js::cast(children) };
            for (i, &child) in children.iter().enumerate() {
                if !validate_suspense_list_nested_child(child, i) {
                    return;
                }
            }
        } else {
            let iterator_fn = get_iterator_fn(children);
            if js::type_of(iterator_fn) == "function" {
                let children_iterator: Option<&'static Iterator<Option<&'static Unknown>>> = unsafe {
                    js::cast(js::function::call(
                        js::cast::<&dyn Fn() -> Option<&'static Unknown>>(iterator_fn),
                        Some(&children),
                        (),
                    ))
                };
                if let Some(children_iterator) = children_iterator {
                    let mut step = children_iterator.next();
                    let mut i = 0;
                    while step.done != Some(true) {
                        if !validate_suspense_list_nested_child(step.value, i) {
                            return;
                        }
                        i += 1;
                        step = children_iterator.next();
                    }
                }
            } else if ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
                && js::type_of(
                    ASYNC_ITERATOR.with(|&async_iterator| js::get(children, async_iterator)),
                ) == "function"
            {
                // TODO: Technically we should warn for nested arrays inside the
                // async iterable but it would require unwrapping the array.
                // However, this mistake is not as easy to make so it's ok not to warn.
            } else if ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
                && SymbolFor::from_value(js::get(children, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
                && js::type_of(js::get(children, "type")) == "function"
                // $FlowFixMe[method-unbinding]
                && (js::object::to_string(js::get(children, "type")).unwrap()
                    == "[object GeneratorFunction]"
                    // $FlowFixMe[incompatible-use]
                    // $FlowFixMe[method-unbinding]
                    || js::object::to_string(js::get(children, "type")).unwrap()
                        == "[object AsyncGeneratorFunction]")
            {
                console::error((
                    concat!(
                        "A generator Component was passed to a <SuspenseList revealOrder=\"%s\" />. ",
                        "This is not supported as a way to generate lists. Instead, pass an ",
                        "iterable as the children."
                    ),
                    reveal_order,
                ));
            } else {
                console::error((
                    concat!(
                        "A single row was passed to a <SuspenseList revealOrder=\"%s\" />. ",
                        "This is not useful since it needs multiple rows. ",
                        "Did you mean to pass multiple children or an array?"
                    ),
                    reveal_order,
                ));
            }
        }
    }
}
