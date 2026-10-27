// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{JsError, Kind, Set, Unknown};
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    ENABLE_PROFILER_COMMIT_HOOKS, ENABLE_PROFILER_NESTED_UPDATE_PHASE, ENABLE_PROFILER_TIMER,
    ENABLE_SCHEDULING_PROFILER, ENABLE_VIEW_TRANSITION,
};
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_fiber::ViewTransitionState;
use crate::react_fiber_call_user_space::{
    CALL_COMPONENT_DID_MOUNT_IN_DEV, CALL_COMPONENT_DID_UPDATE_IN_DEV,
    CALL_COMPONENT_WILL_UNMOUNT_IN_DEV, CALL_CREATE_IN_DEV, CALL_DESTROY_IN_DEV, ClassInstance,
    Destroy,
};
use crate::react_fiber_class_component::resolve_class_component_props;
use crate::react_fiber_class_update_queue::{
    UpdateQueue, commit_callbacks, commit_hidden_callbacks,
};
use crate::react_fiber_dev_tools_hook::{
    mark_component_layout_effect_mount_started, mark_component_layout_effect_mount_stopped,
    mark_component_layout_effect_unmount_started, mark_component_layout_effect_unmount_stopped,
    mark_component_passive_effect_mount_started, mark_component_passive_effect_mount_stopped,
    mark_component_passive_effect_unmount_started, mark_component_passive_effect_unmount_stopped,
};
use crate::react_fiber_flags::NO_FLAGS;
use crate::react_fiber_view_transition_component::{ViewTransitionProps, get_view_transition_name};
use crate::react_hook_effect_tags::HookFlags;
#[cfg_attr(rust_js, rust_js::name = "HookInsertion")]
use crate::react_hook_effect_tags::INSERTION as HOOK_INSERTION;
#[cfg_attr(rust_js, rust_js::name = "HookLayout")]
use crate::react_hook_effect_tags::LAYOUT as HOOK_LAYOUT;
#[cfg_attr(rust_js, rust_js::name = "NoHookEffect")]
use crate::react_hook_effect_tags::NO_FLAGS as NO_HOOK_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "HookPassive")]
use crate::react_hook_effect_tags::PASSIVE as HOOK_PASSIVE;
use crate::react_internal_types::{Fiber, Ref};
use crate::react_profiler_timer::{
    is_current_update_nested, record_effect_duration, start_effect_timer,
};
use crate::react_type_of_mode::{NO_MODE, PROFILE_MODE};
use crate::react_work_tags::{
    CLASS_COMPONENT, FRAGMENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_SINGLETON,
    VIEW_TRANSITION_COMPONENT,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberBeginWork.js#didWarnAboutReassigningProps"]
    safe static DID_WARN_ABOUT_REASSIGNING_PROPS: bool;
}

// ReactFiberConfig's `Instance`'s public instance.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getPublicInstance",
    rust_js::nullable
)]
fn get_public_instance(instance: Option<&'static Unknown>) -> Option<&'static Unknown> {
    unreachable!()
}

// ReactFiberConfig's `ViewTransitionInstance`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createViewTransitionInstance"
)]
fn create_view_transition_instance(name: &str) -> &'static Unknown {
    unreachable!()
}

// ReactFiberConfig's `FragmentInstanceType`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createFragmentInstance"
)]
fn create_fragment_instance(fragment_fiber: &Fiber) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#captureCommitPhaseError"
)]
#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
fn capture_commit_phase_error(
    source_fiber: &Fiber,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
    error: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#setIsRunningInsertionEffect"
)]
fn set_is_running_insertion_effect(is_running: bool) {
    unreachable!()
}

fn should_profile(current: &Fiber) -> bool {
    ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && (current.mode.get() & PROFILE_MODE) != NO_MODE
}

