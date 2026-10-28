// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// UpdateQueue is a linked list of prioritized updates.
//
// Like fibers, update queues come in pairs: a current queue, which represents
// the visible state of the screen, and a work-in-progress queue, which can be
// mutated and processed asynchronously before it is committed — a form of
// double buffering. If a work-in-progress render is discarded before finishing,
// we create a new work-in-progress by cloning the current queue.
//
// Both queues share a persistent, singly-linked list structure. To schedule an
// update, we append it to the end of both queues. Each queue maintains a
// pointer to first update in the persistent list that hasn't been processed.
// The work-in-progress pointer always has a position equal to or greater than
// the current queue, since we always work on that one. The current queue's
// pointer is only updated during the commit phase, when we swap in the
// work-in-progress.
//
// For example:
//
//   Current pointer:           A - B - C - D - E - F
//   Work-in-progress pointer:              D - E - F
//                                          ^
//                                          The work-in-progress queue has
//                                          processed more updates than current.
//
// The reason we append to both queues is because otherwise we might drop
// updates without ever processing them. For example, if we only add updates to
// the work-in-progress queue, some updates could be lost whenever a work-in
// -progress render restarts by cloning from current. Similarly, if we only add
// updates to the current queue, the updates will be lost whenever an already
// in-progress queue commits and swaps with the current queue. However, by
// adding to both queues, we guarantee that the update will be part of the next
// work-in-progress. (And because the work-in-progress queue becomes the
// current queue once it commits, there's no danger of applying the same
// update twice.)
//
// Prioritization
// --------------
//
// Updates are not sorted by priority, but by insertion; new updates are always
// appended to the end of the list.
//
// The priority is still important, though. When processing the update queue
// during the render phase, only the updates with sufficient priority are
// included in the result. If we skip an update because it has insufficient
// priority, it remains in the queue to be processed later, during a lower
// priority render. Crucially, all updates subsequent to a skipped update also
// remain in the queue *regardless of their priority*. That means high priority
// updates are sometimes processed twice, at two separate priorities. We also
// keep track of a base state, that represents the state before the first
// update in the queue is applied.
//
// For example:
//
//   Given a base state of '', and the following queue of updates
//
//     A1 - B2 - C1 - D2
//
//   where the number indicates the priority, and the update is applied to the
//   previous state by appending a letter, React will process these updates as
//   two separate renders, one per distinct priority level:
//
//   First render, at priority 1:
//     Base state: ''
//     Updates: [A1, C1]
//     Result state: 'AC'
//
//   Second render, at priority 2:
//     Base state: 'A'            <-  The base state does not include C1,
//                                    because B2 was skipped.
//     Updates: [B2, C1, D2]      <-  C1 was rebased on top of B2
//     Result state: 'ABCD'
//
// Because we process updates in insertion order, and rebase high priority
// updates when preceding updates are skipped, the final result is deterministic
// regardless of priority. Intermediate state may vary according to system
// resources, but the final state is always the same.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Kind, Unknown};
use shared::assign::assign;
use shared::environment::__DEV__;
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_fiber_async_action::{peek_entangled_action_lane, peek_entangled_action_thenable};
use crate::react_fiber_concurrent_updates::{
    enqueue_concurrent_class_update, unsafe_mark_update_lane_from_fiber_to_root,
};
use crate::react_fiber_dev_tools_hook::set_is_strict_mode_for_devtools;
use crate::react_fiber_flags::{CALLBACK, DID_CAPTURE, SHOULD_CAPTURE, VISIBILITY};
use crate::react_fiber_lane::{
    Lane, Lanes, NO_LANE, NO_LANES, OFFSCREEN_LANE, intersect_lanes, is_subset_of_lanes,
    is_transition_lane, mark_root_entangled, merge_lanes, remove_lanes,
};
use crate::react_fiber_new_context::{
    enter_disallowed_context_read_in_dev, exit_disallowed_context_read_in_dev,
};
use crate::react_fiber_work_loop::{
    get_work_in_progress_root_render_lanes, is_unsafe_class_render_phase_update,
    mark_skipped_update_lanes,
};
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_type_of_mode::STRICT_LEGACY_MODE;

pub type UpdateTag = u8;

/// The callbacks an update queue calls when it commits: what each update's
/// `callback` was, which JS may have given as any value.
pub type Callbacks = Rc<RefCell<Vec<&'static Unknown>>>;

