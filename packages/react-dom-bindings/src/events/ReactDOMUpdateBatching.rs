// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use react_reconciler::react_fiber_work_loop::{
    batched_updates as batched_updates_impl, discrete_updates as discrete_updates_impl,
    flush_sync_work,
};

use crate::react_dom_controlled_component::{needs_state_restore, restore_state_if_needed};

// Used as a way to call batchedUpdates when we don't have a reference to
// the renderer. Such as when we're dispatching events or if third party
// libraries need to call batchedUpdates. Eventually, this API will go away when
// everything is batched by default. We'll then have a similar API to opt-out of
// scheduled work and instead do synchronous work.

thread_local! {
    static IS_INSIDE_EVENT_HANDLER: Cell<bool> = const { Cell::new(false) };
}

fn finish_event_handler() {
    // Here we wait until all updates have propagated, which is important
    // when using controlled components within layers:
    // https://github.com/react/react/issues/1698
    // Then we restore state of any controlled component.
    let controlled_components_have_pending_updates = needs_state_restore();
    if controlled_components_have_pending_updates {
        // If a controlled event was fired, we may need to restore the state of
        // the DOM node back to the controlled value. This is necessary when React
        // bails out of the update without touching the DOM.
        // TODO: Restore state in the microtask, after the discrete updates flush,
        // instead of early flushing them here.
        // @TODO Should move to flushSyncWork once legacy mode is removed but since this flushSync
        // flushes passive effects we can't do this yet.
        flush_sync_work();
        restore_state_if_needed();
    }
}

/// What leaves the event handler however its batch is left.
struct ExitEventHandler;

impl Drop for ExitEventHandler {
    fn drop(&mut self) {
        IS_INSIDE_EVENT_HANDLER.set(false);
        finish_event_handler();
    }
}

pub fn batched_updates<A, B, R>(r#fn: impl FnOnce(A, Option<B>) -> R, a: A, b: Option<B>) -> R {
    if IS_INSIDE_EVENT_HANDLER.get() {
        // If we are currently inside another batch, we need to wait until it
        // fully completes before restoring state.
        return r#fn(a, b);
    }
    IS_INSIDE_EVENT_HANDLER.set(true);
    let _exit = ExitEventHandler;
    batched_updates_impl(|a| r#fn(a, b), a)
}

// TODO: Replace with flushSync
pub fn discrete_updates<A, B, C, D, R>(
    r#fn: impl FnOnce(A, B, C, D) -> R,
    a: A,
    b: B,
    c: C,
    d: D,
) -> R {
    discrete_updates_impl(r#fn, a, b, c, d)
}