pub fn commit_hook_layout_effects(finished_work: &Rc<Fiber>, hook_flags: HookFlags) {
    // At this point layout effects have already been destroyed (during mutation phase).
    // This is done to prevent sibling component effects from interfering with each other,
    // e.g. a destroy function in one component should never override a ref set
    // by a create function in another component during the same commit.
    if should_profile(finished_work) {
        start_effect_timer();
        commit_hook_effect_list_mount(hook_flags, finished_work);
        record_effect_duration(finished_work);
    } else {
        commit_hook_effect_list_mount(hook_flags, finished_work);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
pub fn commit_hook_layout_unmount_effects(
    finished_work: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
    hook_flags: HookFlags,
) {
    // Layout effects are destroyed during the mutation phase so that all
    // destroy functions for all fibers are called before any create functions.
    // This prevents sibling component effects from interfering with each other,
    // e.g. a destroy function in one component should never override a ref set
    // by a create function in another component during the same commit.
    if should_profile(finished_work) {
        start_effect_timer();
        commit_hook_effect_list_unmount(hook_flags, finished_work, nearest_mounted_ancestor);
        record_effect_duration(finished_work);
    } else {
        commit_hook_effect_list_unmount(hook_flags, finished_work, nearest_mounted_ancestor);
    }
}

pub fn commit_hook_effect_list_mount(flags: HookFlags, finished_work: &Rc<Fiber>) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        // ReactFiberHooks' `FunctionComponentUpdateQueue`, and its `Effect`s.
        let update_queue = finished_work.update_queue.get();
        let last_effect = update_queue.and_then(|update_queue| js::get(update_queue, "lastEffect"));
        if let Some(last_effect) = last_effect {
            let first_effect = js::get(last_effect, "next");
            let mut effect = first_effect;
            loop {
                if (unsafe { js::cast::<HookFlags>(js::get(effect, "tag")) } & flags) == flags {
                    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
                        if (flags & HOOK_PASSIVE) != NO_HOOK_EFFECT {
                            mark_component_passive_effect_mount_started(finished_work);
                        } else if (flags & HOOK_LAYOUT) != NO_HOOK_EFFECT {
                            mark_component_layout_effect_mount_started(finished_work);
                        }
                    }

                    // Mount
                    let destroy: Destroy;
                    if __DEV__ {
                        if (flags & HOOK_INSERTION) != NO_HOOK_EFFECT {
                            set_is_running_insertion_effect(true);
                        }
                        destroy = run_with_fiber_in_dev(
                            Some(Rc::clone(finished_work)),
                            |effect: &'static Unknown, _: (), _: (), _: (), _: ()| {
                                CALL_CREATE_IN_DEV
                                    .with(|call_create_in_dev| call_create_in_dev.unwrap()(effect))
                            },
                            unsafe { js::cast(effect) },
                            (),
                            (),
                            (),
                            (),
                        );
                        if (flags & HOOK_INSERTION) != NO_HOOK_EFFECT {
                            set_is_running_insertion_effect(false);
                        }
                    } else {
                        let create: &dyn Fn() -> Destroy =
                            unsafe { js::cast(js::get(effect, "create")) };
                        let inst = js::get(effect, "inst");
                        destroy = create();
                        js::set(inst, "destroy", destroy);
                    }

                    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
                        if (flags & HOOK_PASSIVE) != NO_HOOK_EFFECT {
                            mark_component_passive_effect_mount_stopped();
                        } else if (flags & HOOK_LAYOUT) != NO_HOOK_EFFECT {
                            mark_component_layout_effect_mount_stopped();
                        }
                    }

                    if __DEV__
                        && !matches!(js::classify(destroy), Kind::Undefined)
                        && js::type_of(destroy) != "function"
                    {
                        let hook_name =
                            if (unsafe { js::cast::<HookFlags>(js::get(effect, "tag")) }
                                & HOOK_LAYOUT)
                                != NO_FLAGS
                            {
                                "useLayoutEffect"
                            } else if (unsafe { js::cast::<HookFlags>(js::get(effect, "tag")) }
                                & HOOK_INSERTION)
                                != NO_FLAGS
                            {
                                "useInsertionEffect"
                            } else {
                                "useEffect"
                            };
                        let addendum = if matches!(js::classify(destroy), Kind::Null) {
                            concat!(
                                " You returned null. If your effect does not require clean ",
                                "up, return undefined (or nothing)."
                            )
                            .to_string()
                        } else if js::type_of(js::get(destroy, "then")) == "function" {
                            "\n\nIt looks like you wrote ".to_string()
                                + hook_name
                                + "(async () => ...) or returned a Promise. "
                                + "Instead, write the async function inside your effect "
                                + "and call it immediately:\n\n"
                                + hook_name
                                + "(() => {\n"
                                + "  async function fetchData() {\n"
                                + "    // You can await here\n"
                                + "    const response = await MyAPI.getData(someId);\n"
                                + "    // ...\n"
                                + "  }\n"
                                + "  fetchData();\n"
                                + "}, [someId]); // Or [] if effect doesn't need props or state\n\n"
                                + "Learn more about data fetching with Hooks: https://react.dev/link/hooks-data-fetching"
                        } else {
                            // $FlowFixMe[unsafe-addition] (@poteto)
                            " You returned: ".to_string() + &js::concat_text(destroy).unwrap()
                        };
                        run_with_fiber_in_dev(
                            Some(Rc::clone(finished_work)),
                            |n: &str, a: &str, _: (), _: (), _: ()| {
                                console::error((
                                    concat!(
                                        "%s must not return anything besides a function, ",
                                        "which is used for clean-up.%s"
                                    ),
                                    n,
                                    a,
                                ));
                            },
                            hook_name,
                            addendum.as_str(),
                            (),
                            (),
                            (),
                        );
                    }
                }
                effect = js::get(effect, "next");
                if js::strict_equals(effect, first_effect) {
                    break;
                }
            }
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
pub fn commit_hook_effect_list_unmount(
    flags: HookFlags,
    finished_work: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        // ReactFiberHooks' `FunctionComponentUpdateQueue`, and its `Effect`s.
        let update_queue = finished_work.update_queue.get();
        let last_effect = update_queue.and_then(|update_queue| js::get(update_queue, "lastEffect"));
        if let Some(last_effect) = last_effect {
            let first_effect = js::get(last_effect, "next");
            let mut effect = first_effect;
            loop {
                if (unsafe { js::cast::<HookFlags>(js::get(effect, "tag")) } & flags) == flags {
                    // Unmount
                    let inst = js::get(effect, "inst");
                    let destroy = js::get(inst, "destroy");
                    if !matches!(js::classify(destroy), Kind::Undefined) {
                        js::set(inst, "destroy", ());
                        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
                            if (flags & HOOK_PASSIVE) != NO_HOOK_EFFECT {
                                mark_component_passive_effect_unmount_started(finished_work);
                            } else if (flags & HOOK_LAYOUT) != NO_HOOK_EFFECT {
                                mark_component_layout_effect_unmount_started(finished_work);
                            }
                        }

                        if __DEV__ && (flags & HOOK_INSERTION) != NO_HOOK_EFFECT {
                            set_is_running_insertion_effect(true);
                        }
                        safely_call_destroy(finished_work, nearest_mounted_ancestor, unsafe {
                            js::cast(destroy)
                        });
                        if __DEV__ && (flags & HOOK_INSERTION) != NO_HOOK_EFFECT {
                            set_is_running_insertion_effect(false);
                        }

                        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
                            if (flags & HOOK_PASSIVE) != NO_HOOK_EFFECT {
                                mark_component_passive_effect_unmount_stopped();
                            } else if (flags & HOOK_LAYOUT) != NO_HOOK_EFFECT {
                                mark_component_layout_effect_unmount_stopped();
                            }
                        }
                    }
                }
                effect = js::get(effect, "next");
                if js::strict_equals(effect, first_effect) {
                    break;
                }
            }
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_hook_passive_mount_effects(finished_work: &Rc<Fiber>, hook_flags: HookFlags) {
    if should_profile(finished_work) {
        start_effect_timer();
        commit_hook_effect_list_mount(hook_flags, finished_work);
        record_effect_duration(finished_work);
    } else {
        commit_hook_effect_list_mount(hook_flags, finished_work);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
pub fn commit_hook_passive_unmount_effects(
    finished_work: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
    hook_flags: HookFlags,
) {
    if should_profile(finished_work) {
        start_effect_timer();
        commit_hook_effect_list_unmount(hook_flags, finished_work, nearest_mounted_ancestor);
        record_effect_duration(finished_work);
    } else {
        commit_hook_effect_list_unmount(hook_flags, finished_work, nearest_mounted_ancestor);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(current))]
pub fn commit_class_layout_lifecycles(finished_work: &Rc<Fiber>, current: Option<&Fiber>) {
    let instance: &ClassInstance = unsafe { js::cast(finished_work.state_node.get()) };
    match current {
        None => {
            // We could update instance props and state here,
            // but instead we rely on them being set during last render.
            // TODO: revisit this when we implement resuming.
            if __DEV__
                && !js::truthy(js::get(finished_work.r#type.get(), "defaultProps"))
                && !js::has(finished_work.memoized_props.get(), "ref")
                && !DID_WARN_ABOUT_REASSIGNING_PROPS
            {
                if !js::strict_equals(
                    js::get(instance, "props"),
                    finished_work.memoized_props.get(),
                ) {
                    console::error((
                        concat!(
                            "Expected %s props to match memoized props before ",
                            "componentDidMount. ",
                            "This might either be because of a bug in React, or because ",
                            "a component reassigns its own `this.props`. ",
                            "Please file an issue."
                        ),
                        get_component_name_from_fiber(finished_work)
                            .filter(|name| js::truthy(Some(name.as_str())))
                            .unwrap_or_else(|| "instance".to_string()),
                    ));
                }
                if !js::strict_equals(
                    js::get(instance, "state"),
                    finished_work.memoized_state.get(),
                ) {
                    console::error((
                        concat!(
                            "Expected %s state to match memoized state before ",
                            "componentDidMount. ",
                            "This might either be because of a bug in React, or because ",
                            "a component reassigns its own `this.state`. ",
                            "Please file an issue."
                        ),
                        get_component_name_from_fiber(finished_work)
                            .filter(|name| js::truthy(Some(name.as_str())))
                            .unwrap_or_else(|| "instance".to_string()),
                    ));
                }
            }

            if should_profile(finished_work) {
                start_effect_timer();
                if __DEV__ {
                    run_with_fiber_in_dev(
                        Some(Rc::clone(finished_work)),
                        |finished_work: &Rc<Fiber>,
                         instance: &ClassInstance,
                         _: (),
                         _: (),
                         _: ()| {
                            CALL_COMPONENT_DID_MOUNT_IN_DEV.with(
                                |call_component_did_mount_in_dev| {
                                    call_component_did_mount_in_dev.unwrap()(
                                        finished_work,
                                        instance,
                                    )
                                },
                            )
                        },
                        finished_work,
                        instance,
                        (),
                        (),
                        (),
                    );
                } else if let Err(error) =
                    catch_unwind(AssertUnwindSafe(|| instance.component_did_mount()))
                    && let Some(error) = error.downcast_ref::<&'static JsError>()
                {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    capture_commit_phase_error(
                        finished_work,
                        return_fiber.as_ref(),
                        error.as_unknown(),
                    );
                }
                record_effect_duration(finished_work);
            } else {
                if __DEV__ {
                    run_with_fiber_in_dev(
                        Some(Rc::clone(finished_work)),
                        |finished_work: &Rc<Fiber>,
                         instance: &ClassInstance,
                         _: (),
                         _: (),
                         _: ()| {
                            CALL_COMPONENT_DID_MOUNT_IN_DEV.with(
                                |call_component_did_mount_in_dev| {
                                    call_component_did_mount_in_dev.unwrap()(
                                        finished_work,
                                        instance,
                                    )
                                },
                            )
                        },
                        finished_work,
                        instance,
                        (),
                        (),
                        (),
                    );
                } else if let Err(error) =
                    catch_unwind(AssertUnwindSafe(|| instance.component_did_mount()))
                    && let Some(error) = error.downcast_ref::<&'static JsError>()
                {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    capture_commit_phase_error(
                        finished_work,
                        return_fiber.as_ref(),
                        error.as_unknown(),
                    );
                }
            }
        }
        Some(current) => {
            let prev_props = resolve_class_component_props(finished_work.r#type.get(), unsafe {
                js::cast(current.memoized_props.get())
            });
            let prev_state: &Unknown = unsafe { js::cast(current.memoized_state.get()) };
            // We could update instance props and state here,
            // but instead we rely on them being set during last render.
            // TODO: revisit this when we implement resuming.
            if __DEV__
                && !js::truthy(js::get(finished_work.r#type.get(), "defaultProps"))
                && !js::has(finished_work.memoized_props.get(), "ref")
                && !DID_WARN_ABOUT_REASSIGNING_PROPS
            {
                if !js::strict_equals(
                    js::get(instance, "props"),
                    finished_work.memoized_props.get(),
                ) {
                    console::error((
                        concat!(
                            "Expected %s props to match memoized props before ",
                            "componentDidUpdate. ",
                            "This might either be because of a bug in React, or because ",
                            "a component reassigns its own `this.props`. ",
                            "Please file an issue."
                        ),
                        get_component_name_from_fiber(finished_work)
                            .filter(|name| js::truthy(Some(name.as_str())))
                            .unwrap_or_else(|| "instance".to_string()),
                    ));
                }
                if !js::strict_equals(
                    js::get(instance, "state"),
                    finished_work.memoized_state.get(),
                ) {
                    console::error((
                        concat!(
                            "Expected %s state to match memoized state before ",
                            "componentDidUpdate. ",
                            "This might either be because of a bug in React, or because ",
                            "a component reassigns its own `this.state`. ",
                            "Please file an issue."
                        ),
                        get_component_name_from_fiber(finished_work)
                            .filter(|name| js::truthy(Some(name.as_str())))
                            .unwrap_or_else(|| "instance".to_string()),
                    ));
                }
            }

            if should_profile(finished_work) {
                start_effect_timer();
                if __DEV__ {
                    run_with_fiber_in_dev(
                        Some(Rc::clone(finished_work)),
                        |finished_work: &Rc<Fiber>,
                         instance: &ClassInstance,
                         prev_props: &'static Unknown,
                         prev_state: &'static Unknown,
                         snapshot: &'static Unknown| {
                            CALL_COMPONENT_DID_UPDATE_IN_DEV.with(
                                |call_component_did_update_in_dev| {
                                    call_component_did_update_in_dev.unwrap()(
                                        finished_work,
                                        instance,
                                        prev_props,
                                        prev_state,
                                        snapshot,
                                    )
                                },
                            )
                        },
                        finished_work,
                        instance,
                        prev_props,
                        prev_state,
                        unsafe {
                            js::cast(js::get(instance, "__reactInternalSnapshotBeforeUpdate"))
                        },
                    );
                } else if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                    instance.component_did_update(prev_props, prev_state, unsafe {
                        js::cast(js::get(instance, "__reactInternalSnapshotBeforeUpdate"))
                    })
                })) && let Some(error) = error.downcast_ref::<&'static JsError>()
                {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    capture_commit_phase_error(
                        finished_work,
                        return_fiber.as_ref(),
                        error.as_unknown(),
                    );
                }
                record_effect_duration(finished_work);
            } else {
                if __DEV__ {
                    run_with_fiber_in_dev(
                        Some(Rc::clone(finished_work)),
                        |finished_work: &Rc<Fiber>,
                         instance: &ClassInstance,
                         prev_props: &'static Unknown,
                         prev_state: &'static Unknown,
                         snapshot: &'static Unknown| {
                            CALL_COMPONENT_DID_UPDATE_IN_DEV.with(
                                |call_component_did_update_in_dev| {
                                    call_component_did_update_in_dev.unwrap()(
                                        finished_work,
                                        instance,
                                        prev_props,
                                        prev_state,
                                        snapshot,
                                    )
                                },
                            )
                        },
                        finished_work,
                        instance,
                        prev_props,
                        prev_state,
                        unsafe {
                            js::cast(js::get(instance, "__reactInternalSnapshotBeforeUpdate"))
                        },
                    );
                } else if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                    instance.component_did_update(prev_props, prev_state, unsafe {
                        js::cast(js::get(instance, "__reactInternalSnapshotBeforeUpdate"))
                    })
                })) && let Some(error) = error.downcast_ref::<&'static JsError>()
                {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    capture_commit_phase_error(
                        finished_work,
                        return_fiber.as_ref(),
                        error.as_unknown(),
                    );
                }
            }
        }
    }
}

