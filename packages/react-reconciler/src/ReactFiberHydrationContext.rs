// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Error, Unknown};
use shared::environment::__DEV__;
use webapi::console;

use crate::react_captured_value::{CapturedValue, create_captured_value_at_fiber};
use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_fiber::create_fiber_from_dehydrated_fragment;
use crate::react_fiber_activity_component::ActivityState;
use crate::react_fiber_host_context::{get_host_context, get_root_host_container};
use crate::react_fiber_hydration_diffs::{
    HydrationDiffNode, ServerProps, ServerTailNode, describe_diff,
};
use crate::react_fiber_lane::OFFSCREEN_LANE;
use crate::react_fiber_suspense_component::SuspenseState;
use crate::react_fiber_tree_context::{
    TreeContext, get_suspended_tree_context, restore_suspended_tree_context,
};
use crate::react_fiber_work_loop::queue_recoverable_errors;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, HOST_COMPONENT, HOST_ROOT, HOST_SINGLETON, SUSPENSE_COMPONENT,
};

/// The errors thrown while hydrating a boundary: one array, which the work
/// loop's recoverable errors and the boundary's state may share.
pub type HydrationErrors = Rc<RefCell<Vec<Rc<CapturedValue<Option<&'static Unknown>>>>>>;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsHydration"]
    safe static SUPPORTS_HYDRATION: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#shouldSetTextContent"
)]
fn should_set_text_content(r#type: &str, props: Option<&'static Unknown>) -> bool {
    unreachable!()
}

// ReactFiberConfig's `HydratableInstance`s.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getNextHydratableSibling",
    rust_js::nullable
)]
fn get_next_hydratable_sibling(instance: Option<&'static Unknown>) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getNextHydratableSiblingAfterSingleton",
    rust_js::nullable
)]
fn get_next_hydratable_sibling_after_singleton(
    r#type: &str,
    current_hydratable_instance: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getFirstHydratableChild",
    rust_js::nullable
)]
fn get_first_hydratable_child(parent_instance: &'static Unknown) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getFirstHydratableChildWithinContainer",
    rust_js::nullable
)]
fn get_first_hydratable_child_within_container(
    parent_container: &'static Unknown,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getFirstHydratableChildWithinActivityInstance",
    rust_js::nullable
)]
fn get_first_hydratable_child_within_activity_instance(
    parent_instance: &'static Unknown,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getFirstHydratableChildWithinSuspenseInstance",
    rust_js::nullable
)]
fn get_first_hydratable_child_within_suspense_instance(
    parent_instance: &'static Unknown,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getFirstHydratableChildWithinSingleton",
    rust_js::nullable
)]
fn get_first_hydratable_child_within_singleton(
    r#type: &str,
    singleton_instance: &'static Unknown,
    current_hydratable_instance: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    unreachable!()
}

// ReactFiberConfig's `Instance` and `HostContext`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hydrateInstance"
)]
fn hydrate_instance(
    instance: Option<&'static Unknown>,
    r#type: &str,
    props: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) -> bool {
    unreachable!()
}

// The props that differ, if any.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#diffHydratedPropsForDevWarnings",
    rust_js::nullable
)]
fn diff_hydrated_props_for_dev_warnings(
    instance: &'static Unknown,
    r#type: &str,
    props: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#describeHydratableInstanceForDevWarnings",
    rust_js::gives_plain_data
)]
fn describe_hydratable_instance_for_dev_warnings(instance: &'static Unknown) -> ServerTailNode {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hydrateTextInstance"
)]
fn hydrate_text_instance(
    text_instance: Option<&'static Unknown>,
    text: &str,
    internal_instance_handle: &Fiber,
    parent_instance_props: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#diffHydratedTextForDevWarnings",
    rust_js::nullable
)]
fn diff_hydrated_text_for_dev_warnings(
    text_instance: Option<&'static Unknown>,
    text: &str,
    parent_props: Option<&'static Unknown>,
) -> Option<&'static str> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hydrateActivityInstance"
)]
fn hydrate_activity_instance(
    activity_instance: &'static Unknown,
    internal_instance_handle: &Fiber,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hydrateSuspenseInstance"
)]
fn hydrate_suspense_instance(
    suspense_instance: &'static Unknown,
    internal_instance_handle: &Fiber,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getNextHydratableInstanceAfterActivityInstance",
    rust_js::nullable
)]
fn get_next_hydratable_instance_after_activity_instance(
    activity_instance: &'static Unknown,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getNextHydratableInstanceAfterSuspenseInstance",
    rust_js::nullable
)]
fn get_next_hydratable_instance_after_suspense_instance(
    suspense_instance: &'static Unknown,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#shouldDeleteUnhydratedTailInstances"
)]
fn should_delete_unhydrated_tail_instances(parent_type: &str) -> bool {
    unreachable!()
}

