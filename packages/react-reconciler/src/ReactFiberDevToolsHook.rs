// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::marker::PhantomData;

use js::{JsError, JsObject, Unknown};
use shared::environment::__DEV__;
use shared::react_feature_flags::{ENABLE_PROFILER_TIMER, ENABLE_SCHEDULING_PROFILER};
use shared::react_types::ReactNodeList;
use webapi::console;

use crate::react_event_priorities::{
    CONTINUOUS_EVENT_PRIORITY, DEFAULT_EVENT_PRIORITY, DISCRETE_EVENT_PRIORITY, EventPriority,
    IDLE_EVENT_PRIORITY,
};
use crate::react_fiber_flags::DID_CAPTURE;
use crate::react_fiber_lane::{Lane, Lanes};
use crate::react_internal_types::{Fiber, FiberRoot};
#[cfg_attr(rust_js, rust_js::name = "IdleSchedulerPriority")]
use crate::scheduler::IDLE_PRIORITY as IDLE_SCHEDULER_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "ImmediateSchedulerPriority")]
use crate::scheduler::IMMEDIATE_PRIORITY as IMMEDIATE_SCHEDULER_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "NormalSchedulerPriority")]
use crate::scheduler::NORMAL_PRIORITY as NORMAL_SCHEDULER_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "UserBlockingSchedulerPriority")]
use crate::scheduler::USER_BLOCKING_PRIORITY as USER_BLOCKING_SCHEDULER_PRIORITY;
use crate::scheduler::{LOG, PriorityLevel, UNSTABLE_SET_DISABLE_YIELD_VALUE};

/// React DevTools' global hook, as the reconciler reads it: what DevTools,
/// or Fast Refresh's runtime, installs before React loads.
pub struct DevToolsHook(PhantomData<JsObject>);

impl DevToolsHook {
    /// The renderer's ID DevTools gives it, of its `internals`.
    #[cfg_attr(rust_js, rust_js::link_name = "inject")]
    pub fn inject(
        &self,
        internals: &Unknown,
    ) -> Result<Option<&'static Unknown>, &'static JsError> {
        let _ = internals;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "onScheduleFiberRoot")]
    pub fn on_schedule_fiber_root(
        &self,
        renderer_id: Option<&Unknown>,
        root: &FiberRoot,
        // shared/ReactTypes' `ReactNodeList`.
        children: Option<&Unknown>,
    ) -> Result<(), &'static JsError> {
        let _ = (renderer_id, root, children);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "onCommitFiberRoot")]
    pub fn on_commit_fiber_root(
        &self,
        renderer_id: Option<&Unknown>,
        root: &FiberRoot,
        scheduler_priority: Option<PriorityLevel>,
        did_error: bool,
    ) -> Result<(), &'static JsError> {
        let _ = (renderer_id, root, scheduler_priority, did_error);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "onPostCommitFiberRoot")]
    pub fn on_post_commit_fiber_root(
        &self,
        renderer_id: Option<&Unknown>,
        root: &FiberRoot,
    ) -> Result<(), &'static JsError> {
        let _ = (renderer_id, root);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "onCommitFiberUnmount")]
    pub fn on_commit_fiber_unmount(
        &self,
        renderer_id: Option<&Unknown>,
        fiber: &Fiber,
    ) -> Result<(), &'static JsError> {
        let _ = (renderer_id, fiber);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "setStrictMode")]
    pub fn set_strict_mode(
        &self,
        renderer_id: Option<&Unknown>,
        new_is_strict_mode: bool,
    ) -> Result<(), &'static JsError> {
        let _ = (renderer_id, new_is_strict_mode);
        unreachable!()
    }
}

// import type {DevToolsProfilingHooks} from 'react-devtools-shared/src/backend/types';
// TODO: This import doesn't work because the DevTools depend on the DOM version of React
// and to properly type check against DOM React we can't also type check again non-DOM
// React which this hook might be in.
pub struct DevToolsProfilingHooks(PhantomData<JsObject>);