pub fn commit_class_did_mount(finished_work: &Rc<Fiber>) {
    // TODO: Check for LayoutStatic flag
    let instance: &ClassInstance = unsafe { js::cast(finished_work.state_node.get()) };
    if js::type_of(js::get(instance, "componentDidMount")) == "function" {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |finished_work: &Rc<Fiber>, instance: &ClassInstance, _: (), _: (), _: ()| {
                    CALL_COMPONENT_DID_MOUNT_IN_DEV.with(|call_component_did_mount_in_dev| {
                        call_component_did_mount_in_dev.unwrap()(finished_work, instance)
                    })
                },
                finished_work,
                instance,
                (),
                (),
                (),
            );
        } else if let Err(error) = catch_unwind(AssertUnwindSafe(|| instance.component_did_mount()))
            && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            let return_fiber = finished_work.r#return.borrow().clone();
            capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
        }
    }
}

pub fn commit_class_callbacks(finished_work: &Rc<Fiber>) {
    // TODO: I think this is now always non-null by the time it reaches the
    // commit phase. Consider removing the type check.
    let update_queue: Option<&UpdateQueue> = unsafe { js::cast(finished_work.update_queue.get()) };
    if let Some(update_queue) = update_queue {
        let instance: &ClassInstance = unsafe { js::cast(finished_work.state_node.get()) };
        if __DEV__
            && !js::truthy(js::get(finished_work.r#type.get(), "defaultProps"))
            && !js::has(finished_work.memoized_props.get(), "ref")
            && !DID_WARN_ABOUT_REASSIGNING_PROPS
        {
            if !js::strict_equals(
                js::get(instance, "props"),
                finished_work.memoized_props.get(),
            ) {
                console::error((
                    concat!(
                        "Expected %s props to match memoized props before ",
                        "processing the update queue. ",
                        "This might either be because of a bug in React, or because ",
                        "a component reassigns its own `this.props`. ",
                        "Please file an issue."
                    ),
                    get_component_name_from_fiber(finished_work)
                        .filter(|name| js::truthy(Some(name.as_str())))
                        .unwrap_or_else(|| "instance".to_string()),
                ));
            }
            if !js::strict_equals(
                js::get(instance, "state"),
                finished_work.memoized_state.get(),
            ) {
                console::error((
                    concat!(
                        "Expected %s state to match memoized state before ",
                        "processing the update queue. ",
                        "This might either be because of a bug in React, or because ",
                        "a component reassigns its own `this.state`. ",
                        "Please file an issue."
                    ),
                    get_component_name_from_fiber(finished_work)
                        .filter(|name| js::truthy(Some(name.as_str())))
                        .unwrap_or_else(|| "instance".to_string()),
                ));
            }
        }

        // We could update instance props and state here,
        // but instead we rely on them being set during last render.
        // TODO: revisit this when we implement resuming.
        if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
            if __DEV__ {
                run_with_fiber_in_dev(
                    Some(Rc::clone(finished_work)),
                    |update_queue: &UpdateQueue,
                     instance: Option<&'static Unknown>,
                     _: (),
                     _: (),
                     _: ()| commit_callbacks(update_queue, instance),
                    update_queue,
                    Some(js::unknown(instance)),
                    (),
                    (),
                    (),
                );
            } else {
                commit_callbacks(update_queue, Some(js::unknown(instance)));
            }
        })) && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            let return_fiber = finished_work.r#return.borrow().clone();
            capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
        }
    }
}

