// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use shared::environment::__DEV__;

use crate::react_fiber_class_update_queue::{SharedQueue as ClassQueue, Update as ClassUpdate};
use crate::react_fiber_flags::{HYDRATING, NO_FLAGS, PLACEMENT};
use crate::react_fiber_hooks::{Update as HookUpdate, UpdateQueue as HookQueue};
use crate::react_fiber_lane::{Lane, Lanes, NO_LANE, NO_LANES, mark_hidden_update, merge_lanes};
use crate::react_fiber_offscreen_component::{OFFSCREEN_VISIBLE, OffscreenInstance};
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_work_tags::{HOST_ROOT, OFFSCREEN_COMPONENT};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#warnAboutUpdateOnNotYetMountedFiberInDEV"
)]
fn warn_about_update_on_not_yet_mounted_fiber_in_dev(fiber: &Fiber) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#throwIfInfiniteUpdateLoopDetected"
)]
fn throw_if_infinite_update_loop_detected(
    is_from_infinite_render_loop_detection_instrumentation: bool,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getWorkInProgressRoot"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_work_in_progress_root() -> Option<Rc<FiberRoot>> {
    unreachable!()
}

pub struct ConcurrentUpdate {
    pub next: RefCell<Rc<ConcurrentUpdate>>,
    pub lane: Cell<Lane>,
}

pub struct ConcurrentQueue {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending: RefCell<Option<Rc<ConcurrentUpdate>>>,
}

type ConcurrentQueueEntry = (
    Rc<Fiber>,
    Option<Rc<ConcurrentQueue>>,
    Option<Rc<ConcurrentUpdate>>,
    Lane,
);

thread_local! {
    // If a render is in progress, and we receive an update from a concurrent event,
    // we wait until the current render is over (either finished or interrupted)
    // before adding it to the fiber/hook queue. Push to this array so we can
    // access the queue, fiber, update, et al later.
    static CONCURRENT_QUEUES: RefCell<Vec<ConcurrentQueueEntry>> = const { RefCell::new(Vec::new()) };

    static CONCURRENTLY_UPDATED_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
}

pub fn finish_queueing_concurrent_updates() {
    let queued = CONCURRENT_QUEUES.with_borrow_mut(std::mem::take);

    CONCURRENTLY_UPDATED_LANES.set(NO_LANES);

    for (fiber, queue, update, lane) in queued {
        if let Some(queue) = &queue
            && let Some(update) = &update
        {
            let pending = queue.pending.borrow().clone();
            match pending {
                None => {
                    // This is the first update. Create a circular list.
                    update.next.replace(Rc::clone(update));
                }
                Some(pending) => {
                    let next = pending.next.borrow().clone();
                    update.next.replace(next);
                    pending.next.replace(Rc::clone(update));
                }
            }
            queue.pending.replace(Some(Rc::clone(update)));
        }

        if lane != NO_LANE {
            mark_update_lane_from_fiber_to_root(&fiber, update.as_ref(), lane);
        }
    }
}

pub fn get_concurrently_updated_lanes() -> Lanes {
    CONCURRENTLY_UPDATED_LANES.get()
}