impl DevToolsProfilingHooks {
    #[cfg_attr(rust_js, rust_js::link_name = "markCommitStarted")]
    pub fn mark_commit_started(&self, lanes: Lanes) {
        let _ = lanes;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markCommitStopped")]
    pub fn mark_commit_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentRenderStarted")]
    pub fn mark_component_render_started(&self, fiber: &Fiber) {
        let _ = fiber;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentRenderStopped")]
    pub fn mark_component_render_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentPassiveEffectMountStarted")]
    pub fn mark_component_passive_effect_mount_started(&self, fiber: &Fiber) {
        let _ = fiber;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentPassiveEffectMountStopped")]
    pub fn mark_component_passive_effect_mount_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "markComponentPassiveEffectUnmountStarted"
    )]
    pub fn mark_component_passive_effect_unmount_started(&self, fiber: &Fiber) {
        let _ = fiber;
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "markComponentPassiveEffectUnmountStopped"
    )]
    pub fn mark_component_passive_effect_unmount_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentLayoutEffectMountStarted")]
    pub fn mark_component_layout_effect_mount_started(&self, fiber: &Fiber) {
        let _ = fiber;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentLayoutEffectMountStopped")]
    pub fn mark_component_layout_effect_mount_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "markComponentLayoutEffectUnmountStarted"
    )]
    pub fn mark_component_layout_effect_unmount_started(&self, fiber: &Fiber) {
        let _ = fiber;
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "markComponentLayoutEffectUnmountStopped"
    )]
    pub fn mark_component_layout_effect_unmount_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentErrored")]
    pub fn mark_component_errored(
        &self,
        fiber: &Fiber,
        thrown_value: Option<&'static Unknown>,
        lanes: Lanes,
    ) {
        let _ = (fiber, thrown_value, lanes);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markComponentSuspended")]
    pub fn mark_component_suspended(
        &self,
        fiber: &Fiber,
        wakeable: &'static Unknown,
        lanes: Lanes,
    ) {
        let _ = (fiber, wakeable, lanes);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markLayoutEffectsStarted")]
    pub fn mark_layout_effects_started(&self, lanes: Lanes) {
        let _ = lanes;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markLayoutEffectsStopped")]
    pub fn mark_layout_effects_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markPassiveEffectsStarted")]
    pub fn mark_passive_effects_started(&self, lanes: Lanes) {
        let _ = lanes;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markPassiveEffectsStopped")]
    pub fn mark_passive_effects_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markRenderStarted")]
    pub fn mark_render_started(&self, lanes: Lanes) {
        let _ = lanes;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markRenderYielded")]
    pub fn mark_render_yielded(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markRenderStopped")]
    pub fn mark_render_stopped(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markRenderScheduled")]
    pub fn mark_render_scheduled(&self, lane: Lane) {
        let _ = lane;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markForceUpdateScheduled")]
    pub fn mark_force_update_scheduled(&self, fiber: &Fiber, lane: Lane) {
        let _ = (fiber, lane);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "markStateUpdateScheduled")]
    pub fn mark_state_update_scheduled(&self, fiber: &Fiber, lane: Lane) {
        let _ = (fiber, lane);
        unreachable!()
    }
}

unsafe extern "Rust" {
    #[link_name = "__REACT_DEVTOOLS_GLOBAL_HOOK__"]
    safe static REACT_DEVTOOLS_GLOBAL_HOOK: &'static DevToolsHook;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "rendererID")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    static RENDERER_ID: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static INJECTED_HOOK: Cell<Option<&'static DevToolsHook>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static INJECTED_PROFILING_HOOKS: Cell<Option<&'static DevToolsProfilingHooks>> =
        const { Cell::new(None) };
    static HAS_LOGGED_ERROR: Cell<bool> = const { Cell::new(false) };

    #[cfg_attr(rust_js, rust_js::name = "isDevToolsPresent")]
    pub static IS_DEV_TOOLS_PRESENT: bool = js::is_defined(REACT_DEVTOOLS_GLOBAL_HOOK);
}

pub fn inject_internals(internals: &'static Unknown) -> bool {
    if !js::is_defined(REACT_DEVTOOLS_GLOBAL_HOOK) {
        // No DevTools
        return false;
    }
    let hook = REACT_DEVTOOLS_GLOBAL_HOOK;
    if js::truthy(js::get(hook, "isDisabled")) {
        // This isn't a real property on the hook, but it can be set to opt out
        // of DevTools integration and associated warnings and logs.
        // https://github.com/react/react/issues/3877
        return true;
    }
    if !js::truthy(js::get(hook, "supportsFiber")) {
        if __DEV__ {
            console::error((concat!(
                "The installed version of React DevTools is too old and will not work ",
                "with the current version of React. Please update React DevTools. ",
                "https://react.dev/link/react-devtools"
            ),));
        }
        // DevTools exists, even though it doesn't support Fiber.
        return true;
    }
    match hook.inject(internals) {
        Ok(renderer_id) => {
            RENDERER_ID.set(renderer_id);

            // We have successfully injected, so now it is safe to set up hooks.
            INJECTED_HOOK.set(Some(hook));
        }
        Err(err) => {
            // Catch all errors because it is unsafe to throw during initialization.
            if __DEV__ {
                console::error((
                    "React instrumentation encountered an error: %o.",
                    err.as_unknown(),
                ));
            }
        }
    }
    // This is the real DevTools, where it says it checks dead code
    // elimination, and else likely a hook installed by Fast Refresh runtime.
    js::truthy(js::get(hook, "checkDCE"))
}

pub fn on_schedule_root(root: &FiberRoot, children: Option<&'static ReactNodeList>) {
    if __DEV__
        && let Some(injected_hook) = INJECTED_HOOK.get()
        && js::type_of(js::get(injected_hook, "onScheduleFiberRoot")) == "function"
    {
        match injected_hook.on_schedule_fiber_root(RENDERER_ID.get(), root, children) {
            Ok(()) => {}
            Err(err) => {
                if __DEV__ && !HAS_LOGGED_ERROR.get() {
                    HAS_LOGGED_ERROR.set(true);
                    console::error((
                        "React instrumentation encountered an error: %o",
                        err.as_unknown(),
                    ));
                }
            }
        }
    }
}

pub fn on_commit_root(root: &FiberRoot, event_priority: EventPriority) {
    if let Some(injected_hook) = INJECTED_HOOK.get()
        && js::type_of(js::get(injected_hook, "onCommitFiberRoot")) == "function"
    {
        let did_error = (root.current.borrow().flags.get() & DID_CAPTURE) == DID_CAPTURE;
        let scheduler_priority = if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            Some(match event_priority {
                DISCRETE_EVENT_PRIORITY => IMMEDIATE_SCHEDULER_PRIORITY.with(|&priority| priority),
                CONTINUOUS_EVENT_PRIORITY => {
                    USER_BLOCKING_SCHEDULER_PRIORITY.with(|&priority| priority)
                }
                DEFAULT_EVENT_PRIORITY => NORMAL_SCHEDULER_PRIORITY.with(|&priority| priority),
                IDLE_EVENT_PRIORITY => IDLE_SCHEDULER_PRIORITY.with(|&priority| priority),
                _ => NORMAL_SCHEDULER_PRIORITY.with(|&priority| priority),
            })
        } else {
            None
        };
        match injected_hook.on_commit_fiber_root(
            RENDERER_ID.get(),
            root,
            scheduler_priority,
            did_error,
        ) {
            Ok(()) => {}
            Err(err) => {
                if __DEV__ && !HAS_LOGGED_ERROR.get() {
                    HAS_LOGGED_ERROR.set(true);
                    console::error((
                        "React instrumentation encountered an error: %o",
                        err.as_unknown(),
                    ));
                }
            }
        }
    }
}

pub fn on_post_commit_root(root: &FiberRoot) {
    if let Some(injected_hook) = INJECTED_HOOK.get()
        && js::type_of(js::get(injected_hook, "onPostCommitFiberRoot")) == "function"
    {
        match injected_hook.on_post_commit_fiber_root(RENDERER_ID.get(), root) {
            Ok(()) => {}
            Err(err) => {
                if __DEV__ && !HAS_LOGGED_ERROR.get() {
                    HAS_LOGGED_ERROR.set(true);
                    console::error((
                        "React instrumentation encountered an error: %o",
                        err.as_unknown(),
                    ));
                }
            }
        }
    }
}

pub fn on_commit_unmount(fiber: &Fiber) {
    if let Some(injected_hook) = INJECTED_HOOK.get()
        && js::type_of(js::get(injected_hook, "onCommitFiberUnmount")) == "function"
    {
        match injected_hook.on_commit_fiber_unmount(RENDERER_ID.get(), fiber) {
            Ok(()) => {}
            Err(err) => {
                if __DEV__ && !HAS_LOGGED_ERROR.get() {
                    HAS_LOGGED_ERROR.set(true);
                    console::error((
                        "React instrumentation encountered an error: %o",
                        err.as_unknown(),
                    ));
                }
            }
        }
    }
}

pub fn set_is_strict_mode_for_devtools(new_is_strict_mode: bool) {
    if js::type_of(LOG.with(|&log| log.map(js::unknown))) == "function" {
        // We're in a test because Scheduler.log only exists
        // in SchedulerMock. To reduce the noise in strict mode tests,
        // suppress warnings and disable scheduler yielding during the double render
        UNSTABLE_SET_DISABLE_YIELD_VALUE.with(|&set_disable_yield_value| {
            set_disable_yield_value.unwrap()(new_is_strict_mode);
        });
    }

    if let Some(injected_hook) = INJECTED_HOOK.get()
        && js::type_of(js::get(injected_hook, "setStrictMode")) == "function"
    {
        match injected_hook.set_strict_mode(RENDERER_ID.get(), new_is_strict_mode) {
            Ok(()) => {}
            Err(err) => {
                if __DEV__ && !HAS_LOGGED_ERROR.get() {
                    HAS_LOGGED_ERROR.set(true);
                    console::error((
                        "React instrumentation encountered an error: %o",
                        err.as_unknown(),
                    ));
                }
            }
        }
    }
}

// Profiler API hooks

pub fn inject_profiling_hooks(profiling_hooks: &'static DevToolsProfilingHooks) {
    INJECTED_PROFILING_HOOKS.set(Some(profiling_hooks));
}

pub fn mark_commit_started(lanes: Lanes) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markCommitStarted")) == "function"
    {
        injected_profiling_hooks.mark_commit_started(lanes);
    }
}

pub fn mark_commit_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markCommitStopped")) == "function"
    {
        injected_profiling_hooks.mark_commit_stopped();
    }
}

pub fn mark_component_render_started(fiber: &Fiber) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentRenderStarted",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_render_started(fiber);
    }
}

pub fn mark_component_render_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentRenderStopped",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_render_stopped();
    }
}