pub fn commit_class_hidden_callbacks(finished_work: &Rc<Fiber>) {
    // Commit any callbacks that would have fired while the component
    // was hidden.
    let update_queue: Option<&UpdateQueue> = unsafe { js::cast(finished_work.update_queue.get()) };
    if let Some(update_queue) = update_queue {
        let instance = finished_work.state_node.get();
        if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
            if __DEV__ {
                run_with_fiber_in_dev(
                    Some(Rc::clone(finished_work)),
                    |update_queue: &UpdateQueue,
                     instance: Option<&'static Unknown>,
                     _: (),
                     _: (),
                     _: ()| commit_hidden_callbacks(update_queue, instance),
                    update_queue,
                    instance,
                    (),
                    (),
                    (),
                );
            } else {
                commit_hidden_callbacks(update_queue, instance);
            }
        })) && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            let return_fiber = finished_work.r#return.borrow().clone();
            capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
        }
    }
}

pub fn commit_root_callbacks(finished_work: &Rc<Fiber>) {
    // TODO: I think this is now always non-null by the time it reaches the
    // commit phase. Consider removing the type check.
    let update_queue: Option<&UpdateQueue> = unsafe { js::cast(finished_work.update_queue.get()) };
    if let Some(update_queue) = update_queue {
        let mut instance = js::null();
        let child = finished_work.child.borrow().clone();
        if let Some(child) = child {
            match child.tag.get() {
                HOST_SINGLETON | HOST_COMPONENT => {
                    instance = get_public_instance(child.state_node.get());
                }
                CLASS_COMPONENT => {
                    instance = child.state_node.get();
                }
                _ => {}
            }
        }
        if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
            if __DEV__ {
                run_with_fiber_in_dev(
                    Some(Rc::clone(finished_work)),
                    |update_queue: &UpdateQueue,
                     instance: Option<&'static Unknown>,
                     _: (),
                     _: (),
                     _: ()| commit_callbacks(update_queue, instance),
                    update_queue,
                    instance,
                    (),
                    (),
                    (),
                );
            } else {
                commit_callbacks(update_queue, instance);
            }
        })) && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            let return_fiber = finished_work.r#return.borrow().clone();
            capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
        }
    }
}