// ReactFiberConfig's `Instance`, of a `Container`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resolveSingletonInstance"
)]
fn resolve_singleton_instance(
    r#type: &str,
    props: Option<&'static Unknown>,
    root_container_instance: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
    validate_dom_nesting_dev: bool,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#canHydrateInstance",
    rust_js::nullable
)]
fn can_hydrate_instance(
    instance: &'static Unknown,
    r#type: &str,
    props: Option<&'static Unknown>,
    in_root_or_singleton: bool,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#canHydrateTextInstance",
    rust_js::nullable
)]
fn can_hydrate_text_instance(
    instance: &'static Unknown,
    text: &str,
    in_root_or_singleton: bool,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#canHydrateActivityInstance",
    rust_js::nullable
)]
fn can_hydrate_activity_instance(
    instance: &'static Unknown,
    in_root_or_singleton: bool,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#canHydrateSuspenseInstance",
    rust_js::nullable
)]
fn can_hydrate_suspense_instance(
    instance: &'static Unknown,
    in_root_or_singleton: bool,
) -> Option<&'static Unknown> {
    unreachable!()
}

// ReactFiberConfig's `FormStateMarkerInstance`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#canHydrateFormStateMarker",
    rust_js::nullable
)]
fn can_hydrate_form_state_marker(
    instance: &'static Unknown,
    in_root_or_singleton: bool,
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isFormStateMarkerMatching"
)]
fn is_form_state_marker_matching(marker_instance: &'static Unknown) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#validateHydratableInstance"
)]
fn validate_hydratable_instance(
    r#type: &str,
    props: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#validateHydratableTextInstance"
)]
fn validate_hydratable_text_instance(text: &str, host_context: Option<&'static Unknown>) -> bool {
    unreachable!()
}

thread_local! {
    // The deepest Fiber on the stack involved in a hydration context.
    // This may have been an insertion or a hydration.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static HYDRATION_PARENT_FIBER: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    // ReactFiberConfig's `HydratableInstance`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static NEXT_HYDRATABLE_INSTANCE: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
    static IS_HYDRATING: Cell<bool> = const { Cell::new(false) };

    // This flag allows for warning supression when we expect there to be mismatches
    // due to earlier mismatches or a suspended fiber.
    #[cfg_attr(rust_js, rust_js::name = "didSuspendOrErrorDEV")]
    static DID_SUSPEND_OR_ERROR_DEV: Cell<bool> = const { Cell::new(false) };

    // Hydration differences found that haven't yet been logged.
    #[cfg_attr(rust_js, rust_js::name = "hydrationDiffRootDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    static HYDRATION_DIFF_ROOT_DEV: RefCell<Option<Rc<HydrationDiffNode>>> = const { RefCell::new(None) };

    // Hydration errors that were thrown inside this boundary
    #[cfg_attr(rust_js, rust_js::nullable)]
    static HYDRATION_ERRORS: RefCell<Option<HydrationErrors>> = const { RefCell::new(None) };

    static ROOT_OR_SINGLETON_CONTEXT: Cell<bool> = const { Cell::new(false) };
}

// Builds a common ancestor tree from the root down for collecting diffs.
fn build_hydration_diff_node(fiber: &Rc<Fiber>, distance_from_leaf: i32) -> Rc<HydrationDiffNode> {
    let return_fiber = fiber.r#return.borrow().clone();
    let Some(return_fiber) = return_fiber else {
        // We're at the root.
        let diff_root = HYDRATION_DIFF_ROOT_DEV.with_borrow_mut(|diff_root| {
            Rc::clone(diff_root.get_or_insert_with(|| {
                Rc::new(HydrationDiffNode {
                    fiber: Rc::clone(fiber),
                    children: RefCell::new(Vec::new()),
                    server_props: Cell::new(ServerProps::Unknown),
                    server_tail: RefCell::new(Vec::new()),
                    distance_from_leaf: Cell::new(distance_from_leaf),
                })
            }))
        });
        if !Rc::ptr_eq(&diff_root.fiber, fiber) {
            panic!("Saw multiple hydration diff roots in a pass. This is a bug in React.");
        } else if diff_root.distance_from_leaf.get() > distance_from_leaf {
            diff_root.distance_from_leaf.set(distance_from_leaf);
        }
        return diff_root;
    };
    let siblings = &build_hydration_diff_node(&return_fiber, distance_from_leaf + 1).children;
    // The same node may already exist in the parent. Since we currently always render depth first
    // and rerender if we suspend or terminate early, if a shared ancestor was added we should still
    // be inside of that shared ancestor which means it was the last one to be added. If this changes
    // we may have to scan the whole set.
    let last = siblings.borrow().last().cloned();
    if let Some(existing) = last
        && Rc::ptr_eq(&existing.fiber, fiber)
    {
        if existing.distance_from_leaf.get() > distance_from_leaf {
            existing.distance_from_leaf.set(distance_from_leaf);
        }
        return existing;
    }
    let new_node = Rc::new(HydrationDiffNode {
        fiber: Rc::clone(fiber),
        children: RefCell::new(Vec::new()),
        server_props: Cell::new(ServerProps::Unknown),
        server_tail: RefCell::new(Vec::new()),
        distance_from_leaf: Cell::new(distance_from_leaf),
    });
    siblings.borrow_mut().push(Rc::clone(&new_node));
    new_node
}

pub fn warn_if_hydrating() {
    if __DEV__ && IS_HYDRATING.get() {
        console::error((
            "We should not be hydrating here. This is a bug in React. Please file a bug.",
        ));
    }
}

