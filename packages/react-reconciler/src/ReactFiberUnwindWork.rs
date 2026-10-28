// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::Unknown;
use react_isomorphic::react_context::ReactContext;
use shared::react_feature_flags::{ENABLE_PROFILER_TIMER, ENABLE_TRANSITION_TRACING};

use crate::react_fiber_activity_component::ActivityState;
use crate::react_fiber_cache_component::{CacheComponentState, pop_cache_provider};
use crate::react_fiber_flags::{DID_CAPTURE, NO_FLAGS, SHOULD_CAPTURE, UPDATE};
use crate::react_fiber_hidden_context::pop_hidden_context;
use crate::react_fiber_host_context::{pop_host_container, pop_host_context};
use crate::react_fiber_hydration_context::reset_hydration_state;
use crate::react_fiber_lane::Lanes;
use crate::react_fiber_legacy_context::{
    is_context_provider as is_legacy_context_provider, pop_context as pop_legacy_context,
    pop_top_level_context_object as pop_top_level_legacy_context_object,
};
use crate::react_fiber_new_context::pop_provider;
use crate::react_fiber_root::RootState;
use crate::react_fiber_suspense_component::{SuspenseListRenderState, SuspenseState};
use crate::react_fiber_suspense_context::{pop_suspense_handler, pop_suspense_list_context};
use crate::react_fiber_tracing_marker_component::{pop_marker_instance, pop_root_marker_instance};
use crate::react_fiber_transition::{pop_root_transition, pop_transition};
use crate::react_fiber_tree_context::pop_tree_context;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_profiler_timer::transfer_actual_duration;
use crate::react_type_of_mode::{NO_MODE, PROFILE_MODE};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CACHE_COMPONENT, CLASS_COMPONENT, CONTEXT_PROVIDER, HOST_COMPONENT,
    HOST_HOISTABLE, HOST_PORTAL, HOST_ROOT, HOST_SINGLETON, LEGACY_HIDDEN_COMPONENT,
    OFFSCREEN_COMPONENT, SUSPENSE_COMPONENT, SUSPENSE_LIST_COMPONENT, TRACING_MARKER_COMPONENT,
};

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn unwind_work(
    current: Option<&Fiber>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    // Note: This intentionally doesn't check if we're hydrating because comparing
    // to the current tree provider fiber is just as fast and less error-prone.
    // Ideally we would have a special version of the work loop only
    // for hydration.
    pop_tree_context(work_in_progress);
    match work_in_progress.tag.get() {
        CLASS_COMPONENT => {
            let component = work_in_progress.r#type.get();
            if is_legacy_context_provider(component) {
                pop_legacy_context(work_in_progress);
            }
            let flags = work_in_progress.flags.get();
            if (flags & SHOULD_CAPTURE) != 0 {
                work_in_progress
                    .flags
                    .set((flags & !SHOULD_CAPTURE) | DID_CAPTURE);
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
                {
                    transfer_actual_duration(work_in_progress);
                }
                return Some(Rc::clone(work_in_progress));
            }
            None
        }
        HOST_ROOT => {
            let root: &FiberRoot = unsafe { js::cast(work_in_progress.state_node.get()) };
            let cache =
                &unsafe { js::cast::<&RootState>(work_in_progress.memoized_state.get()) }.cache;
            pop_cache_provider(work_in_progress, cache);

            if ENABLE_TRANSITION_TRACING {
                pop_root_marker_instance(work_in_progress);
            }

            pop_root_transition(work_in_progress, root, render_lanes);
            pop_host_container(work_in_progress);
            pop_top_level_legacy_context_object(work_in_progress);
            let flags = work_in_progress.flags.get();
            if (flags & SHOULD_CAPTURE) != NO_FLAGS && (flags & DID_CAPTURE) == NO_FLAGS {
                // There was an error during render that wasn't captured by a suspense
                // boundary. Do a second pass on the root to unmount the children.
                work_in_progress
                    .flags
                    .set((flags & !SHOULD_CAPTURE) | DID_CAPTURE);
                return Some(Rc::clone(work_in_progress));
            }
            // We unwound to the root without completing it. Exit.
            None
        }
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            // TODO: popHydrationState
            pop_host_context(work_in_progress);
            None
        }
        ACTIVITY_COMPONENT => {
            let activity_state: Option<&ActivityState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };
            if activity_state.is_some() {
                pop_suspense_handler(work_in_progress);

                if work_in_progress.alternate.borrow().is_none() {
                    panic!(
                        "{}",
                        concat!(
                            "Threw in newly mounted dehydrated component. This is likely a bug in ",
                            "React. Please file an issue."
                        )
                    );
                }

                reset_hydration_state();
            }

            let flags = work_in_progress.flags.get();
            if (flags & SHOULD_CAPTURE) != 0 {
                work_in_progress
                    .flags
                    .set((flags & !SHOULD_CAPTURE) | DID_CAPTURE);
                // Captured a suspense effect. Re-render the boundary.
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
                {
                    transfer_actual_duration(work_in_progress);
                }
                return Some(Rc::clone(work_in_progress));
            }
            None
        }
        SUSPENSE_COMPONENT => {
            pop_suspense_handler(work_in_progress);
            let suspense_state: Option<&SuspenseState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };
            if let Some(suspense_state) = suspense_state
                && suspense_state.dehydrated.is_some()
            {
                if work_in_progress.alternate.borrow().is_none() {
                    panic!(
                        "{}",
                        concat!(
                            "Threw in newly mounted dehydrated component. This is likely a bug in ",
                            "React. Please file an issue."
                        )
                    );
                }

                reset_hydration_state();
            }

            let flags = work_in_progress.flags.get();
            if (flags & SHOULD_CAPTURE) != 0 {
                work_in_progress
                    .flags
                    .set((flags & !SHOULD_CAPTURE) | DID_CAPTURE);
                // Captured a suspense effect. Re-render the boundary.
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
                {
                    transfer_actual_duration(work_in_progress);
                }
                return Some(Rc::clone(work_in_progress));
            }
            None
        }
        SUSPENSE_LIST_COMPONENT => {
            pop_suspense_list_context(work_in_progress);
            // SuspenseList doesn't normally catch anything. It should've been
            // caught by a nested boundary. If not, it should bubble through.
            let flags = work_in_progress.flags.get();
            if (flags & SHOULD_CAPTURE) != 0 {
                work_in_progress
                    .flags
                    .set((flags & !SHOULD_CAPTURE) | DID_CAPTURE);
                // If we caught something on the SuspenseList itself it's because
                // we want to ignore something. Re-enter the cycle and handle it
                // in the complete phase.
                let render_state: Option<&SuspenseListRenderState> =
                    unsafe { js::cast(work_in_progress.memoized_state.get()) };
                if let Some(render_state) = render_state {
                    // Cut off any remaining tail work and don't commit the rendering one.
                    // This assumes that we have already confirmed that none of these are
                    // already mounted.
                    render_state.rendering.replace(None);
                    render_state.tail.replace(None);
                }
                // Schedule the commit phase to attach retry listeners.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | UPDATE);
                return Some(Rc::clone(work_in_progress));
            }
            None
        }
        HOST_PORTAL => {
            pop_host_container(work_in_progress);
            None
        }
        CONTEXT_PROVIDER => {
            let context: &ReactContext<Option<&'static Unknown>> =
                unsafe { js::cast(work_in_progress.r#type.get()) };
            pop_provider(context, work_in_progress);
            None
        }
        OFFSCREEN_COMPONENT | LEGACY_HIDDEN_COMPONENT => {
            pop_suspense_handler(work_in_progress);
            pop_hidden_context(work_in_progress);
            pop_transition(work_in_progress, current);
            let flags = work_in_progress.flags.get();
            if (flags & SHOULD_CAPTURE) != 0 {
                work_in_progress
                    .flags
                    .set((flags & !SHOULD_CAPTURE) | DID_CAPTURE);
                // Captured a suspense effect. Re-render the boundary.
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
                {
                    transfer_actual_duration(work_in_progress);
                }
                return Some(Rc::clone(work_in_progress));
            }
            None
        }
        CACHE_COMPONENT => {
            let cache =
                &unsafe { js::cast::<&CacheComponentState>(work_in_progress.memoized_state.get()) }
                    .cache;
            pop_cache_provider(work_in_progress, cache);
            None
        }
        TRACING_MARKER_COMPONENT => {
            if ENABLE_TRANSITION_TRACING && work_in_progress.state_node.get().is_some() {
                pop_marker_instance(work_in_progress);
            }
            None
        }
        _ => None,
    }
}