pub fn mark_component_passive_effect_mount_started(fiber: &Fiber) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentPassiveEffectMountStarted",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_passive_effect_mount_started(fiber);
    }
}

pub fn mark_component_passive_effect_mount_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentPassiveEffectMountStopped",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_passive_effect_mount_stopped();
    }
}

pub fn mark_component_passive_effect_unmount_started(fiber: &Fiber) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentPassiveEffectUnmountStarted",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_passive_effect_unmount_started(fiber);
    }
}

pub fn mark_component_passive_effect_unmount_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentPassiveEffectUnmountStopped",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_passive_effect_unmount_stopped();
    }
}

pub fn mark_component_layout_effect_mount_started(fiber: &Fiber) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentLayoutEffectMountStarted",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_layout_effect_mount_started(fiber);
    }
}

pub fn mark_component_layout_effect_mount_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentLayoutEffectMountStopped",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_layout_effect_mount_stopped();
    }
}

pub fn mark_component_layout_effect_unmount_started(fiber: &Fiber) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentLayoutEffectUnmountStarted",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_layout_effect_unmount_started(fiber);
    }
}

pub fn mark_component_layout_effect_unmount_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markComponentLayoutEffectUnmountStopped",
        )) == "function"
    {
        injected_profiling_hooks.mark_component_layout_effect_unmount_stopped();
    }
}