#[cfg_attr(rust_js, rust_js::name = "markDidThrowWhileHydratingDEV")]
pub fn mark_did_throw_while_hydrating_dev() {
    if __DEV__ {
        DID_SUSPEND_OR_ERROR_DEV.set(true);
    }
}

pub fn enter_hydration_state(fiber: &Rc<Fiber>) -> bool {
    if !SUPPORTS_HYDRATION {
        return false;
    }

    let parent_instance = unsafe { js::cast::<&FiberRoot>(fiber.state_node.get()) }.container_info;
    NEXT_HYDRATABLE_INSTANCE.set(get_first_hydratable_child_within_container(parent_instance));
    HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
    IS_HYDRATING.set(true);
    HYDRATION_ERRORS.set(None);
    DID_SUSPEND_OR_ERROR_DEV.set(false);
    HYDRATION_DIFF_ROOT_DEV.set(None);
    ROOT_OR_SINGLETON_CONTEXT.set(true);

    true
}

// ReactFiberConfig's `ActivityInstance`.
pub fn reenter_hydration_state_from_dehydrated_activity_instance(
    fiber: &Rc<Fiber>,
    activity_instance: &'static Unknown,
    tree_context: Option<&TreeContext>,
) -> bool {
    if !SUPPORTS_HYDRATION {
        return false;
    }
    NEXT_HYDRATABLE_INSTANCE.set(get_first_hydratable_child_within_activity_instance(
        activity_instance,
    ));
    HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
    IS_HYDRATING.set(true);
    HYDRATION_ERRORS.set(None);
    DID_SUSPEND_OR_ERROR_DEV.set(false);
    HYDRATION_DIFF_ROOT_DEV.set(None);
    ROOT_OR_SINGLETON_CONTEXT.set(false);
    if let Some(tree_context) = tree_context {
        restore_suspended_tree_context(fiber, tree_context);
    }
    true
}

// ReactFiberConfig's `SuspenseInstance`.
pub fn reenter_hydration_state_from_dehydrated_suspense_instance(
    fiber: &Rc<Fiber>,
    suspense_instance: &'static Unknown,
    tree_context: Option<&TreeContext>,
) -> bool {
    if !SUPPORTS_HYDRATION {
        return false;
    }
    NEXT_HYDRATABLE_INSTANCE.set(get_first_hydratable_child_within_suspense_instance(
        suspense_instance,
    ));
    HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
    IS_HYDRATING.set(true);
    HYDRATION_ERRORS.set(None);
    DID_SUSPEND_OR_ERROR_DEV.set(false);
    HYDRATION_DIFF_ROOT_DEV.set(None);
    ROOT_OR_SINGLETON_CONTEXT.set(false);
    if let Some(tree_context) = tree_context {
        restore_suspended_tree_context(fiber, tree_context);
    }
    true
}

// ReactFiberConfig's `HydratableInstance`.
fn warn_non_hydrated_instance(fiber: &Rc<Fiber>, rejected_candidate: Option<&'static Unknown>) {
    if __DEV__ {
        if DID_SUSPEND_OR_ERROR_DEV.get() {
            // Inside a boundary that already suspended. We're currently rendering the
            // siblings of a suspended node. The mismatch may be due to the missing
            // data, so it's probably a false positive.
            return;
        }

        // Add this fiber to the diff tree.
        let diff_node = build_hydration_diff_node(fiber, 0);
        // We use null as a signal that there was no node to match.
        diff_node.server_props.set(ServerProps::Unmatched);
        if let Some(rejected_candidate) = rejected_candidate {
            let description = describe_hydratable_instance_for_dev_warnings(rejected_candidate);
            diff_node.server_tail.borrow_mut().push(description);
        }
    }
}