pub fn unwind_interrupted_work(
    current: Option<&Fiber>,
    interrupted_work: &Rc<Fiber>,
    render_lanes: Lanes,
) {
    // Note: This intentionally doesn't check if we're hydrating because comparing
    // to the current tree provider fiber is just as fast and less error-prone.
    // Ideally we would have a special version of the work loop only
    // for hydration.
    pop_tree_context(interrupted_work);
    match interrupted_work.tag.get() {
        CLASS_COMPONENT => {
            let child_context_types = js::get(interrupted_work.r#type.get(), "childContextTypes");
            if child_context_types.is_some() {
                pop_legacy_context(interrupted_work);
            }
        }
        HOST_ROOT => {
            let root: &FiberRoot = unsafe { js::cast(interrupted_work.state_node.get()) };
            let cache =
                &unsafe { js::cast::<&RootState>(interrupted_work.memoized_state.get()) }.cache;
            pop_cache_provider(interrupted_work, cache);

            if ENABLE_TRANSITION_TRACING {
                pop_root_marker_instance(interrupted_work);
            }

            pop_root_transition(interrupted_work, root, render_lanes);
            pop_host_container(interrupted_work);
            pop_top_level_legacy_context_object(interrupted_work);
        }
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            pop_host_context(interrupted_work);
        }
        HOST_PORTAL => pop_host_container(interrupted_work),
        ACTIVITY_COMPONENT => {
            if interrupted_work.memoized_state.get().is_some() {
                pop_suspense_handler(interrupted_work);
            }
        }
        SUSPENSE_COMPONENT => pop_suspense_handler(interrupted_work),
        SUSPENSE_LIST_COMPONENT => pop_suspense_list_context(interrupted_work),
        CONTEXT_PROVIDER => {
            let context: &ReactContext<Option<&'static Unknown>> =
                unsafe { js::cast(interrupted_work.r#type.get()) };
            pop_provider(context, interrupted_work);
        }
        OFFSCREEN_COMPONENT | LEGACY_HIDDEN_COMPONENT => {
            pop_suspense_handler(interrupted_work);
            pop_hidden_context(interrupted_work);
            pop_transition(interrupted_work, current);
        }
        CACHE_COMPONENT => {
            let cache =
                &unsafe { js::cast::<&CacheComponentState>(interrupted_work.memoized_state.get()) }
                    .cache;
            pop_cache_provider(interrupted_work, cache);
        }
        TRACING_MARKER_COMPONENT if ENABLE_TRANSITION_TRACING => {
            let instance = interrupted_work.state_node.get();
            if instance.is_some() {
                pop_marker_instance(interrupted_work);
            }
        }
        _ => {}
    }
}