thread_local! {
    static DID_WARN_ABOUT_UNDEFINED_SNAPSHOT_BEFORE_UPDATE: &'static Set<Option<&'static Unknown>> =
        js::set::new(None);
}

fn call_get_snapshot_before_updates(
    instance: &ClassInstance,
    prev_props: &'static Unknown,
    prev_state: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    instance.get_snapshot_before_update(prev_props, prev_state)
}

pub fn commit_class_snapshot(finished_work: &Rc<Fiber>, current: &Fiber) {
    let prev_props = current.memoized_props.get();
    let prev_state = current.memoized_state.get();
    let instance: &ClassInstance = unsafe { js::cast(finished_work.state_node.get()) };
    // We could update instance props and state here,
    // but instead we rely on them being set during last render.
    // TODO: revisit this when we implement resuming.
    if __DEV__
        && !js::truthy(js::get(finished_work.r#type.get(), "defaultProps"))
        && !js::has(finished_work.memoized_props.get(), "ref")
        && !DID_WARN_ABOUT_REASSIGNING_PROPS
    {
        if !js::strict_equals(
            js::get(instance, "props"),
            finished_work.memoized_props.get(),
        ) {
            console::error((
                concat!(
                    "Expected %s props to match memoized props before ",
                    "getSnapshotBeforeUpdate. ",
                    "This might either be because of a bug in React, or because ",
                    "a component reassigns its own `this.props`. ",
                    "Please file an issue."
                ),
                get_component_name_from_fiber(finished_work)
                    .filter(|name| js::truthy(Some(name.as_str())))
                    .unwrap_or_else(|| "instance".to_string()),
            ));
        }
        if !js::strict_equals(
            js::get(instance, "state"),
            finished_work.memoized_state.get(),
        ) {
            console::error((
                concat!(
                    "Expected %s state to match memoized state before ",
                    "getSnapshotBeforeUpdate. ",
                    "This might either be because of a bug in React, or because ",
                    "a component reassigns its own `this.state`. ",
                    "Please file an issue."
                ),
                get_component_name_from_fiber(finished_work)
                    .filter(|name| js::truthy(Some(name.as_str())))
                    .unwrap_or_else(|| "instance".to_string()),
            ));
        }
    }
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        let resolved_prev_props =
            resolve_class_component_props(finished_work.r#type.get(), unsafe {
                js::cast(prev_props)
            });
        let snapshot;
        if __DEV__ {
            snapshot = run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |instance: &ClassInstance,
                 resolved_prev_props: &'static Unknown,
                 prev_state: Option<&'static Unknown>,
                 _: (),
                 _: ()| {
                    call_get_snapshot_before_updates(instance, resolved_prev_props, prev_state)
                },
                instance,
                resolved_prev_props,
                prev_state,
                (),
                (),
            );
            let did_warn_set =
                DID_WARN_ABOUT_UNDEFINED_SNAPSHOT_BEFORE_UPDATE.with(|did_warn| *did_warn);
            if matches!(js::classify(snapshot), Kind::Undefined)
                && !did_warn_set.has(&finished_work.r#type.get())
            {
                did_warn_set.add(&finished_work.r#type.get());
                run_with_fiber_in_dev(
                    Some(Rc::clone(finished_work)),
                    |_: (), _: (), _: (), _: (), _: ()| {
                        console::error((
                            concat!(
                                "%s.getSnapshotBeforeUpdate(): A snapshot value (or null) ",
                                "must be returned. You have returned undefined."
                            ),
                            get_component_name_from_fiber(finished_work),
                        ));
                    },
                    (),
                    (),
                    (),
                    (),
                    (),
                );
            }
        } else {
            snapshot = call_get_snapshot_before_updates(instance, resolved_prev_props, prev_state);
        }
        js::set(instance, "__reactInternalSnapshotBeforeUpdate", snapshot);
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

// Capture errors so they don't interrupt unmounting.
#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
pub fn safely_call_component_will_unmount(
    current: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
    instance: &'static ClassInstance,
) {
    js::set(
        instance,
        "props",
        resolve_class_component_props(current.r#type.get(), unsafe {
            js::cast(current.memoized_props.get())
        }),
    );
    js::set(instance, "state", current.memoized_state.get());
    if should_profile(current) {
        start_effect_timer();
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(current)),
                |current: &Rc<Fiber>,
                 nearest_mounted_ancestor: Option<&Rc<Fiber>>,
                 instance: &ClassInstance,
                 _: (),
                 _: ()| {
                    CALL_COMPONENT_WILL_UNMOUNT_IN_DEV.with(|call_component_will_unmount_in_dev| {
                        call_component_will_unmount_in_dev.unwrap()(
                            current,
                            nearest_mounted_ancestor,
                            instance,
                        )
                    })
                },
                current,
                nearest_mounted_ancestor,
                instance,
                (),
                (),
            );
        } else if let Err(error) =
            catch_unwind(AssertUnwindSafe(|| instance.component_will_unmount()))
            && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            capture_commit_phase_error(current, nearest_mounted_ancestor, error.as_unknown());
        }
        record_effect_duration(current);
    } else {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(current)),
                |current: &Rc<Fiber>,
                 nearest_mounted_ancestor: Option<&Rc<Fiber>>,
                 instance: &ClassInstance,
                 _: (),
                 _: ()| {
                    CALL_COMPONENT_WILL_UNMOUNT_IN_DEV.with(|call_component_will_unmount_in_dev| {
                        call_component_will_unmount_in_dev.unwrap()(
                            current,
                            nearest_mounted_ancestor,
                            instance,
                        )
                    })
                },
                current,
                nearest_mounted_ancestor,
                instance,
                (),
                (),
            );
        } else if let Err(error) =
            catch_unwind(AssertUnwindSafe(|| instance.component_will_unmount()))
            && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            capture_commit_phase_error(current, nearest_mounted_ancestor, error.as_unknown());
        }
    }
}