// ReactFiberConfig's `HostContext`.
fn try_hydrate_instance(
    fiber: &Rc<Fiber>,
    next_instance: &'static Unknown,
    host_context: Option<&'static Unknown>,
) -> bool {
    // fiber is a HostComponent Fiber
    let instance = can_hydrate_instance(
        next_instance,
        unsafe { js::cast(fiber.r#type.get()) },
        fiber.pending_props.get(),
        ROOT_OR_SINGLETON_CONTEXT.get(),
    );
    if let Some(instance) = instance {
        fiber.state_node.set(Some(instance));

        if __DEV__ && !DID_SUSPEND_OR_ERROR_DEV.get() {
            let differences = diff_hydrated_props_for_dev_warnings(
                instance,
                unsafe { js::cast(fiber.r#type.get()) },
                fiber.pending_props.get(),
                host_context,
            );
            if let Some(differences) = differences {
                let diff_node = build_hydration_diff_node(fiber, 0);
                diff_node.server_props.set(ServerProps::Props(differences));
            }
        }

        HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
        NEXT_HYDRATABLE_INSTANCE.set(get_first_hydratable_child(instance));
        ROOT_OR_SINGLETON_CONTEXT.set(false);
        return true;
    }
    false
}

fn try_hydrate_text(fiber: &Rc<Fiber>, next_instance: &'static Unknown) -> bool {
    // fiber is a HostText Fiber
    let text: &str = unsafe { js::cast(fiber.pending_props.get()) };
    let text_instance =
        can_hydrate_text_instance(next_instance, text, ROOT_OR_SINGLETON_CONTEXT.get());
    if let Some(text_instance) = text_instance {
        fiber.state_node.set(Some(text_instance));
        HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
        // Text Instances don't have children so there's nothing to hydrate.
        NEXT_HYDRATABLE_INSTANCE.set(None);
        return true;
    }
    false
}

// ReactFiberConfig's `ActivityInstance`.
#[cfg_attr(rust_js, rust_js::nullable)]
fn try_hydrate_activity(
    fiber: &Rc<Fiber>,
    next_instance: &'static Unknown,
) -> Option<&'static Unknown> {
    // fiber is a ActivityComponent Fiber
    let activity_instance =
        can_hydrate_activity_instance(next_instance, ROOT_OR_SINGLETON_CONTEXT.get());
    if let Some(activity_instance) = activity_instance {
        let activity_state = ActivityState {
            dehydrated: activity_instance,
            tree_context: get_suspended_tree_context(),
            retry_lane: Cell::new(OFFSCREEN_LANE),
            hydration_errors: RefCell::new(None),
        };
        fiber
            .memoized_state
            .set(Some(js::unknown_of(activity_state)));
        // Store the dehydrated fragment as a child fiber.
        // This simplifies the code for getHostSibling and deleting nodes,
        // since it doesn't have to consider all Suspense boundaries and
        // check if they're dehydrated ones or not.
        let dehydrated_fragment = create_fiber_from_dehydrated_fragment(activity_instance);
        dehydrated_fragment.r#return.replace(Some(Rc::clone(fiber)));
        fiber.child.replace(Some(dehydrated_fragment));
        HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
        // While an Activity Instance does have children, we won't step into
        // it during the first pass. Instead, we'll reenter it later.
        NEXT_HYDRATABLE_INSTANCE.set(None);
    }
    activity_instance
}

// ReactFiberConfig's `SuspenseInstance`.
#[cfg_attr(rust_js, rust_js::nullable)]
fn try_hydrate_suspense(
    fiber: &Rc<Fiber>,
    next_instance: &'static Unknown,
) -> Option<&'static Unknown> {
    // fiber is a SuspenseComponent Fiber
    let suspense_instance =
        can_hydrate_suspense_instance(next_instance, ROOT_OR_SINGLETON_CONTEXT.get());
    if let Some(suspense_instance) = suspense_instance {
        let suspense_state = SuspenseState {
            dehydrated: Some(suspense_instance),
            tree_context: get_suspended_tree_context(),
            retry_lane: Cell::new(OFFSCREEN_LANE),
            hydration_errors: RefCell::new(None),
        };
        fiber
            .memoized_state
            .set(Some(js::unknown_of(suspense_state)));
        // Store the dehydrated fragment as a child fiber.
        // This simplifies the code for getHostSibling and deleting nodes,
        // since it doesn't have to consider all Suspense boundaries and
        // check if they're dehydrated ones or not.
        let dehydrated_fragment = create_fiber_from_dehydrated_fragment(suspense_instance);
        dehydrated_fragment.r#return.replace(Some(Rc::clone(fiber)));
        fiber.child.replace(Some(dehydrated_fragment));
        HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
        // While a Suspense Instance does have children, we won't step into
        // it during the first pass. Instead, we'll reenter it later.
        NEXT_HYDRATABLE_INSTANCE.set(None);
    }
    suspense_instance
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "HydrationMismatchException")]
    pub static HYDRATION_MISMATCH_EXCEPTION: &'static Error = js::error::new(concat!(
        "Hydration Mismatch Exception: This is not a real error, and should not leak into ",
        "userspace. If you're seeing this, it's likely a bug in React."
    ), None);
}

fn throw_on_hydration_mismatch(fiber: &Rc<Fiber>, from_text: bool) -> ! {
    let mut diff = String::new();
    if __DEV__ {
        // Consume the diff root for this mismatch.
        // Any other errors will get their own diffs.
        let diff_root = HYDRATION_DIFF_ROOT_DEV.take();
        if let Some(diff_root) = diff_root {
            diff = describe_diff(&diff_root);
        }
    }
    let error = js::error::new(
        &format!(
            concat!(
                "Hydration failed because the server rendered {} didn't match the client. As a result this tree will be regenerated on the client. This can happen if a SSR-ed Client Component used:\n",
                "\n",
                "- A server/client branch `if (typeof window !== 'undefined')`.\n",
                "- Variable input such as `Date.now()` or `Math.random()` which changes each time it's called.\n",
                "- Date formatting in a user's locale which doesn't match the server.\n",
                "- External changing data without sending a snapshot of it along with the HTML.\n",
                "- Invalid HTML tag nesting.\n",
                "\n",
                "It can also happen if the client has a browser extension installed which messes with the HTML before React loaded.\n",
                "\n",
                "https://react.dev/link/hydration-mismatch{}"
            ),
            if from_text { "text" } else { "HTML" },
            diff
        ),
        None,
    );
    queue_hydration_error(create_captured_value_at_fiber(error.as_unknown(), fiber));
    HYDRATION_MISMATCH_EXCEPTION.with(|exception| js::throw(exception.as_unknown()))
}