pub fn mark_component_errored(fiber: &Fiber, thrown_value: Option<&'static Unknown>, lanes: Lanes) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markComponentErrored")) == "function"
    {
        injected_profiling_hooks.mark_component_errored(fiber, thrown_value, lanes);
    }
}

pub fn mark_component_suspended(fiber: &Fiber, wakeable: &'static Unknown, lanes: Lanes) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markComponentSuspended")) == "function"
    {
        injected_profiling_hooks.mark_component_suspended(fiber, wakeable, lanes);
    }
}

pub fn mark_layout_effects_started(lanes: Lanes) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markLayoutEffectsStarted",
        )) == "function"
    {
        injected_profiling_hooks.mark_layout_effects_started(lanes);
    }
}

pub fn mark_layout_effects_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markLayoutEffectsStopped",
        )) == "function"
    {
        injected_profiling_hooks.mark_layout_effects_stopped();
    }
}

pub fn mark_passive_effects_started(lanes: Lanes) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markPassiveEffectsStarted",
        )) == "function"
    {
        injected_profiling_hooks.mark_passive_effects_started(lanes);
    }
}

pub fn mark_passive_effects_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markPassiveEffectsStopped",
        )) == "function"
    {
        injected_profiling_hooks.mark_passive_effects_stopped();
    }
}

pub fn mark_render_started(lanes: Lanes) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markRenderStarted")) == "function"
    {
        injected_profiling_hooks.mark_render_started(lanes);
    }
}

pub fn mark_render_yielded() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markRenderYielded")) == "function"
    {
        injected_profiling_hooks.mark_render_yielded();
    }
}

pub fn mark_render_stopped() {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markRenderStopped")) == "function"
    {
        injected_profiling_hooks.mark_render_stopped();
    }
}

pub fn mark_render_scheduled(lane: Lane) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(injected_profiling_hooks, "markRenderScheduled")) == "function"
    {
        injected_profiling_hooks.mark_render_scheduled(lane);
    }
}

pub fn mark_force_update_scheduled(fiber: &Fiber, lane: Lane) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markForceUpdateScheduled",
        )) == "function"
    {
        injected_profiling_hooks.mark_force_update_scheduled(fiber, lane);
    }
}

pub fn mark_state_update_scheduled(fiber: &Fiber, lane: Lane) {
    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled)
        && let Some(injected_profiling_hooks) = INJECTED_PROFILING_HOOKS.get()
        && js::type_of(js::get(
            injected_profiling_hooks,
            "markStateUpdateScheduled",
        )) == "function"
    {
        injected_profiling_hooks.mark_state_update_scheduled(fiber, lane);
    }
}