struct RecordEffectDuration<'a>(&'a Fiber);

impl Drop for RecordEffectDuration<'_> {
    fn drop(&mut self) {
        record_effect_duration(self.0);
    }
}

fn commit_attach_ref(finished_work: &Fiber) {
    let r#ref = finished_work.r#ref.get();
    if let Some(r#ref) = r#ref {
        let instance_to_use = match finished_work.tag.get() {
            HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
                get_public_instance(finished_work.state_node.get())
            }
            VIEW_TRANSITION_COMPONENT => {
                if ENABLE_VIEW_TRANSITION {
                    let instance: &ViewTransitionState =
                        unsafe { js::cast(finished_work.state_node.get()) };
                    let props: &ViewTransitionProps =
                        unsafe { js::cast(finished_work.memoized_props.get()) };
                    let name = get_view_transition_name(props, instance);
                    if instance.r#ref.get().is_none_or(|instance_ref| {
                        !js::strict_equals(js::get(instance_ref, "name"), name.as_str())
                    }) {
                        instance
                            .r#ref
                            .set(Some(create_view_transition_instance(&name)));
                    }
                    instance.r#ref.get()
                } else {
                    finished_work.state_node.get()
                }
            }
            FRAGMENT => {
                // ReactFiberConfig's `FragmentInstanceType`.
                let instance = finished_work.state_node.get();
                if instance.is_none() {
                    finished_work
                        .state_node
                        .set(Some(create_fragment_instance(finished_work)));
                }
                finished_work.state_node.get()
            }
            _ => finished_work.state_node.get(),
        };
        match r#ref {
            Ref::Callback(r#ref) => {
                if should_profile(finished_work) {
                    let _record = RecordEffectDuration(finished_work);
                    start_effect_timer();
                    finished_work.ref_cleanup.set(r#ref(instance_to_use));
                } else {
                    finished_work.ref_cleanup.set(r#ref(instance_to_use));
                }
            }
            Ref::Object(r#ref) => {
                if __DEV__ {
                    // TODO: We should move these warnings to happen during the render
                    // phase (markRef).
                    if js::type_of(Some(r#ref)) == "string" {
                        console::error(("String refs are no longer supported.",));
                    } else if !r#ref.has_own_property("current") {
                        console::error((
                            concat!(
                                "Unexpected ref object provided for %s. ",
                                "Use either a ref-setter function or React.createRef()."
                            ),
                            get_component_name_from_fiber(finished_work),
                        ));
                    }
                }

                // $FlowFixMe[incompatible-use] unable to narrow type to the non-function case
                js::set(r#ref, "current", instance_to_use);
            }
        }
    }
}

