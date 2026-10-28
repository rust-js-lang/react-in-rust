// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Error, Unknown, function};
use shared::environment::__DEV__;
use shared::react_feature_flags::{DISABLE_LEGACY_MODE, ENABLE_UPDATER_TRACKING};
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_captured_value::{CapturedValue, create_captured_value_at_fiber};
use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_fiber_call_user_space::{
    CALL_COMPONENT_DID_CATCH_IN_DEV, ClassInstance, DidCatchInfo,
};
use crate::react_fiber_class_update_queue::{
    CAPTURE_UPDATE, FORCE_UPDATE, Update, create_update, enqueue_captured_update, enqueue_update,
};
use crate::react_fiber_dev_tools_hook::IS_DEV_TOOLS_PRESENT;
use crate::react_fiber_error_logger::{log_caught_error, log_uncaught_error};
use crate::react_fiber_flags::{
    DID_CAPTURE, FORCE_CLIENT_RENDER, FORCE_UPDATE_FOR_LEGACY_SUSPENSE, INCOMPLETE,
    LIFECYCLE_EFFECT_MASK, NO_FLAGS, SCHEDULE_RETRY, SHOULD_CAPTURE,
};
use crate::react_fiber_hot_reloading::mark_failed_error_boundary_for_hot_reloading;
use crate::react_fiber_hydration_context::{
    HYDRATION_MISMATCH_EXCEPTION, get_is_hydrating, mark_did_throw_while_hydrating_dev,
    queue_hydration_error,
};
use crate::react_fiber_lane::{
    Lane, Lanes, SYNC_LANE, includes_some_lane, merge_lanes, pick_arbitrary_lane,
};
use crate::react_fiber_new_context::propagate_parent_context_changes_to_deferred_tree;
use crate::react_fiber_offscreen_component::{OffscreenQueue, OffscreenState};
use crate::react_fiber_suspense_component::RetryQueue;
use crate::react_fiber_suspense_context::{get_shell_boundary, get_suspense_handler};
use crate::react_fiber_thenable::NOOP_SUSPENSEY_COMMIT_THENABLE;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_root_tags::CONCURRENT_ROOT;
use crate::react_type_of_mode::{CONCURRENT_MODE, NO_MODE};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CLASS_COMPONENT, FORWARD_REF, FUNCTION_COMPONENT, HOST_ROOT,
    INCOMPLETE_CLASS_COMPONENT, INCOMPLETE_FUNCTION_COMPONENT, OFFSCREEN_COMPONENT,
    SIMPLE_MEMO_COMPONENT, SUSPENSE_COMPONENT, SUSPENSE_LIST_COMPONENT,
};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#renderDidError"
)]
fn render_did_error() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#queueConcurrentError"
)]
fn queue_concurrent_error(error: Rc<CapturedValue<Option<&'static Unknown>>>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#renderDidSuspendDelayIfPossible"
)]
fn render_did_suspend_delay_if_possible() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#markLegacyErrorBoundaryAsFailed"
)]
fn mark_legacy_error_boundary_as_failed(instance: &ClassInstance) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#isAlreadyFailedLegacyErrorBoundary"
)]
fn is_already_failed_legacy_error_boundary(instance: &Unknown) -> bool {
    unreachable!()
}

// shared/ReactTypes' `Wakeable`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#attachPingListener"
)]
fn attach_ping_listener(root: &FiberRoot, wakeable: &'static Unknown, lanes: Lanes) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#restorePendingUpdaters"
)]
fn restore_pending_updaters(root: &FiberRoot, lanes: Lanes) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#renderDidSuspend"
)]
fn render_did_suspend() {
    unreachable!()
}

/// What `new Error(message, options)` is given: what caused the error.
struct ErrorOptions {
    cause: Option<&'static Unknown>,
}

unsafe impl js::Defined for ErrorOptions {}