pub fn claim_hydratable_singleton(fiber: &Rc<Fiber>) {
    if SUPPORTS_SINGLETONS {
        if !IS_HYDRATING.get() {
            return;
        }
        let current_root_container = get_root_host_container();
        let current_host_context = get_host_context();
        let instance = resolve_singleton_instance(
            unsafe { js::cast(fiber.r#type.get()) },
            fiber.pending_props.get(),
            current_root_container,
            current_host_context,
            false,
        );
        fiber.state_node.set(Some(instance));

        if __DEV__ && !DID_SUSPEND_OR_ERROR_DEV.get() {
            let differences = diff_hydrated_props_for_dev_warnings(
                instance,
                unsafe { js::cast(fiber.r#type.get()) },
                fiber.pending_props.get(),
                current_host_context,
            );
            if let Some(differences) = differences {
                let diff_node = build_hydration_diff_node(fiber, 0);
                diff_node.server_props.set(ServerProps::Props(differences));
            }
        }

        HYDRATION_PARENT_FIBER.set(Some(Rc::clone(fiber)));
        ROOT_OR_SINGLETON_CONTEXT.set(true);
        NEXT_HYDRATABLE_INSTANCE.set(get_first_hydratable_child_within_singleton(
            unsafe { js::cast(fiber.r#type.get()) },
            instance,
            NEXT_HYDRATABLE_INSTANCE.get(),
        ));
    }
}

pub fn try_to_claim_next_hydratable_instance(fiber: &Rc<Fiber>) {
    if !IS_HYDRATING.get() {
        return;
    }

    // Validate that this is ok to render here before any mismatches.
    let current_host_context = get_host_context();
    let should_keep_warning = validate_hydratable_instance(
        unsafe { js::cast(fiber.r#type.get()) },
        fiber.pending_props.get(),
        current_host_context,
    );

    let next_instance = NEXT_HYDRATABLE_INSTANCE.get();
    if next_instance.is_none_or(|next_instance| {
        !try_hydrate_instance(fiber, next_instance, current_host_context)
    }) {
        if should_keep_warning {
            warn_non_hydrated_instance(fiber, next_instance);
        }
        throw_on_hydration_mismatch(fiber, false);
    }
}

pub fn try_to_claim_next_hydratable_text_instance(fiber: &Rc<Fiber>) {
    if !IS_HYDRATING.get() {
        return;
    }
    let text: &str = unsafe { js::cast(fiber.pending_props.get()) };

    // Validate that this is ok to render here before any mismatches.
    let current_host_context = get_host_context();
    let should_keep_warning = validate_hydratable_text_instance(text, current_host_context);

    let next_instance = NEXT_HYDRATABLE_INSTANCE.get();
    if next_instance.is_none_or(|next_instance| !try_hydrate_text(fiber, next_instance)) {
        if should_keep_warning {
            warn_non_hydrated_instance(fiber, next_instance);
        }
        throw_on_hydration_mismatch(fiber, false);
    }
}

// ReactFiberConfig's `ActivityInstance`.
pub fn claim_next_hydratable_activity_instance(fiber: &Rc<Fiber>) -> &'static Unknown {
    let next_instance = NEXT_HYDRATABLE_INSTANCE.get();
    let activity_instance =
        next_instance.and_then(|next_instance| try_hydrate_activity(fiber, next_instance));
    let Some(activity_instance) = activity_instance else {
        warn_non_hydrated_instance(fiber, next_instance);
        throw_on_hydration_mismatch(fiber, false);
    };
    activity_instance
}

// ReactFiberConfig's `SuspenseInstance`.
pub fn claim_next_hydratable_suspense_instance(fiber: &Rc<Fiber>) -> &'static Unknown {
    let next_instance = NEXT_HYDRATABLE_INSTANCE.get();
    let suspense_instance =
        next_instance.and_then(|next_instance| try_hydrate_suspense(fiber, next_instance));
    let Some(suspense_instance) = suspense_instance else {
        warn_non_hydrated_instance(fiber, next_instance);
        throw_on_hydration_mismatch(fiber, false);
    };
    suspense_instance
}

pub fn try_to_claim_next_hydratable_form_marker_instance(fiber: &Rc<Fiber>) -> bool {
    if !IS_HYDRATING.get() {
        return false;
    }
    if let Some(next_hydratable_instance) = NEXT_HYDRATABLE_INSTANCE.get() {
        let marker_instance = can_hydrate_form_state_marker(
            next_hydratable_instance,
            ROOT_OR_SINGLETON_CONTEXT.get(),
        );
        if let Some(marker_instance) = marker_instance {
            // Found the marker instance.
            NEXT_HYDRATABLE_INSTANCE.set(get_next_hydratable_sibling(Some(marker_instance)));
            // Return true if this marker instance should use the state passed
            // to hydrateRoot.
            // TODO: As an optimization, Fizz should only emit these markers if form
            // state is passed at the root.
            return is_form_state_marker_matching(marker_instance);
        }
    }
    // Should have found a marker instance. Throw an error to trigger client
    // rendering. We don't bother to check if we're in a concurrent root because
    // useActionState is a new API, so backwards compat is not an issue.
    throw_on_hydration_mismatch(fiber, false);
}

// ReactFiberConfig's `HostContext`.
pub fn prepare_to_hydrate_host_instance(fiber: &Rc<Fiber>, host_context: Option<&'static Unknown>) {
    if !SUPPORTS_HYDRATION {
        panic!(concat!(
            "Expected prepareToHydrateHostInstance() to never be called. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    }

    let instance = fiber.state_node.get();
    let did_hydrate = hydrate_instance(
        instance,
        unsafe { js::cast(fiber.r#type.get()) },
        fiber.memoized_props.get(),
        host_context,
        fiber,
    );
    if !did_hydrate {
        throw_on_hydration_mismatch(fiber, true);
    }
}

pub fn prepare_to_hydrate_host_text_instance(fiber: &Rc<Fiber>) {
    if !SUPPORTS_HYDRATION {
        panic!(concat!(
            "Expected prepareToHydrateHostTextInstance() to never be called. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    }

    let text_instance = fiber.state_node.get();
    let text_content: &str = unsafe { js::cast(fiber.memoized_props.get()) };
    let should_warn_if_mismatch_dev = !DID_SUSPEND_OR_ERROR_DEV.get();
    let mut parent_props = js::null();
    // We assume that prepareToHydrateHostTextInstance is called in a context where the
    // hydration parent is the parent host component of this host text.
    let return_fiber = HYDRATION_PARENT_FIBER.with_borrow(Clone::clone);
    if let Some(return_fiber) = return_fiber {
        match return_fiber.tag.get() {
            HOST_ROOT => {
                if __DEV__ && should_warn_if_mismatch_dev {
                    let difference = diff_hydrated_text_for_dev_warnings(
                        text_instance,
                        text_content,
                        parent_props,
                    );
                    if let Some(difference) = difference {
                        let diff_node = build_hydration_diff_node(fiber, 0);
                        diff_node.server_props.set(ServerProps::Text(difference));
                    }
                }
            }
            HOST_SINGLETON | HOST_COMPONENT => {
                parent_props = return_fiber.memoized_props.get();
                if __DEV__ && should_warn_if_mismatch_dev {
                    let difference = diff_hydrated_text_for_dev_warnings(
                        text_instance,
                        text_content,
                        parent_props,
                    );
                    if let Some(difference) = difference {
                        let diff_node = build_hydration_diff_node(fiber, 0);
                        diff_node.server_props.set(ServerProps::Text(difference));
                    }
                }
            }
            _ => {}
        }
        // TODO: What if it's a SuspenseInstance?
    }

    let did_hydrate = hydrate_text_instance(text_instance, text_content, fiber, parent_props);
    if !did_hydrate {
        throw_on_hydration_mismatch(fiber, true);
    }
}

pub fn prepare_to_hydrate_host_activity_instance(fiber: &Rc<Fiber>) {
    if !SUPPORTS_HYDRATION {
        panic!(concat!(
            "Expected prepareToHydrateHostActivityInstance() to never be called. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    }
    let activity_state: Option<&ActivityState> = unsafe { js::cast(fiber.memoized_state.get()) };
    let activity_instance = activity_state.map(|activity_state| activity_state.dehydrated);

    let Some(activity_instance) = activity_instance else {
        panic!(concat!(
            "Expected to have a hydrated activity instance. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    };

    hydrate_activity_instance(activity_instance, fiber);
}

pub fn prepare_to_hydrate_host_suspense_instance(fiber: &Rc<Fiber>) {
    if !SUPPORTS_HYDRATION {
        panic!(concat!(
            "Expected prepareToHydrateHostSuspenseInstance() to never be called. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    }

    let suspense_state: Option<&SuspenseState> = unsafe { js::cast(fiber.memoized_state.get()) };
    let suspense_instance = suspense_state.and_then(|suspense_state| suspense_state.dehydrated);

    let Some(suspense_instance) = suspense_instance else {
        panic!(concat!(
            "Expected to have a hydrated suspense instance. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    };

    hydrate_suspense_instance(suspense_instance, fiber);
}

// ReactFiberConfig's `HydratableInstance`.
#[cfg_attr(rust_js, rust_js::nullable)]
fn skip_past_dehydrated_activity_instance(fiber: &Fiber) -> Option<&'static Unknown> {
    let activity_state: Option<&ActivityState> = unsafe { js::cast(fiber.memoized_state.get()) };
    let activity_instance = activity_state.map(|activity_state| activity_state.dehydrated);

    let Some(activity_instance) = activity_instance else {
        panic!(concat!(
            "Expected to have a hydrated suspense instance. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    };

    get_next_hydratable_instance_after_activity_instance(activity_instance)
}

// ReactFiberConfig's `HydratableInstance`.
#[cfg_attr(rust_js, rust_js::nullable)]
fn skip_past_dehydrated_suspense_instance(fiber: &Fiber) -> Option<&'static Unknown> {
    if !SUPPORTS_HYDRATION {
        panic!(concat!(
            "Expected skipPastDehydratedSuspenseInstance() to never be called. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    }
    let suspense_state: Option<&SuspenseState> = unsafe { js::cast(fiber.memoized_state.get()) };
    let suspense_instance = suspense_state.and_then(|suspense_state| suspense_state.dehydrated);

    let Some(suspense_instance) = suspense_instance else {
        panic!(concat!(
            "Expected to have a hydrated suspense instance. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    };

    get_next_hydratable_instance_after_suspense_instance(suspense_instance)
}

fn pop_to_next_host_parent(fiber: &Fiber) {
    HYDRATION_PARENT_FIBER.set(fiber.r#return.borrow().clone());
    while let Some(hydration_parent_fiber) = HYDRATION_PARENT_FIBER.with_borrow(Clone::clone) {
        match hydration_parent_fiber.tag.get() {
            HOST_COMPONENT | ACTIVITY_COMPONENT | SUSPENSE_COMPONENT => {
                ROOT_OR_SINGLETON_CONTEXT.set(false);
                return;
            }
            HOST_SINGLETON | HOST_ROOT => {
                ROOT_OR_SINGLETON_CONTEXT.set(true);
                return;
            }
            _ => HYDRATION_PARENT_FIBER.set(hydration_parent_fiber.r#return.borrow().clone()),
        }
    }
}

pub fn pop_hydration_state(fiber: &Rc<Fiber>) -> bool {
    if !SUPPORTS_HYDRATION {
        return false;
    }
    if HYDRATION_PARENT_FIBER
        .with_borrow(|hydration_parent_fiber| hydration_parent_fiber.as_ref() != Some(fiber))
    {
        // We're deeper than the current hydration context, inside an inserted
        // tree.
        return false;
    }
    if !IS_HYDRATING.get() {
        // If we're not currently hydrating but we're in a hydration context, then
        // we were an insertion and now need to pop up reenter hydration of our
        // siblings.
        pop_to_next_host_parent(fiber);
        IS_HYDRATING.set(true);
        return false;
    }

    let tag = fiber.tag.get();

    if SUPPORTS_SINGLETONS {
        // With float we never clear the Root, or Singleton instances. We also do not clear Instances
        // that have singleton text content
        if tag != HOST_ROOT
            && tag != HOST_SINGLETON
            && !(tag == HOST_COMPONENT
                && (!should_delete_unhydrated_tail_instances(unsafe {
                    js::cast(fiber.r#type.get())
                }) || should_set_text_content(
                    unsafe { js::cast(fiber.r#type.get()) },
                    fiber.memoized_props.get(),
                )))
        {
            let next_instance = NEXT_HYDRATABLE_INSTANCE.get();
            if next_instance.is_some() {
                warn_if_unhydrated_tail_nodes(fiber);
                throw_on_hydration_mismatch(fiber, false);
            }
        }
    } else {
        // If we have any remaining hydratable nodes, we need to delete them now.
        // We only do this deeper than head and body since they tend to have random
        // other nodes in them. We also ignore components with pure text content in
        // side of them. We also don't delete anything inside the root container.
        if tag != HOST_ROOT
            && (tag != HOST_COMPONENT
                || (should_delete_unhydrated_tail_instances(unsafe {
                    js::cast(fiber.r#type.get())
                }) && !should_set_text_content(
                    unsafe { js::cast(fiber.r#type.get()) },
                    fiber.memoized_props.get(),
                )))
        {
            let next_instance = NEXT_HYDRATABLE_INSTANCE.get();
            if next_instance.is_some() {
                warn_if_unhydrated_tail_nodes(fiber);
                throw_on_hydration_mismatch(fiber, false);
            }
        }
    }
    pop_to_next_host_parent(fiber);
    if tag == SUSPENSE_COMPONENT {
        NEXT_HYDRATABLE_INSTANCE.set(skip_past_dehydrated_suspense_instance(fiber));
    } else if tag == ACTIVITY_COMPONENT {
        NEXT_HYDRATABLE_INSTANCE.set(skip_past_dehydrated_activity_instance(fiber));
    } else if SUPPORTS_SINGLETONS && tag == HOST_SINGLETON {
        NEXT_HYDRATABLE_INSTANCE.set(get_next_hydratable_sibling_after_singleton(
            unsafe { js::cast(fiber.r#type.get()) },
            NEXT_HYDRATABLE_INSTANCE.get(),
        ));
    } else {
        NEXT_HYDRATABLE_INSTANCE.set(
            if HYDRATION_PARENT_FIBER
                .with_borrow(|hydration_parent_fiber| hydration_parent_fiber.is_some())
            {
                get_next_hydratable_sibling(fiber.state_node.get())
            } else {
                js::null()
            },
        );
    }
    true
}

fn warn_if_unhydrated_tail_nodes(fiber: &Rc<Fiber>) {
    if __DEV__ {
        let mut next_instance = NEXT_HYDRATABLE_INSTANCE.get();
        while let Some(instance) = next_instance {
            let diff_node = build_hydration_diff_node(fiber, 0);
            let description = describe_hydratable_instance_for_dev_warnings(instance);
            let is_suspense = matches!(
                &description,
                ServerTailNode::Element(element) if element.r#type == "Suspense"
            );
            diff_node.server_tail.borrow_mut().push(description);
            if is_suspense {
                // ReactFiberConfig's `SuspenseInstance`.
                let suspense_instance = instance;
                next_instance =
                    get_next_hydratable_instance_after_suspense_instance(suspense_instance);
            } else {
                next_instance = get_next_hydratable_sibling(Some(instance));
            }
        }
    }
}

pub fn reset_hydration_state() {
    if !SUPPORTS_HYDRATION {
        return;
    }

    HYDRATION_PARENT_FIBER.set(None);
    NEXT_HYDRATABLE_INSTANCE.set(None);
    IS_HYDRATING.set(false);
    DID_SUSPEND_OR_ERROR_DEV.set(false);
}

// Restore the hydration cursor when unwinding a HostComponent that already
// claimed a DOM node. This is a fork of popHydrationState that does all the
// same validity checks but restores the cursor to this fiber's DOM node
// instead of advancing past it. It also does NOT clear unhydrated tail nodes
// or throw on mismatches since we're unwinding, not completing.
//
// This is needed when replaySuspendedUnitOfWork calls unwindInterruptedWork
// before re-running beginWork on the same fiber, or when throwAndUnwindWorkLoop
// calls unwindWork on ancestor fibers.
pub fn pop_hydration_state_on_interrupted_work(fiber: &Rc<Fiber>) {
    if !SUPPORTS_HYDRATION {
        return;
    }
    if HYDRATION_PARENT_FIBER
        .with_borrow(|hydration_parent_fiber| hydration_parent_fiber.as_ref() != Some(fiber))
    {
        // We're deeper than the current hydration context, inside an inserted
        // tree. Don't touch the cursor.
        return;
    }
    if !IS_HYDRATING.get() {
        // If we're not currently hydrating but we're in a hydration context, then
        // we were an insertion and now need to pop up to reenter hydration of our
        // siblings. Same as popHydrationState.
        pop_to_next_host_parent(fiber);
        IS_HYDRATING.set(true);
        return;
    }

    // We're in a valid hydration context. Restore the cursor to this fiber's
    // DOM node so that when beginWork re-runs, it can claim the same node.
    // Unlike popHydrationState, we do NOT check for unhydrated tail nodes
    // or advance the cursor - we're restoring, not completing.
    pop_to_next_host_parent(fiber);
    if fiber.tag.get() == HOST_COMPONENT && fiber.state_node.get().is_some() {
        NEXT_HYDRATABLE_INSTANCE.set(fiber.state_node.get());
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn upgrade_hydration_errors_to_recoverable() -> Option<HydrationErrors> {
    let queued_errors = HYDRATION_ERRORS.with_borrow(Clone::clone);
    if let Some(queued_errors) = &queued_errors {
        // Successfully completed a forced client render. The errors that occurred
        // during the hydration attempt are now recovered. We will log them in
        // commit phase, once the entire tree has finished.
        queue_recoverable_errors(queued_errors);
        HYDRATION_ERRORS.set(None);
    }
    queued_errors
}

pub fn get_is_hydrating() -> bool {
    IS_HYDRATING.get()
}

pub fn queue_hydration_error(error: Rc<CapturedValue<Option<&'static Unknown>>>) {
    HYDRATION_ERRORS.with_borrow_mut(|hydration_errors| match hydration_errors {
        None => *hydration_errors = Some(Rc::new(RefCell::new(vec![error]))),
        Some(hydration_errors) => hydration_errors.borrow_mut().push(error),
    });
}

pub fn emit_pending_hydration_warnings() {
    if __DEV__ {
        // If we haven't yet thrown any hydration errors by the time we reach the end we've successfully
        // hydrated, however, we might still have DEV-only mismatches that we log now.
        let diff_root = HYDRATION_DIFF_ROOT_DEV.take();
        if let Some(diff_root) = diff_root {
            let diff = describe_diff(&diff_root);

            // Just pick the DFS-first leaf as the owner.
            // Should be good enough since most warnings only have a single error.
            let mut diff_owner = Rc::clone(&diff_root);
            while !diff_owner.children.borrow().is_empty() {
                let child = Rc::clone(&diff_owner.children.borrow()[0]);
                diff_owner = child;
            }

            run_with_fiber_in_dev(
                Some(Rc::clone(&diff_owner.fiber)),
                |_: (), _: (), _: (), _: (), _: ()| {
                    console::error((
                        concat!(
                            "A tree hydrated but some attributes of the server rendered HTML didn't match the client properties. This won't be patched up. ",
                            "This can happen if a SSR-ed Client Component used:\n",
                            "\n",
                            "- A server/client branch `if (typeof window !== 'undefined')`.\n",
                            "- Variable input such as `Date.now()` or `Math.random()` which changes each time it's called.\n",
                            "- Date formatting in a user's locale which doesn't match the server.\n",
                            "- External changing data without sending a snapshot of it along with the HTML.\n",
                            "- Invalid HTML tag nesting.\n",
                            "\n",
                            "It can also happen if the client has a browser extension installed which messes with the HTML before React loaded.\n",
                            "\n",
                            "%s%s"
                        ),
                        "https://react.dev/link/hydration-mismatch",
                        &diff,
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