// Capture errors so they don't interrupt mounting.
#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
pub fn safely_attach_ref(current: &Rc<Fiber>, nearest_mounted_ancestor: Option<&Rc<Fiber>>) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(current)),
                |current: &Fiber, _: (), _: (), _: (), _: ()| commit_attach_ref(current),
                &**current,
                (),
                (),
                (),
                (),
            );
        } else {
            commit_attach_ref(current);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        capture_commit_phase_error(current, nearest_mounted_ancestor, error.as_unknown());
    }
}

struct ClearRefCleanup<'a>(&'a Fiber);

impl Drop for ClearRefCleanup<'_> {
    fn drop(&mut self) {
        // `refCleanup` has been called. Nullify all references to it to prevent double invocation.
        self.0.ref_cleanup.set(js::null());
        let finished_work = self.0.alternate.borrow().clone();
        if let Some(finished_work) = finished_work {
            finished_work.ref_cleanup.set(js::null());
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
pub fn safely_detach_ref(current: &Rc<Fiber>, nearest_mounted_ancestor: Option<&Rc<Fiber>>) {
    let r#ref = current.r#ref.get();
    let ref_cleanup = current.ref_cleanup.get();

    if let Some(r#ref) = r#ref {
        if js::type_of(ref_cleanup) == "function" {
            let ref_cleanup: &dyn Fn() = unsafe { js::cast(ref_cleanup) };
            let _clear = ClearRefCleanup(current);
            if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                if should_profile(current) {
                    let _record = RecordEffectDuration(current);
                    start_effect_timer();
                    if __DEV__ {
                        run_with_fiber_in_dev(
                            Some(Rc::clone(current)),
                            |_: (), _: (), _: (), _: (), _: ()| ref_cleanup(),
                            (),
                            (),
                            (),
                            (),
                            (),
                        );
                    } else {
                        ref_cleanup();
                    }
                } else if __DEV__ {
                    run_with_fiber_in_dev(
                        Some(Rc::clone(current)),
                        |_: (), _: (), _: (), _: (), _: ()| ref_cleanup(),
                        (),
                        (),
                        (),
                        (),
                        (),
                    );
                } else {
                    ref_cleanup();
                }
            })) && let Some(error) = error.downcast_ref::<&'static JsError>()
            {
                capture_commit_phase_error(current, nearest_mounted_ancestor, error.as_unknown());
            }
        } else {
            match r#ref {
                Ref::Callback(r#ref) => {
                    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                        if should_profile(current) {
                            let _record = RecordEffectDuration(current);
                            start_effect_timer();
                            if __DEV__ {
                                run_with_fiber_in_dev(
                                    Some(Rc::clone(current)),
                                    |instance: Option<&'static Unknown>,
                                     _: (),
                                     _: (),
                                     _: (),
                                     _: ()| { r#ref(instance) },
                                    js::null(),
                                    (),
                                    (),
                                    (),
                                    (),
                                );
                            } else {
                                r#ref(js::null());
                            }
                        } else if __DEV__ {
                            run_with_fiber_in_dev(
                                Some(Rc::clone(current)),
                                |instance: Option<&'static Unknown>, _: (), _: (), _: (), _: ()| {
                                    r#ref(instance)
                                },
                                js::null(),
                                (),
                                (),
                                (),
                                (),
                            );
                        } else {
                            r#ref(js::null());
                        }
                    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
                    {
                        capture_commit_phase_error(
                            current,
                            nearest_mounted_ancestor,
                            error.as_unknown(),
                        );
                    }
                }
                Ref::Object(r#ref) => {
                    // $FlowFixMe[incompatible-use] unable to narrow type to RefObject
                    js::set(r#ref, "current", js::null::<&Unknown>());
                }
            }
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
fn safely_call_destroy(
    current: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
    destroy: &'static dyn Fn(),
) {
    if __DEV__ {
        run_with_fiber_in_dev(
            Some(Rc::clone(current)),
            |current: &Rc<Fiber>,
             nearest_mounted_ancestor: Option<&Rc<Fiber>>,
             destroy: &'static dyn Fn(),
             _: (),
             _: ()| {
                CALL_DESTROY_IN_DEV.with(|call_destroy_in_dev| {
                    call_destroy_in_dev.unwrap()(current, nearest_mounted_ancestor, destroy)
                })
            },
            current,
            nearest_mounted_ancestor,
            destroy,
            (),
            (),
        );
    } else if let Err(error) = catch_unwind(AssertUnwindSafe(destroy))
        && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        capture_commit_phase_error(current, nearest_mounted_ancestor, error.as_unknown());
    }
}

/// shared/ReactTypes' `ProfilerProps`' `onRender`.
type OnRender = dyn Fn(Option<&'static Unknown>, &str, Option<f64>, Option<f64>, Option<f64>, f64);
/// shared/ReactTypes' `ProfilerProps`' `onCommit` and `onPostCommit`.
type OnCommit = dyn Fn(Option<&'static Unknown>, &str, f64, f64);

#[cfg_attr(rust_js, rust_js::nullable(current))]
fn commit_profiler(
    finished_work: &Fiber,
    current: Option<&Fiber>,
    commit_start_time: f64,
    effect_duration: f64,
) {
    let id = js::get(finished_work.memoized_props.get(), "id");
    let on_commit = js::get(finished_work.memoized_props.get(), "onCommit");
    let on_render = js::get(finished_work.memoized_props.get(), "onRender");

    let mut phase = if current.is_none() { "mount" } else { "update" };
    if ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled) && is_current_update_nested() {
        phase = "nested-update";
    }

    if js::type_of(on_render) == "function" {
        let on_render: &OnRender = unsafe { js::cast(on_render) };
        on_render(
            id,
            phase,
            // $FlowFixMe[incompatible-type]: This should be always a number in profiling mode
            finished_work.actual_duration.get(),
            // $FlowFixMe[incompatible-type]: This should be always a number in profiling mode
            finished_work.tree_base_duration.get(),
            // $FlowFixMe[incompatible-type]: This should be always a number in profiling mode
            finished_work.actual_start_time.get(),
            commit_start_time,
        );
    }

    if ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled) && js::type_of(on_commit) == "function"
    {
        let on_commit: &OnCommit = unsafe { js::cast(on_commit) };
        on_commit(id, phase, effect_duration, commit_start_time);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(current))]
pub fn commit_profiler_update(
    finished_work: &Rc<Fiber>,
    current: Option<&Fiber>,
    commit_start_time: f64,
    effect_duration: f64,
) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && let Err(error) = catch_unwind(AssertUnwindSafe(|| {
            if __DEV__ {
                run_with_fiber_in_dev(
                    Some(Rc::clone(finished_work)),
                    |finished_work: &Fiber,
                     current: Option<&Fiber>,
                     commit_start_time: f64,
                     effect_duration: f64,
                     _: ()| {
                        commit_profiler(finished_work, current, commit_start_time, effect_duration)
                    },
                    &**finished_work,
                    current,
                    commit_start_time,
                    effect_duration,
                    (),
                );
            } else {
                commit_profiler(finished_work, current, commit_start_time, effect_duration);
            }
        }))
        && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

#[cfg_attr(rust_js, rust_js::nullable(current))]
fn commit_profiler_post_commit_impl(
    finished_work: &Fiber,
    current: Option<&Fiber>,
    commit_start_time: f64,
    passive_effect_duration: f64,
) {
    let id = js::get(finished_work.memoized_props.get(), "id");
    let on_post_commit = js::get(finished_work.memoized_props.get(), "onPostCommit");

    let mut phase = if current.is_none() { "mount" } else { "update" };
    if ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled) && is_current_update_nested() {
        phase = "nested-update";
    }

    if js::type_of(on_post_commit) == "function" {
        let on_post_commit: &OnCommit = unsafe { js::cast(on_post_commit) };
        on_post_commit(id, phase, passive_effect_duration, commit_start_time);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(current))]
pub fn commit_profiler_post_commit(
    finished_work: &Rc<Fiber>,
    current: Option<&Fiber>,
    commit_start_time: f64,
    passive_effect_duration: f64,
) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |finished_work: &Fiber,
                 current: Option<&Fiber>,
                 commit_start_time: f64,
                 passive_effect_duration: f64,
                 _: ()| {
                    commit_profiler_post_commit_impl(
                        finished_work,
                        current,
                        commit_start_time,
                        passive_effect_duration,
                    )
                },
                &**finished_work,
                current,
                commit_start_time,
                passive_effect_duration,
                (),
            );
        } else {
            commit_profiler_post_commit_impl(
                finished_work,
                current,
                commit_start_time,
                passive_effect_duration,
            );
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}