#[cfg_attr(rust_js, rust_js::nullable(queue, update))]
fn enqueue_update(
    fiber: &Rc<Fiber>,
    queue: Option<&Rc<ConcurrentQueue>>,
    update: Option<&Rc<ConcurrentUpdate>>,
    lane: Lane,
) {
    // Don't update the `childLanes` on the return path yet. If we already in
    // the middle of rendering, wait until after it has completed.
    CONCURRENT_QUEUES.with_borrow_mut(|concurrent_queues| {
        concurrent_queues.push((Rc::clone(fiber), queue.cloned(), update.cloned(), lane));
    });

    CONCURRENTLY_UPDATED_LANES.set(merge_lanes(CONCURRENTLY_UPDATED_LANES.get(), lane));

    // The fiber's `lane` field is used in some places to check if any work is
    // scheduled, to perform an eager bailout, so we need to update it immediately.
    // TODO: We should probably move this to the "shared" queue instead.
    fiber.lanes.set(merge_lanes(fiber.lanes.get(), lane));
    let alternate = fiber.alternate.borrow().clone();
    if let Some(alternate) = alternate {
        alternate
            .lanes
            .set(merge_lanes(alternate.lanes.get(), lane));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn enqueue_concurrent_hook_update(
    fiber: &Rc<Fiber>,
    queue: &Rc<HookQueue>,
    update: &Rc<HookUpdate>,
    lane: Lane,
) -> Option<Rc<FiberRoot>> {
    let concurrent_queue: Rc<ConcurrentQueue> =
        unsafe { js::cast(Some(js::unknown_of(Rc::clone(queue)))) };
    let concurrent_update: Rc<ConcurrentUpdate> =
        unsafe { js::cast(Some(js::unknown_of(Rc::clone(update)))) };
    enqueue_update(
        fiber,
        Some(&concurrent_queue),
        Some(&concurrent_update),
        lane,
    );
    get_root_for_updated_fiber(fiber)
}

pub fn enqueue_concurrent_hook_update_and_eagerly_bailout(
    fiber: &Rc<Fiber>,
    queue: &Rc<HookQueue>,
    update: &Rc<HookUpdate>,
) {
    // This function is used to queue an update that doesn't need a rerender. The
    // only reason we queue it is in case there's a subsequent higher priority
    // update that causes it to be rebased.
    let lane = NO_LANE;
    let concurrent_queue: Rc<ConcurrentQueue> =
        unsafe { js::cast(Some(js::unknown_of(Rc::clone(queue)))) };
    let concurrent_update: Rc<ConcurrentUpdate> =
        unsafe { js::cast(Some(js::unknown_of(Rc::clone(update)))) };
    enqueue_update(
        fiber,
        Some(&concurrent_queue),
        Some(&concurrent_update),
        lane,
    );

    // Usually we can rely on the upcoming render phase to process the concurrent
    // queue. However, since this is a bail out, we're not scheduling any work
    // here. So the update we just queued will leak until something else happens
    // to schedule work (if ever).
    //
    // Check if we're currently in the middle of rendering a tree, and if not,
    // process the queue immediately to prevent a leak.
    let is_concurrently_rendering = get_work_in_progress_root().is_some();
    if !is_concurrently_rendering {
        finish_queueing_concurrent_updates();
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn enqueue_concurrent_class_update(
    fiber: &Rc<Fiber>,
    queue: &Rc<ClassQueue>,
    update: &Rc<ClassUpdate>,
    lane: Lane,
) -> Option<Rc<FiberRoot>> {
    let concurrent_queue: Rc<ConcurrentQueue> =
        unsafe { js::cast(Some(js::unknown_of(Rc::clone(queue)))) };
    let concurrent_update: Rc<ConcurrentUpdate> =
        unsafe { js::cast(Some(js::unknown_of(Rc::clone(update)))) };
    enqueue_update(
        fiber,
        Some(&concurrent_queue),
        Some(&concurrent_update),
        lane,
    );
    get_root_for_updated_fiber(fiber)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn enqueue_concurrent_render_for_lane(fiber: &Rc<Fiber>, lane: Lane) -> Option<Rc<FiberRoot>> {
    enqueue_update(fiber, None, None, lane);
    get_root_for_updated_fiber(fiber)
}

// Calling this function outside this module should only be done for backwards
// compatibility and should always be accompanied by a warning.
#[cfg_attr(rust_js, rust_js::name = "unsafe_markUpdateLaneFromFiberToRoot")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn unsafe_mark_update_lane_from_fiber_to_root(
    source_fiber: &Rc<Fiber>,
    lane: Lane,
) -> Option<Rc<FiberRoot>> {
    // NOTE: For Hyrum's Law reasons, if an infinite update loop is detected, it
    // should throw before `markUpdateLaneFromFiberToRoot` is called. But this is
    // undefined behavior and we can change it if we need to; it just so happens
    // that, at the time of this writing, there's an internal product test that
    // happens to rely on this.
    let root = get_root_for_updated_fiber(source_fiber);
    mark_update_lane_from_fiber_to_root(source_fiber, None, lane);
    root
}

#[cfg_attr(rust_js, rust_js::nullable, rust_js::nullable(update))]
fn mark_update_lane_from_fiber_to_root(
    source_fiber: &Rc<Fiber>,
    update: Option<&Rc<ConcurrentUpdate>>,
    lane: Lane,
) -> Option<Rc<FiberRoot>> {
    // Update the source fiber's lanes
    source_fiber
        .lanes
        .set(merge_lanes(source_fiber.lanes.get(), lane));
    let mut alternate = source_fiber.alternate.borrow().clone();
    if let Some(alternate) = &alternate {
        alternate
            .lanes
            .set(merge_lanes(alternate.lanes.get(), lane));
    }
    // Walk the parent path to the root and update the child lanes.
    let mut is_hidden = false;
    let mut parent = source_fiber.r#return.borrow().clone();
    let mut node = Rc::clone(source_fiber);
    while let Some(parent_fiber) = parent {
        parent_fiber
            .child_lanes
            .set(merge_lanes(parent_fiber.child_lanes.get(), lane));
        alternate = parent_fiber.alternate.borrow().clone();
        if let Some(alternate) = &alternate {
            alternate
                .child_lanes
                .set(merge_lanes(alternate.child_lanes.get(), lane));
        }

        if parent_fiber.tag.get() == OFFSCREEN_COMPONENT {
            // Check if this offscreen boundary is currently hidden.
            //
            // The instance may be null if the Offscreen parent was unmounted. Usually
            // the parent wouldn't be reachable in that case because we disconnect
            // fibers from the tree when they are deleted. However, there's a weird
            // edge case where setState is called on a fiber that was interrupted
            // before it ever mounted. Because it never mounts, it also never gets
            // deleted. Because it never gets deleted, its return pointer never gets
            // disconnected. Which means it may be attached to a deleted Offscreen
            // parent node. (This discovery suggests it may be better for memory usage
            // if we don't attach the `return` pointer until the commit phase, though
            // in order to do that we'd need some other way to track the return
            // pointer during the initial render, like on the stack.)
            //
            // This case is always accompanied by a warning, but we still need to
            // account for it. (There may be other cases that we haven't discovered,
            // too.)
            let offscreen_instance: Option<&OffscreenInstance> =
                unsafe { js::cast(parent_fiber.state_node.get()) };
            if let Some(offscreen_instance) = offscreen_instance
                && (offscreen_instance.visibility.get() & OFFSCREEN_VISIBLE) == 0
            {
                is_hidden = true;
            }
        }

        node = parent_fiber;
        parent = node.r#return.borrow().clone();
    }

    if node.tag.get() == HOST_ROOT {
        let root: Rc<FiberRoot> = unsafe { js::cast(node.state_node.get()) };
        if is_hidden && let Some(update) = update {
            mark_hidden_update(&root, update, lane);
        }
        return Some(root);
    }
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_root_for_updated_fiber(source_fiber: &Rc<Fiber>) -> Option<Rc<FiberRoot>> {
    // TODO: We will detect and infinite update loop and throw even if this fiber
    // has already unmounted. This isn't really necessary but it happens to be the
    // current behavior we've used for several release cycles. Consider not
    // performing this check if the updated fiber already unmounted, since it's
    // not possible for that to cause an infinite update loop.
    throw_if_infinite_update_loop_detected(false);

    // When a setState happens, we must ensure the root is scheduled. Because
    // update queues do not have a backpointer to the root, the only way to do
    // this currently is to walk up the return path. This used to not be a big
    // deal because we would have to walk up the return path to set
    // the `childLanes`, anyway, but now those two traversals happen at
    // different times.
    // TODO: Consider adding a `root` backpointer on the update queue.
    detect_update_on_unmounted_fiber(source_fiber, source_fiber);
    let mut node = Rc::clone(source_fiber);
    let mut parent = node.r#return.borrow().clone();
    while let Some(parent_fiber) = parent {
        detect_update_on_unmounted_fiber(source_fiber, &node);
        node = parent_fiber;
        parent = node.r#return.borrow().clone();
    }
    if node.tag.get() == HOST_ROOT {
        Some(unsafe { js::cast(node.state_node.get()) })
    } else {
        None
    }
}

fn detect_update_on_unmounted_fiber(source_fiber: &Fiber, parent: &Fiber) {
    if __DEV__ {
        let alternate = parent.alternate.borrow().clone();
        if alternate.is_none() && (parent.flags.get() & (PLACEMENT | HYDRATING)) != NO_FLAGS {
            warn_about_update_on_not_yet_mounted_fiber_in_dev(source_fiber);
        }
    }
}