pub struct Update {
    pub lane: Cell<Lane>,

    pub tag: Cell<UpdateTag>,
    pub payload: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub callback: Cell<Option<&'static Unknown>>,

    #[cfg_attr(rust_js, rust_js::nullable)]
    pub next: RefCell<Option<Rc<Update>>>,
}

pub struct SharedQueue {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub pending: RefCell<Option<Rc<Update>>>,
    pub lanes: Cell<Lanes>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub hidden_callbacks: RefCell<Option<Callbacks>>,
}

pub struct UpdateQueue {
    pub base_state: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub first_base_update: RefCell<Option<Rc<Update>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub last_base_update: RefCell<Option<Rc<Update>>>,
    pub shared: Rc<SharedQueue>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub callbacks: RefCell<Option<Callbacks>>,
}

// Safety: each is a JS object: an update, a queue's shared part, and a
// fiber's `updateQueue`.
unsafe impl js::Defined for Update {}
unsafe impl js::Defined for SharedQueue {}
unsafe impl js::Defined for UpdateQueue {}

#[cfg_attr(rust_js, rust_js::name = "UpdateState")]
pub const UPDATE_STATE: UpdateTag = 0;
#[cfg_attr(rust_js, rust_js::name = "ReplaceState")]
pub const REPLACE_STATE: UpdateTag = 1;
#[cfg_attr(rust_js, rust_js::name = "ForceUpdate")]
pub const FORCE_UPDATE: UpdateTag = 2;
#[cfg_attr(rust_js, rust_js::name = "CaptureUpdate")]
pub const CAPTURE_UPDATE: UpdateTag = 3;