/// The payload of a root's error update: the element it renders, none.
struct RootErrorPayload {
    // Caution: React DevTools currently depends on this property
    // being called "element".
    #[cfg_attr(rust_js, rust_js::nullable)]
    element: Option<&'static Unknown>,
}

unsafe impl js::Defined for RootErrorPayload {}

pub fn create_root_error_update(
    root: &Rc<FiberRoot>,
    error_info: &Rc<CapturedValue<Option<&'static Unknown>>>,
    lane: Lane,
) -> Rc<Update> {
    let update = create_update(lane);
    // Unmount the root by rendering null.
    update.tag.set(CAPTURE_UPDATE);
    // Caution: React DevTools currently depends on this property
    // being called "element".
    update
        .payload
        .set(Some(js::unknown_of(RootErrorPayload { element: None })));
    let root = Rc::clone(root);
    let error_info = Rc::clone(error_info);
    update.callback.set(Some(js::unknown_of(move || {
        if __DEV__ {
            run_with_fiber_in_dev(
                error_info.source.clone(),
                |root: &FiberRoot,
                 error_info: &CapturedValue<Option<&'static Unknown>>,
                 _: (),
                 _: (),
                 _: ()| { log_uncaught_error(root, error_info) },
                &*root,
                &*error_info,
                (),
                (),
                (),
            );
        } else {
            log_uncaught_error(&root, &error_info);
        }
    })));
    update
}

pub fn create_class_error_update(lane: Lane) -> Rc<Update> {
    let update = create_update(lane);
    update.tag.set(CAPTURE_UPDATE);
    update
}

pub fn initialize_class_error_update(
    update: &Update,
    root: &Rc<FiberRoot>,
    fiber: &Rc<Fiber>,
    error_info: &Rc<CapturedValue<Option<&'static Unknown>>>,
) {
    let get_derived_state_from_error = js::get(fiber.r#type.get(), "getDerivedStateFromError");
    if js::type_of(get_derived_state_from_error) == "function" {
        let get_derived_state_from_error: &'static dyn Fn(
            Option<&'static Unknown>,
        ) -> Option<&'static Unknown> = unsafe { js::cast(get_derived_state_from_error) };
        let error = error_info.value;
        update.payload.set(Some(js::unknown_of(move || {
            get_derived_state_from_error(error)
        })));
        let root = Rc::clone(root);
        let fiber = Rc::clone(fiber);
        let error_info = Rc::clone(error_info);
        update.callback.set(Some(js::unknown_of(move || {
            if __DEV__ {
                mark_failed_error_boundary_for_hot_reloading(&fiber);
            }
            if __DEV__ {
                run_with_fiber_in_dev(
                    error_info.source.clone(),
                    |root: &FiberRoot,
                     fiber: &Fiber,
                     error_info: &CapturedValue<Option<&'static Unknown>>,
                     _: (),
                     _: ()| log_caught_error(root, fiber, error_info),
                    &*root,
                    &*fiber,
                    &*error_info,
                    (),
                    (),
                );
            } else {
                log_caught_error(&root, &fiber, &error_info);
            }
        })));
    }

    let inst = fiber.state_node.get();
    if inst.is_some() && js::type_of(js::get(inst, "componentDidCatch")) == "function" {
        let root = Rc::clone(root);
        let fiber = Rc::clone(fiber);
        let error_info = Rc::clone(error_info);
        update.callback.set(Some(js::unknown_of(function::with_this(move |this, ()| {
            let this: &'static ClassInstance = unsafe { js::cast(this) };
            if __DEV__ {
                mark_failed_error_boundary_for_hot_reloading(&fiber);
            }
            if __DEV__ {
                run_with_fiber_in_dev(
                    error_info.source.clone(),
                    |root: &FiberRoot,
                     fiber: &Fiber,
                     error_info: &CapturedValue<Option<&'static Unknown>>,
                     _: (),
                     _: ()| log_caught_error(root, fiber, error_info),
                    &*root,
                    &*fiber,
                    &*error_info,
                    (),
                    (),
                );
            } else {
                log_caught_error(&root, &fiber, &error_info);
            }
            if js::type_of(get_derived_state_from_error) != "function" {
                // To preserve the preexisting retry behavior of error boundaries,
                // we keep track of which ones already failed during this batch.
                // This gets reset before we yield back to the browser.
                // TODO: Warn in strict mode if getDerivedStateFromError is
                // not defined.
                mark_legacy_error_boundary_as_failed(this);
            }
            if __DEV__ {
                CALL_COMPONENT_DID_CATCH_IN_DEV.with(|call_component_did_catch_in_dev| {
                    call_component_did_catch_in_dev.unwrap()(this, &error_info)
                });
            } else {
                let error = error_info.value;
                let stack = error_info.stack.clone();
                this.component_did_catch(
                    error,
                    &DidCatchInfo {
                        component_stack: stack.unwrap_or_default(),
                    },
                );
            }
            if __DEV__
                && js::type_of(get_derived_state_from_error) != "function"
                // If componentDidCatch is the only error boundary method defined,
                // then it needs to call setState to recover from errors.
                // If no state update is scheduled then the boundary will swallow the error.
                && !includes_some_lane(fiber.lanes.get(), SYNC_LANE)
            {
                console::error((
                    concat!(
                        "%s: Error boundaries should implement getDerivedStateFromError(). ",
                        "In that method, return a state update to display an error message or fallback UI."
                    ),
                    get_component_name_from_fiber(&fiber)
                        .filter(|name| js::truthy(Some(name.as_str())))
                        .unwrap_or_else(|| "Unknown".to_string()),
                ));
            }
        }))));
    }
}