thread_local! {
    // Global state that is reset at the beginning of calling `processUpdateQueue`.
    // It should only be read right after calling `processUpdateQueue`, via
    // `checkHasForceUpdateAfterProcessing`.
    static HAS_FORCE_UPDATE: Cell<bool> = const { Cell::new(false) };

    static DID_WARN_UPDATE_INSIDE_UPDATE: Cell<bool> = const { Cell::new(false) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_PROCESSING_QUEUE: RefCell<Option<Rc<SharedQueue>>> = const { RefCell::new(None) };
}

pub fn reset_currently_processing_queue() {
    CURRENTLY_PROCESSING_QUEUE.set(None);
}

pub fn initialize_update_queue(fiber: &Fiber) {
    let queue = Rc::new(UpdateQueue {
        base_state: Cell::new(fiber.memoized_state.get()),
        first_base_update: RefCell::new(None),
        last_base_update: RefCell::new(None),
        shared: Rc::new(SharedQueue {
            pending: RefCell::new(None),
            lanes: Cell::new(NO_LANES),
            hidden_callbacks: RefCell::new(None),
        }),
        callbacks: RefCell::new(None),
    });
    fiber.update_queue.set(Some(js::unknown_of(queue)));
}

pub fn clone_update_queue(current: &Fiber, work_in_progress: &Fiber) {
    // Clone the update queue from current. Unless it's already a clone.
    let queue: &UpdateQueue = unsafe { js::cast(work_in_progress.update_queue.get()) };
    let current_queue: &UpdateQueue = unsafe { js::cast(current.update_queue.get()) };
    if std::ptr::eq(queue, current_queue) {
        let clone = Rc::new(UpdateQueue {
            base_state: Cell::new(current_queue.base_state.get()),
            first_base_update: RefCell::new(current_queue.first_base_update.borrow().clone()),
            last_base_update: RefCell::new(current_queue.last_base_update.borrow().clone()),
            shared: Rc::clone(&current_queue.shared),
            callbacks: RefCell::new(None),
        });
        work_in_progress
            .update_queue
            .set(Some(js::unknown_of(clone)));
    }
}

pub fn create_update(lane: Lane) -> Rc<Update> {
    Rc::new(Update {
        lane: Cell::new(lane),

        tag: Cell::new(UPDATE_STATE),
        payload: Cell::new(js::null()),
        callback: Cell::new(None),

        next: RefCell::new(None),
    })
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn enqueue_update(fiber: &Rc<Fiber>, update: &Rc<Update>, lane: Lane) -> Option<Rc<FiberRoot>> {
    let update_queue: Option<&UpdateQueue> = unsafe { js::cast(fiber.update_queue.get()) };
    let Some(update_queue) = update_queue else {
        // Only occurs if the fiber has been unmounted.
        return None;
    };

    let shared_queue = &update_queue.shared;

    if __DEV__
        && CURRENTLY_PROCESSING_QUEUE.with_borrow(|queue| {
            queue
                .as_ref()
                .is_some_and(|queue| Rc::ptr_eq(queue, shared_queue))
        })
        && !DID_WARN_UPDATE_INSIDE_UPDATE.get()
    {
        let component_name = get_component_name_from_fiber(fiber);
        console::error((
            concat!(
                "An update (setState, replaceState, or forceUpdate) was scheduled ",
                "from inside an update function. Update functions should be pure, ",
                "with zero side-effects. Consider using componentDidUpdate or a ",
                "callback.\n\nPlease update the following component: %s"
            ),
            component_name,
        ));
        DID_WARN_UPDATE_INSIDE_UPDATE.set(true);
    }

    if is_unsafe_class_render_phase_update(fiber) {
        // This is an unsafe render phase update. Add directly to the update
        // queue so we can process it immediately during the current render.
        let pending = shared_queue.pending.borrow().clone();
        match pending {
            None => {
                // This is the first update. Create a circular list.
                update.next.replace(Some(Rc::clone(update)));
            }
            Some(pending) => {
                let next = pending.next.borrow().clone();
                update.next.replace(next);
                pending.next.replace(Some(Rc::clone(update)));
            }
        }
        shared_queue.pending.replace(Some(Rc::clone(update)));

        // Update the childLanes even though we're most likely already rendering
        // this fiber. This is for backwards compatibility in the case where you
        // update a different component during render phase than the one that is
        // currently renderings (a pattern that is accompanied by a warning).
        unsafe_mark_update_lane_from_fiber_to_root(fiber, lane)
    } else {
        enqueue_concurrent_class_update(fiber, shared_queue, update, lane)
    }
}

pub fn entangle_transitions(root: &FiberRoot, fiber: &Fiber, lane: Lane) {
    let update_queue: Option<&UpdateQueue> = unsafe { js::cast(fiber.update_queue.get()) };
    let Some(update_queue) = update_queue else {
        // Only occurs if the fiber has been unmounted.
        return;
    };

    let shared_queue = &update_queue.shared;
    if is_transition_lane(lane) {
        let mut queue_lanes = shared_queue.lanes.get();

        // If any entangled lanes are no longer pending on the root, then they must
        // have finished. We can remove them from the shared queue, which represents
        // a superset of the actually pending lanes. In some cases we may entangle
        // more than we need to, but that's OK. In fact it's worse if we *don't*
        // entangle when we should.
        queue_lanes = intersect_lanes(queue_lanes, root.pending_lanes.get());

        // Entangle the new transition lane with the other transition lanes.
        let new_queue_lanes = merge_lanes(queue_lanes, lane);
        shared_queue.lanes.set(new_queue_lanes);
        // Even if queue.lanes already include lane, we don't know for certain if
        // the lane finished since the last time we entangled it. So we need to
        // entangle it again, just to be sure.
        mark_root_entangled(root, new_queue_lanes);
    }
}

pub fn enqueue_captured_update(work_in_progress: &Fiber, captured_update: &Rc<Update>) {
    // Captured updates are updates that are thrown by a child during the render
    // phase. They should be discarded if the render is aborted. Therefore,
    // we should only put them on the work-in-progress queue, not the current one.
    let queue: &UpdateQueue = unsafe { js::cast(work_in_progress.update_queue.get()) };

    // Check if the work-in-progress queue is a clone.
    let current = work_in_progress.alternate.borrow().clone();
    if let Some(current) = current {
        let current_queue: &UpdateQueue = unsafe { js::cast(current.update_queue.get()) };
        if std::ptr::eq(queue, current_queue) {
            // The work-in-progress queue is the same as current. This happens when
            // we bail out on a parent fiber that then captures an error thrown by
            // a child. Since we want to append the update only to the work-in
            // -progress queue, we need to clone the updates. We usually clone during
            // processUpdateQueue, but that didn't happen in this case because we
            // skipped over the parent when we bailed out.
            let mut new_first = None;
            let mut new_last: Option<Rc<Update>> = None;
            let first_base_update = queue.first_base_update.borrow().clone();
            if let Some(first_base_update) = first_base_update {
                // Loop through the updates and clone them.
                let mut update = first_base_update;
                loop {
                    let clone = Rc::new(Update {
                        lane: Cell::new(update.lane.get()),

                        tag: Cell::new(update.tag.get()),
                        payload: Cell::new(update.payload.get()),
                        // When this update is rebased, we should not fire its
                        // callback again.
                        callback: Cell::new(None),

                        next: RefCell::new(None),
                    });
                    match &new_last {
                        None => {
                            new_first = Some(Rc::clone(&clone));
                            new_last = Some(clone);
                        }
                        Some(last) => {
                            last.next.replace(Some(Rc::clone(&clone)));
                            new_last = Some(clone);
                        }
                    }
                    let next = update.next.borrow().clone();
                    match next {
                        Some(next) => update = next,
                        None => break,
                    }
                }

                // Append the captured update the end of the cloned list.
                match &new_last {
                    None => {
                        new_first = Some(Rc::clone(captured_update));
                        new_last = Some(Rc::clone(captured_update));
                    }
                    Some(last) => {
                        last.next.replace(Some(Rc::clone(captured_update)));
                        new_last = Some(Rc::clone(captured_update));
                    }
                }
            } else {
                // There are no base updates.
                new_first = Some(Rc::clone(captured_update));
                new_last = Some(Rc::clone(captured_update));
            }
            let queue = Rc::new(UpdateQueue {
                base_state: Cell::new(current_queue.base_state.get()),
                first_base_update: RefCell::new(new_first),
                last_base_update: RefCell::new(new_last),
                shared: Rc::clone(&current_queue.shared),
                callbacks: RefCell::new(current_queue.callbacks.borrow().clone()),
            });
            work_in_progress
                .update_queue
                .set(Some(js::unknown_of(queue)));
            return;
        }
    }

    // Append the update to the end of the list.
    let last_base_update = queue.last_base_update.borrow().clone();
    match last_base_update {
        None => {
            queue
                .first_base_update
                .replace(Some(Rc::clone(captured_update)));
        }
        Some(last_base_update) => {
            last_base_update
                .next
                .replace(Some(Rc::clone(captured_update)));
        }
    }
    queue
        .last_base_update
        .replace(Some(Rc::clone(captured_update)));
}

struct ExitStrictModeForDevtools;

impl Drop for ExitStrictModeForDevtools {
    fn drop(&mut self) {
        set_is_strict_mode_for_devtools(false);
    }
}

fn get_state_from_update(
    work_in_progress: &Fiber,
    _queue: &UpdateQueue,
    update: &Update,
    prev_state: Option<&'static Unknown>,
    next_props: Option<&'static Unknown>,
    instance: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    match update.tag.get() {
        REPLACE_STATE => {
            let payload = update.payload.get();
            if let Kind::Function(payload) = js::classify(payload) {
                // Updater function
                if __DEV__ {
                    enter_disallowed_context_read_in_dev();
                }
                let next_state =
                    js::function::call(payload, Some(&instance), (prev_state, next_props));
                if __DEV__ {
                    if (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
                        set_is_strict_mode_for_devtools(true);
                        let _exit = ExitStrictModeForDevtools;
                        js::function::call(payload, Some(&instance), (prev_state, next_props));
                    }
                    exit_disallowed_context_read_in_dev();
                }
                return next_state;
            }
            // State object
            payload
        }
        CAPTURE_UPDATE | UPDATE_STATE => {
            if update.tag.get() == CAPTURE_UPDATE {
                work_in_progress
                    .flags
                    .set((work_in_progress.flags.get() & !SHOULD_CAPTURE) | DID_CAPTURE);
            }
            let payload = update.payload.get();
            let partial_state;
            if let Kind::Function(payload) = js::classify(payload) {
                // Updater function
                if __DEV__ {
                    enter_disallowed_context_read_in_dev();
                }
                partial_state =
                    js::function::call(payload, Some(&instance), (prev_state, next_props));
                if __DEV__ {
                    if (work_in_progress.mode.get() & STRICT_LEGACY_MODE) != 0 {
                        set_is_strict_mode_for_devtools(true);
                        let _exit = ExitStrictModeForDevtools;
                        js::function::call(payload, Some(&instance), (prev_state, next_props));
                    }
                    exit_disallowed_context_read_in_dev();
                }
            } else {
                // Partial state object
                partial_state = payload;
            }
            if partial_state.is_none() {
                // Null and undefined are treated as no-ops.
                return prev_state;
            }
            // Merge the partial state and the previous state.
            Some(assign(
                js::unknown(js::dict::new::<()>()),
                (prev_state, partial_state),
            ))
        }
        FORCE_UPDATE => {
            HAS_FORCE_UPDATE.set(true);
            prev_state
        }
        _ => prev_state,
    }
}

thread_local! {
    static DID_READ_FROM_ENTANGLED_ASYNC_ACTION: Cell<bool> = const { Cell::new(false) };
}

// Each call to processUpdateQueue should be accompanied by a call to this. It's
// only in a separate function because in updateHostRoot, it must happen after
// all the context stacks have been pushed to, to prevent a stack mismatch. A
// bit unfortunate.
pub fn suspend_if_update_read_from_entangled_async_action() {
    // Check if this update is part of a pending async action. If so, we'll
    // need to suspend until the action has finished, so that it's batched
    // together with future updates in the same action.
    // TODO: Once we support hooks inside useMemo (or an equivalent
    // memoization boundary like Forget), hoist this logic so that it only
    // suspends if the memo boundary produces a new value.
    if DID_READ_FROM_ENTANGLED_ASYNC_ACTION.get() {
        let entangled_action_thenable = peek_entangled_action_thenable();
        if let Some(entangled_action_thenable) = entangled_action_thenable {
            // TODO: Instead of the throwing the thenable directly, throw a
            // special object like `use` does so we can detect if it's captured
            // by userspace.
            js::throw(Some(js::unknown(entangled_action_thenable)));
        }
    }
}

pub fn process_update_queue(
    work_in_progress: &Fiber,
    props: Option<&'static Unknown>,
    instance: Option<&'static Unknown>,
    render_lanes: Lanes,
) {
    DID_READ_FROM_ENTANGLED_ASYNC_ACTION.set(false);

    // This is always non-null on a ClassComponent or HostRoot
    let queue: &UpdateQueue = unsafe { js::cast(work_in_progress.update_queue.get()) };

    HAS_FORCE_UPDATE.set(false);

    if __DEV__ {
        CURRENTLY_PROCESSING_QUEUE.set(Some(Rc::clone(&queue.shared)));
    }

    let mut first_base_update = queue.first_base_update.borrow().clone();
    let mut last_base_update = queue.last_base_update.borrow().clone();

    // Check if there are pending updates. If so, transfer them to the base queue.
    let mut pending_queue = queue.shared.pending.borrow().clone();
    if let Some(last_pending_update) = pending_queue {
        queue.shared.pending.replace(None);

        // The pending queue is circular. Disconnect the pointer between first
        // and last so that it's non-circular.
        let first_pending_update = last_pending_update.next.take();
        // Append pending updates to base queue
        match &last_base_update {
            None => first_base_update = first_pending_update.clone(),
            Some(last_base_update) => {
                last_base_update.next.replace(first_pending_update.clone());
            }
        }
        last_base_update = Some(Rc::clone(&last_pending_update));

        // If there's a current queue, and it's different from the base queue, then
        // we need to transfer the updates to that queue, too. Because the base
        // queue is a singly-linked list with no cycles, we can append to both
        // lists and take advantage of structural sharing.
        // TODO: Pass `current` as argument
        let current = work_in_progress.alternate.borrow().clone();
        if let Some(current) = current {
            // This is always non-null on a ClassComponent or HostRoot
            let current_queue: &UpdateQueue = unsafe { js::cast(current.update_queue.get()) };
            let current_last_base_update = current_queue.last_base_update.borrow().clone();
            let is_same_update = match (&current_last_base_update, &last_base_update) {
                (Some(current_last_base_update), Some(last_base_update)) => {
                    Rc::ptr_eq(current_last_base_update, last_base_update)
                }
                (current_last_base_update, last_base_update) => {
                    current_last_base_update.is_none() && last_base_update.is_none()
                }
            };
            if !is_same_update {
                match current_last_base_update {
                    None => {
                        current_queue
                            .first_base_update
                            .replace(first_pending_update);
                    }
                    Some(current_last_base_update) => {
                        current_last_base_update.next.replace(first_pending_update);
                    }
                }
                current_queue
                    .last_base_update
                    .replace(Some(last_pending_update));
            }
        }
    }

    // These values may change as we process the queue.
    if let Some(first) = &first_base_update {
        // Iterate through the list of updates to compute the result.
        let mut new_state = queue.base_state.get();
        // TODO: Don't need to accumulate this. Instead, we can remove renderLanes
        // from the original lanes.
        let mut new_lanes: Lanes = NO_LANES;

        let mut new_base_state = None;
        let mut new_first_base_update = None;
        let mut new_last_base_update: Option<Rc<Update>> = None;

        let mut update = Rc::clone(first);
        loop {
            // An extra OffscreenLane bit is added to updates that were made to
            // a hidden tree, so that we can distinguish them from updates that were
            // already there when the tree was hidden.
            let update_lane = remove_lanes(update.lane.get(), OFFSCREEN_LANE);
            let is_hidden_update = update_lane != update.lane.get();

            // Check if this update was made while the tree was hidden. If so, then
            // it's not a "base" update and we should disregard the extra base lanes
            // that were added to renderLanes when we entered the Offscreen tree.
            let should_skip_update = if is_hidden_update {
                !is_subset_of_lanes(get_work_in_progress_root_render_lanes(), update_lane)
            } else {
                !is_subset_of_lanes(render_lanes, update_lane)
            };

            if should_skip_update {
                // Priority is insufficient. Skip this update. If this is the first
                // skipped update, the previous update/state is the new base
                // update/state.
                let clone = Rc::new(Update {
                    lane: Cell::new(update_lane),

                    tag: Cell::new(update.tag.get()),
                    payload: Cell::new(update.payload.get()),
                    callback: Cell::new(update.callback.get()),

                    next: RefCell::new(None),
                });
                match &new_last_base_update {
                    None => {
                        new_first_base_update = Some(Rc::clone(&clone));
                        new_last_base_update = Some(clone);
                        new_base_state = new_state;
                    }
                    Some(last) => {
                        last.next.replace(Some(Rc::clone(&clone)));
                        new_last_base_update = Some(clone);
                    }
                }
                // Update the remaining priority in the queue.
                new_lanes = merge_lanes(new_lanes, update_lane);
            } else {
                // This update does have sufficient priority.

                // Check if this update is part of a pending async action. If so,
                // we'll need to suspend until the action has finished, so that it's
                // batched together with future updates in the same action.
                if update_lane != NO_LANE && update_lane == peek_entangled_action_lane() {
                    DID_READ_FROM_ENTANGLED_ASYNC_ACTION.set(true);
                }

                if let Some(last) = &new_last_base_update {
                    let clone = Rc::new(Update {
                        // This update is going to be committed so we never want uncommit
                        // it. Using NoLane works because 0 is a subset of all bitmasks, so
                        // this will never be skipped by the check above.
                        lane: Cell::new(NO_LANE),

                        tag: Cell::new(update.tag.get()),
                        payload: Cell::new(update.payload.get()),

                        // When this update is rebased, we should not fire its
                        // callback again.
                        callback: Cell::new(None),

                        next: RefCell::new(None),
                    });
                    last.next.replace(Some(Rc::clone(&clone)));
                    new_last_base_update = Some(clone);
                }

                // Process this update.
                new_state = get_state_from_update(
                    work_in_progress,
                    queue,
                    &update,
                    new_state,
                    props,
                    instance,
                );
                let callback = update.callback.get();
                if let Some(callback) = callback {
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | CALLBACK);
                    if is_hidden_update {
                        work_in_progress
                            .flags
                            .set(work_in_progress.flags.get() | VISIBILITY);
                    }
                    let callbacks = queue.callbacks.borrow().clone();
                    match callbacks {
                        None => {
                            queue
                                .callbacks
                                .replace(Some(Rc::new(RefCell::new(vec![callback]))));
                        }
                        Some(callbacks) => callbacks.borrow_mut().push(callback),
                    }
                }
            }
            let next = update.next.borrow().clone();
            match next {
                Some(next) => update = next,
                None => {
                    pending_queue = queue.shared.pending.borrow().clone();
                    match pending_queue {
                        None => break,
                        Some(last_pending_update) => {
                            // An update was scheduled from inside a reducer. Add the new
                            // pending updates to the end of the list and keep processing.
                            // Intentionally unsound. Pending updates form a circular list, but we
                            // unravel them when transferring them to the base queue.
                            let first_pending_update = last_pending_update
                                .next
                                .take()
                                .expect("a pending update in a circular list");
                            update = first_pending_update;
                            queue.last_base_update.replace(Some(last_pending_update));
                            queue.shared.pending.replace(None);
                        }
                    }
                }
            }
        }

        if new_last_base_update.is_none() {
            new_base_state = new_state;
        }

        queue.base_state.set(new_base_state);
        queue.first_base_update.replace(new_first_base_update);
        queue.last_base_update.replace(new_last_base_update);

        if first_base_update.is_none() {
            // `queue.lanes` is used for entangling transitions. We can set it back to
            // zero once the queue is empty.
            queue.shared.lanes.set(NO_LANES);
        }

        // Set the remaining expiration time to be whatever is remaining in the queue.
        // This should be fine because the only two other things that contribute to
        // expiration time are props and context. We're already in the middle of the
        // begin phase by the time we start processing the queue, so we've already
        // dealt with the props. Context in components that specify
        // shouldComponentUpdate is tricky; but we'll have to account for
        // that regardless.
        mark_skipped_update_lanes(new_lanes);
        work_in_progress.lanes.set(new_lanes);
        work_in_progress.memoized_state.set(new_state);
    }

    if __DEV__ {
        CURRENTLY_PROCESSING_QUEUE.set(None);
    }
}

fn call_callback(callback: &'static Unknown, context: Option<&'static Unknown>) {
    let Kind::Function(callback) = js::classify(Some(callback)) else {
        panic!(
            "Invalid argument passed as callback. Expected a function. Instead received: {}",
            js::shown(callback)
        );
    };

    js::function::call(callback, Some(&context), ());
}

pub fn reset_has_force_update_before_processing() {
    HAS_FORCE_UPDATE.set(false);
}

pub fn check_has_force_update_after_processing() -> bool {
    HAS_FORCE_UPDATE.get()
}

pub fn defer_hidden_callbacks(update_queue: &UpdateQueue) {
    // When an update finishes on a hidden component, its callback should not
    // be fired until/unless the component is made visible again. Stash the
    // callback on the shared queue object so it can be fired later.
    let new_hidden_callbacks = update_queue.callbacks.borrow().clone();
    if let Some(new_hidden_callbacks) = new_hidden_callbacks {
        let existing_hidden_callbacks = update_queue.shared.hidden_callbacks.borrow().clone();
        match existing_hidden_callbacks {
            None => {
                update_queue
                    .shared
                    .hidden_callbacks
                    .replace(Some(new_hidden_callbacks));
            }
            Some(existing_hidden_callbacks) => {
                let hidden_callbacks = [
                    existing_hidden_callbacks.borrow().as_slice(),
                    new_hidden_callbacks.borrow().as_slice(),
                ]
                .concat();
                update_queue
                    .shared
                    .hidden_callbacks
                    .replace(Some(Rc::new(RefCell::new(hidden_callbacks))));
            }
        }
    }
}

pub fn commit_hidden_callbacks(update_queue: &UpdateQueue, context: Option<&'static Unknown>) {
    // This component is switching from hidden -> visible. Commit any callbacks
    // that were previously deferred.
    let hidden_callbacks = update_queue.shared.hidden_callbacks.borrow().clone();
    if let Some(hidden_callbacks) = hidden_callbacks {
        update_queue.shared.hidden_callbacks.replace(None);
        let mut i = 0;
        while i < hidden_callbacks.borrow().len() {
            let callback = hidden_callbacks.borrow()[i];
            call_callback(callback, context);
            i += 1;
        }
    }
}

pub fn commit_callbacks(update_queue: &UpdateQueue, context: Option<&'static Unknown>) {
    let callbacks = update_queue.callbacks.borrow().clone();
    if let Some(callbacks) = callbacks {
        update_queue.callbacks.replace(None);
        let mut i = 0;
        while i < callbacks.borrow().len() {
            let callback = callbacks.borrow()[i];
            call_callback(callback, context);
            i += 1;
        }
    }
}