fn reset_suspended_component(source_fiber: &Rc<Fiber>, root_render_lanes: Lanes) {
    let current_source_fiber = source_fiber.alternate.borrow().clone();
    if let Some(current_source_fiber) = current_source_fiber {
        // Since we never visited the children of the suspended component, we
        // need to propagate the context change now, to ensure that we visit
        // them during the retry.
        //
        // We don't have to do this for errors because we retry errors without
        // committing in between. So this is specific to Suspense.
        propagate_parent_context_changes_to_deferred_tree(
            &current_source_fiber,
            source_fiber,
            root_render_lanes,
        );
    }

    // Reset the memoizedState to what it was before we attempted to render it.
    // A legacy mode Suspense quirk, only relevant to hook components.
    let tag = source_fiber.tag.get();
    if !DISABLE_LEGACY_MODE
        && (source_fiber.mode.get() & CONCURRENT_MODE) == NO_MODE
        && (tag == FUNCTION_COMPONENT || tag == FORWARD_REF || tag == SIMPLE_MEMO_COMPONENT)
    {
        let current_source = source_fiber.alternate.borrow().clone();
        if let Some(current_source) = current_source {
            source_fiber
                .update_queue
                .set(current_source.update_queue.get());
            source_fiber
                .memoized_state
                .set(current_source.memoized_state.get());
            source_fiber.lanes.set(current_source.lanes.get());
        } else {
            source_fiber.update_queue.set(js::null());
            source_fiber.memoized_state.set(js::null());
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn mark_suspense_boundary_should_capture(
    suspense_boundary: &Rc<Fiber>,
    return_fiber: Option<&Rc<Fiber>>,
    source_fiber: &Rc<Fiber>,
    _root: &FiberRoot,
    root_render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // This marks a Suspense boundary so that when we're unwinding the stack,
    // it captures the suspended "exception" and does a second (fallback) pass.
    if !DISABLE_LEGACY_MODE && (suspense_boundary.mode.get() & CONCURRENT_MODE) == NO_MODE {
        // Legacy Mode Suspense
        //
        // If the boundary is in legacy mode, we should *not*
        // suspend the commit. Pretend as if the suspended component rendered
        // null and keep rendering. When the Suspense boundary completes,
        // we'll do a second pass to render the fallback.
        if Some(suspense_boundary) == return_fiber {
            // Special case where we suspended while reconciling the children of
            // a Suspense boundary's inner Offscreen wrapper fiber. This happens
            // when a React.lazy component is a direct child of a
            // Suspense boundary.
            //
            // Suspense boundaries are implemented as multiple fibers, but they
            // are a single conceptual unit. The legacy mode behavior where we
            // pretend the suspended fiber committed as `null` won't work,
            // because in this case the "suspended" fiber is the inner
            // Offscreen wrapper.
            //
            // Because the contents of the boundary haven't started rendering
            // yet (i.e. nothing in the tree has partially rendered) we can
            // switch to the regular, concurrent mode behavior: mark the
            // boundary with ShouldCapture and enter the unwind phase.
            suspense_boundary
                .flags
                .set(suspense_boundary.flags.get() | SHOULD_CAPTURE);
        } else {
            suspense_boundary
                .flags
                .set(suspense_boundary.flags.get() | DID_CAPTURE);
            source_fiber
                .flags
                .set(source_fiber.flags.get() | FORCE_UPDATE_FOR_LEGACY_SUSPENSE);

            // We're going to commit this fiber even though it didn't complete.
            // But we shouldn't call any lifecycle methods or callbacks. Remove
            // all lifecycle effect tags.
            source_fiber
                .flags
                .set(source_fiber.flags.get() & !(LIFECYCLE_EFFECT_MASK | INCOMPLETE));

            if source_fiber.tag.get() == CLASS_COMPONENT {
                let current_source_fiber = source_fiber.alternate.borrow().clone();
                if current_source_fiber.is_none() {
                    // This is a new mount. Change the tag so it's not mistaken for a
                    // completed class component. For example, we should not call
                    // componentWillUnmount if it is deleted.
                    source_fiber.tag.set(INCOMPLETE_CLASS_COMPONENT);
                } else {
                    // When we try rendering again, we should not reuse the current fiber,
                    // since it's known to be in an inconsistent state. Use a force update to
                    // prevent a bail out.
                    let update = create_update(SYNC_LANE);
                    update.tag.set(FORCE_UPDATE);
                    enqueue_update(source_fiber, &update, SYNC_LANE);
                }
            } else if source_fiber.tag.get() == FUNCTION_COMPONENT {
                let current_source_fiber = source_fiber.alternate.borrow().clone();
                if current_source_fiber.is_none() {
                    // This is a new mount. Change the tag so it's not mistaken for a
                    // completed function component.
                    source_fiber.tag.set(INCOMPLETE_FUNCTION_COMPONENT);
                }
            }

            // The source fiber did not complete. Mark it with Sync priority to
            // indicate that it still has pending work.
            source_fiber
                .lanes
                .set(merge_lanes(source_fiber.lanes.get(), SYNC_LANE));
        }
        return Some(Rc::clone(suspense_boundary));
    }
    // Confirmed that the boundary is in a concurrent mode tree. Continue
    // with the normal suspend path.
    //
    // After this we'll use a set of heuristics to determine whether this
    // render pass will run to completion or restart or "suspend" the commit.
    // The actual logic for this is spread out in different places.
    //
    // This first principle is that if we're going to suspend when we complete
    // a root, then we should also restart if we get an update or ping that
    // might unsuspend it, and vice versa. The only reason to suspend is
    // because you think you might want to restart before committing. However,
    // it doesn't make sense to restart only while in the period we're suspended.
    //
    // Restarting too aggressively is also not good because it starves out any
    // intermediate loading state. So we use heuristics to determine when.

    // Suspense Heuristics
    //
    // If nothing threw a Promise or all the same fallbacks are already showing,
    // then don't suspend/restart.
    //
    // If this is an initial render of a new tree of Suspense boundaries and
    // those trigger a fallback, then don't suspend/restart. We want to ensure
    // that we can show the initial loading state as quickly as possible.
    //
    // If we hit a "Delayed" case, such as when we'd switch from content back into
    // a fallback, then we should always suspend/restart. Transitions apply
    // to this case. If none is defined, JND is used instead.
    //
    // If we're already showing a fallback and it gets "retried", allowing us to show
    // another level, but there's still an inner boundary that would show a fallback,
    // then we suspend/restart for 500ms since the last time we showed a fallback
    // anywhere in the tree. This effectively throttles progressive loading into a
    // consistent train of commits. This also gives us an opportunity to restart to
    // get to the completed state slightly earlier.
    //
    // If there's ambiguity due to batching it's resolved in preference of:
    // 1) "delayed", 2) "initial render", 3) "retry".
    //
    // We want to ensure that a "busy" state doesn't get force committed. We want to
    // ensure that new initial loading states can commit as soon as possible.
    suspense_boundary
        .flags
        .set(suspense_boundary.flags.get() | SHOULD_CAPTURE);
    // TODO: I think we can remove this, since we now use `DidCapture` in
    // the begin phase to prevent an early bailout.
    suspense_boundary.lanes.set(root_render_lanes);
    Some(Rc::clone(suspense_boundary))
}

pub fn throw_exception(
    root: &Rc<FiberRoot>,
    return_fiber: Option<&Rc<Fiber>>,
    source_fiber: &Rc<Fiber>,
    value: Option<&'static Unknown>,
    root_render_lanes: Lanes,
) -> bool {
    let mut value = value;
    // The source fiber did not complete.
    source_fiber
        .flags
        .set(source_fiber.flags.get() | INCOMPLETE);

    if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
        && IS_DEV_TOOLS_PRESENT.with(|&present| present)
    {
        // If we have pending work still, restore the original updaters
        restore_pending_updaters(root, root_render_lanes);
    }

    if let Some(object) = value
        && js::type_of(value) == "object"
        && js::type_of(js::get(object, "then")) == "function"
    {
        // This is a wakeable. The component suspended.
        let wakeable = object;
        reset_suspended_component(source_fiber, root_render_lanes);

        if __DEV__
            && get_is_hydrating()
            && (DISABLE_LEGACY_MODE || (source_fiber.mode.get() & CONCURRENT_MODE) != NO_MODE)
        {
            mark_did_throw_while_hydrating_dev();
        }

        // Mark the nearest Suspense boundary to switch to rendering a fallback.
        let suspense_boundary = get_suspense_handler();
        if let Some(suspense_boundary) = suspense_boundary {
            match suspense_boundary.tag.get() {
                ACTIVITY_COMPONENT | SUSPENSE_COMPONENT | SUSPENSE_LIST_COMPONENT => {
                    // If this suspense/activity boundary is not already showing a fallback, mark
                    // the in-progress render as suspended. We try to perform this logic
                    // as soon as soon as possible during the render phase, so the work
                    // loop can know things like whether it's OK to switch to other tasks,
                    // or whether it can wait for data to resolve before continuing.
                    // TODO: Most of these checks are already performed when entering a
                    // Suspense boundary. We should track the information on the stack so
                    // we don't have to recompute it on demand. This would also allow us
                    // to unify with `use` which needs to perform this logic even sooner,
                    // before `throwException` is called.
                    if DISABLE_LEGACY_MODE || (source_fiber.mode.get() & CONCURRENT_MODE) != NO_MODE
                    {
                        if get_shell_boundary().is_none() {
                            // Suspended in the "shell" of the app. This is an undesirable
                            // loading state. We should avoid committing this tree.
                            render_did_suspend_delay_if_possible();
                        } else {
                            // If we suspended deeper than the shell, we don't need to delay
                            // the commmit. However, we still call renderDidSuspend if this is
                            // a new boundary, to tell the work loop that a new fallback has
                            // appeared during this render.
                            // TODO: Theoretically we should be able to delete this branch.
                            // It's currently used for two things: 1) to throttle the
                            // appearance of successive loading states, and 2) in
                            // SuspenseList, to determine whether the children include any
                            // pending fallbacks. For 1, we should apply throttling to all
                            // retries, not just ones that render an additional fallback. For
                            // 2, we should check subtreeFlags instead. Then we can delete
                            // this branch.
                            let current = suspense_boundary.alternate.borrow().clone();
                            if current.is_none() {
                                render_did_suspend();
                            }
                        }
                    }

                    suspense_boundary
                        .flags
                        .set(suspense_boundary.flags.get() & !FORCE_CLIENT_RENDER);
                    mark_suspense_boundary_should_capture(
                        &suspense_boundary,
                        return_fiber,
                        source_fiber,
                        root,
                        root_render_lanes,
                    );
                    // Retry listener
                    //
                    // If the fallback does commit, we need to attach a different type of
                    // listener. This one schedules an update on the Suspense boundary to
                    // turn the fallback state off.
                    //
                    // Stash the wakeable on the boundary fiber so we can access it in the
                    // commit phase.
                    //
                    // When the wakeable resolves, we'll attempt to render the boundary
                    // again ("retry").

                    // Check if this is a Suspensey resource. We do not attach retry
                    // listeners to these, because we don't actually need them for
                    // rendering. Only for committing. Instead, if a fallback commits
                    // and the only thing that suspended was a Suspensey resource, we
                    // retry immediately.
                    // TODO: Refactor throwException so that we don't have to do this type
                    // check. The caller already knows what the cause was.
                    let is_suspensey_resource =
                        js::strict_equals(wakeable, &NOOP_SUSPENSEY_COMMIT_THENABLE);
                    if is_suspensey_resource {
                        suspense_boundary
                            .flags
                            .set(suspense_boundary.flags.get() | SCHEDULE_RETRY);
                    } else {
                        let retry_queue: Option<&RetryQueue> =
                            unsafe { js::cast(suspense_boundary.update_queue.get()) };
                        match retry_queue {
                            None => suspense_boundary
                                .update_queue
                                .set(Some(js::unknown(js::set::new(Some(&[wakeable]))))),
                            Some(retry_queue) => retry_queue.add(wakeable),
                        }

                        // We only attach ping listeners in concurrent mode. Legacy
                        // Suspense always commits fallbacks synchronously, so there are
                        // no pings.
                        if DISABLE_LEGACY_MODE
                            || (suspense_boundary.mode.get() & CONCURRENT_MODE) != NO_MODE
                        {
                            attach_ping_listener(root, wakeable, root_render_lanes);
                        }
                    }
                    return false;
                }
                OFFSCREEN_COMPONENT
                    if DISABLE_LEGACY_MODE
                        || (suspense_boundary.mode.get() & CONCURRENT_MODE) != NO_MODE =>
                {
                    suspense_boundary
                        .flags
                        .set(suspense_boundary.flags.get() | SHOULD_CAPTURE);
                    let is_suspensey_resource =
                        js::strict_equals(wakeable, &NOOP_SUSPENSEY_COMMIT_THENABLE);
                    if is_suspensey_resource {
                        suspense_boundary
                            .flags
                            .set(suspense_boundary.flags.get() | SCHEDULE_RETRY);
                    } else {
                        let offscreen_queue: Option<&OffscreenQueue> =
                            unsafe { js::cast(suspense_boundary.update_queue.get()) };
                        match offscreen_queue {
                            None => {
                                let new_offscreen_queue = OffscreenQueue {
                                    transitions: RefCell::new(None),
                                    marker_instances: RefCell::new(None),
                                    retry_queue: Cell::new(Some(js::set::new(Some(&[wakeable])))),
                                };
                                suspense_boundary
                                    .update_queue
                                    .set(Some(js::unknown_of(new_offscreen_queue)));
                            }
                            Some(offscreen_queue) => {
                                let retry_queue = offscreen_queue.retry_queue.get();
                                match retry_queue {
                                    None => offscreen_queue
                                        .retry_queue
                                        .set(Some(js::set::new(Some(&[wakeable])))),
                                    Some(retry_queue) => retry_queue.add(wakeable),
                                }
                            }
                        }

                        attach_ping_listener(root, wakeable, root_render_lanes);
                    }
                    return false;
                }
                _ => {}
            }
            panic!(
                "Unexpected Suspense handler tag ({}). This is a bug in React.",
                suspense_boundary.tag.get()
            );
        } else {
            // No boundary was found. Unless this is a sync update, this is OK.
            // We can suspend and wait for more data to arrive.

            if DISABLE_LEGACY_MODE || root.tag == CONCURRENT_ROOT {
                // In a concurrent root, suspending without a Suspense boundary is
                // allowed. It will suspend indefinitely without committing.
                //
                // TODO: Should we have different behavior for discrete updates? What
                // about flushSync? Maybe it should put the tree into an inert state,
                // and potentially log a warning. Revisit this for a future release.
                attach_ping_listener(root, wakeable, root_render_lanes);
                render_did_suspend_delay_if_possible();
                return false;
            } else {
                // In a legacy root, suspending without a boundary is always an error.
                let uncaught_suspense_error = js::error::new(concat!(
                    "A component suspended while responding to synchronous input. This ",
                    "will cause the UI to be replaced with a loading indicator. To ",
                    "fix, updates that suspend should be wrapped ",
                    "with startTransition."
                ));
                value = uncaught_suspense_error.as_unknown();
            }
        }
    }

    // This is a regular error, not a Suspense wakeable.
    if get_is_hydrating()
        && (DISABLE_LEGACY_MODE || (source_fiber.mode.get() & CONCURRENT_MODE) != NO_MODE)
    {
        mark_did_throw_while_hydrating_dev();
        let hydration_boundary = get_suspense_handler();
        // If the error was thrown during hydration, we may be able to recover by
        // discarding the dehydrated content and switching to a client render.
        // Instead of surfacing the error, find the nearest Suspense boundary
        // and render it again without hydration.
        if let Some(hydration_boundary) = hydration_boundary {
            if __DEV__ && hydration_boundary.tag.get() == SUSPENSE_LIST_COMPONENT {
                console::error((
                    "SuspenseList should never catch while hydrating. This is a bug in React.",
                ));
            }
            if (hydration_boundary.flags.get() & SHOULD_CAPTURE) == NO_FLAGS {
                // Set a flag to indicate that we should try rendering the normal
                // children again, not the fallback.
                hydration_boundary
                    .flags
                    .set(hydration_boundary.flags.get() | FORCE_CLIENT_RENDER);
            }
            mark_suspense_boundary_should_capture(
                &hydration_boundary,
                return_fiber,
                source_fiber,
                root,
                root_render_lanes,
            );

            // Even though the user may not be affected by this error, we should
            // still log it so it can be fixed.
            if !HYDRATION_MISMATCH_EXCEPTION.with(|exception| js::strict_equals(value, *exception))
            {
                let wrapper_error = js::new(
                    js::unknown(js::class::<Error>()),
                    (
                        concat!(
                            "There was an error while hydrating but React was able to recover by ",
                            "instead client rendering from the nearest Suspense boundary."
                        ),
                        &ErrorOptions { cause: value },
                    ),
                );
                queue_hydration_error(create_captured_value_at_fiber(
                    Some(wrapper_error),
                    source_fiber,
                ));
            }
            return false;
        } else {
            if !HYDRATION_MISMATCH_EXCEPTION.with(|exception| js::strict_equals(value, *exception))
            {
                let wrapper_error = js::new(
                    js::unknown(js::class::<Error>()),
                    (
                        concat!(
                            "There was an error while hydrating but React was able to recover by ",
                            "instead client rendering the entire root."
                        ),
                        &ErrorOptions { cause: value },
                    ),
                );
                queue_hydration_error(create_captured_value_at_fiber(
                    Some(wrapper_error),
                    source_fiber,
                ));
            }
            let current = root.current.borrow().clone();
            let work_in_progress = current.alternate.borrow().clone().unwrap();
            // Schedule an update at the root to log the error but this shouldn't
            // actually happen because we should recover.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | SHOULD_CAPTURE);
            let lane = pick_arbitrary_lane(root_render_lanes);
            work_in_progress
                .lanes
                .set(merge_lanes(work_in_progress.lanes.get(), lane));
            let root_error_info = create_captured_value_at_fiber(value, source_fiber);
            let update = create_root_error_update(
                &unsafe { js::cast(work_in_progress.state_node.get()) },
                &root_error_info, // This should never actually get logged due to the recovery.
                lane,
            );
            enqueue_captured_update(&work_in_progress, &update);
            render_did_error();
            return false;
        }
    } else {
        // Otherwise, fall through to the error path.
    }

    let wrapper_error = js::new(
        js::unknown(js::class::<Error>()),
        (
            concat!(
                "There was an error during concurrent rendering but React was able to recover by ",
                "instead synchronously rendering the entire root."
            ),
            &ErrorOptions { cause: value },
        ),
    );
    queue_concurrent_error(create_captured_value_at_fiber(
        Some(wrapper_error),
        source_fiber,
    ));
    render_did_error();

    // We didn't find a boundary that could handle this type of exception. Start
    // over and traverse parent path again, this time treating the exception
    // as an error.

    let Some(return_fiber) = return_fiber else {
        // There's no return fiber, which means the root errored. This should never
        // happen. Return `true` to trigger a fatal error (panic).
        return true;
    };

    let error_info = create_captured_value_at_fiber(value, source_fiber);
    let mut work_in_progress = Rc::clone(return_fiber);
    loop {
        match work_in_progress.tag.get() {
            HOST_ROOT => {
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | SHOULD_CAPTURE);
                let lane = pick_arbitrary_lane(root_render_lanes);
                work_in_progress
                    .lanes
                    .set(merge_lanes(work_in_progress.lanes.get(), lane));
                let update = create_root_error_update(
                    &unsafe { js::cast(work_in_progress.state_node.get()) },
                    &error_info,
                    lane,
                );
                enqueue_captured_update(&work_in_progress, &update);
                return false;
            }
            CLASS_COMPONENT => {
                // Capture and retry
                let ctor = work_in_progress.r#type.get();
                let instance = work_in_progress.state_node.get();
                if (work_in_progress.flags.get() & DID_CAPTURE) == NO_FLAGS
                    && (js::type_of(js::get(ctor, "getDerivedStateFromError")) == "function"
                        || instance.is_some_and(|instance| {
                            js::type_of(js::get(instance, "componentDidCatch")) == "function"
                                && !is_already_failed_legacy_error_boundary(instance)
                        }))
                {
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | SHOULD_CAPTURE);
                    let lane = pick_arbitrary_lane(root_render_lanes);
                    work_in_progress
                        .lanes
                        .set(merge_lanes(work_in_progress.lanes.get(), lane));
                    // Schedule the error boundary to re-render using updated state
                    let update = create_class_error_update(lane);
                    initialize_class_error_update(&update, root, &work_in_progress, &error_info);
                    enqueue_captured_update(&work_in_progress, &update);
                    return false;
                }
            }
            OFFSCREEN_COMPONENT => {
                let offscreen_state: Option<&OffscreenState> =
                    unsafe { js::cast(work_in_progress.memoized_state.get()) };
                if offscreen_state.is_some() {
                    // An error was thrown inside a hidden Offscreen boundary. This should
                    // not be allowed to escape into the visible part of the UI. Mark the
                    // boundary with ShouldCapture to abort the ongoing prerendering
                    // attempt. This is the same flag would be set if something were to
                    // suspend. It will be cleared the next time the boundary
                    // is attempted.
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | SHOULD_CAPTURE);
                    return false;
                }
            }
            _ => {}
        }
        let next = work_in_progress.r#return.borrow().clone();
        let Some(next) = next else {
            break;
        };
        work_in_progress = next;
    }

    false
}
