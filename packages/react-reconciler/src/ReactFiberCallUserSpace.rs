// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{JsError, JsObject, Unknown, function};
use react_isomorphic::react_lazy::LazyComponent;
use shared::environment::__DEV__;

use crate::react_captured_value::CapturedValue;
use crate::react_current_fiber::{IS_RENDERING, set_is_rendering};
use crate::react_internal_types::Fiber;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#captureCommitPhaseError"
)]
#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor))]
fn capture_commit_phase_error(
    source_fiber: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
    error: Option<&'static Unknown>,
) {
    unreachable!()
}

// These indirections exists so we can exclude its stack frame in DEV (and anything below it).
// TODO: Consider marking the whole bundle instead of these boundaries.

struct RestoreIsRendering(bool);

impl Drop for RestoreIsRendering {
    fn drop(&mut self) {
        set_is_rendering(self.0);
    }
}

/// A function component: called with its props and a second argument, the
/// ref of a `forwardRef`'s render, or the legacy context.
pub type Component =
    dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>) -> Option<&'static Unknown>;

pub type CallComponent =
    fn(&Component, Option<&'static Unknown>, Option<&'static Unknown>) -> Option<&'static Unknown>;

struct CallComponentFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallComponent,
}

#[cfg_attr(rust_js, rust_js::name = "callComponent")]
const CALL_COMPONENT: CallComponentFrame = CallComponentFrame {
    react_stack_bottom_frame: |component, props, second_arg| {
        let was_rendering = IS_RENDERING.get();
        set_is_rendering(true);
        let _restore = RestoreIsRendering(was_rendering);
        component(props, second_arg)
    },
};

/// What a class component's instance is to the reconciler: its render and
/// its lifecycles.
pub struct ClassInstance(PhantomData<JsObject>);

unsafe impl js::Defined for ClassInstance {}

/// What `componentDidCatch` is given with the error: where it was thrown.
pub struct DidCatchInfo {
    pub component_stack: String,
}

impl ClassInstance {
    #[cfg_attr(rust_js, rust_js::link_name = "render")]
    pub fn render(&self) -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "componentDidMount")]
    pub fn component_did_mount(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "componentDidUpdate")]
    pub fn component_did_update(
        &self,
        prev_props: &'static Unknown,
        prev_state: &'static Unknown,
        snaphot: &'static Unknown,
    ) {
        let _ = (prev_props, prev_state, snaphot);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "componentDidCatch")]
    pub fn component_did_catch(&self, error: Option<&'static Unknown>, error_info: &DidCatchInfo) {
        let _ = (error, error_info);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "componentWillUnmount")]
    pub fn component_will_unmount(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "componentWillMount")]
    pub fn component_will_mount(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "UNSAFE_componentWillMount")]
    pub fn unsafe_component_will_mount(&self) {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "componentWillReceiveProps")]
    pub fn component_will_receive_props(
        &self,
        new_props: Option<&'static Unknown>,
        next_context: Option<&'static Unknown>,
    ) {
        let _ = (new_props, next_context);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "UNSAFE_componentWillReceiveProps")]
    pub fn unsafe_component_will_receive_props(
        &self,
        new_props: Option<&'static Unknown>,
        next_context: Option<&'static Unknown>,
    ) {
        let _ = (new_props, next_context);
        unreachable!()
    }

    // Whether to rerender: what's returned, which JS may make any value.
    #[cfg_attr(rust_js, rust_js::link_name = "shouldComponentUpdate")]
    pub fn should_component_update(
        &self,
        new_props: Option<&'static Unknown>,
        new_state: Option<&'static Unknown>,
        next_context: Option<&'static Unknown>,
    ) -> Option<&'static Unknown> {
        let _ = (new_props, new_state, next_context);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "componentWillUpdate")]
    pub fn component_will_update(
        &self,
        new_props: Option<&'static Unknown>,
        new_state: Option<&'static Unknown>,
        next_context: Option<&'static Unknown>,
    ) {
        let _ = (new_props, new_state, next_context);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "getSnapshotBeforeUpdate")]
    pub fn get_snapshot_before_update(
        &self,
        prev_props: &'static Unknown,
        prev_state: Option<&'static Unknown>,
    ) -> Option<&'static Unknown> {
        let _ = (prev_props, prev_state);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "UNSAFE_componentWillUpdate")]
    pub fn unsafe_component_will_update(
        &self,
        new_props: Option<&'static Unknown>,
        new_state: Option<&'static Unknown>,
        next_context: Option<&'static Unknown>,
    ) {
        let _ = (new_props, new_state, next_context);
        unreachable!()
    }
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callComponentInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_COMPONENT_IN_DEV: Option<CallComponent> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(CALL_COMPONENT.react_stack_bottom_frame)
    } else {
        None
    };
}

pub type CallRender = fn(&ClassInstance) -> Option<&'static Unknown>;

struct CallRenderFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallRender,
}

#[cfg_attr(rust_js, rust_js::name = "callRender")]
const CALL_RENDER: CallRenderFrame = CallRenderFrame {
    react_stack_bottom_frame: |instance| {
        let was_rendering = IS_RENDERING.get();
        set_is_rendering(true);
        let _restore = RestoreIsRendering(was_rendering);
        instance.render()
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callRenderInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_RENDER_IN_DEV: Option<CallRender> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_RENDER.react_stack_bottom_frame, Some(&CALL_RENDER)))
    } else {
        None
    };
}

pub type CallComponentDidMount = fn(&Rc<Fiber>, &ClassInstance);

struct CallComponentDidMountFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallComponentDidMount,
}

#[cfg_attr(rust_js, rust_js::name = "callComponentDidMount")]
const CALL_COMPONENT_DID_MOUNT: CallComponentDidMountFrame = CallComponentDidMountFrame {
    react_stack_bottom_frame: |finished_work, instance| {
        if let Err(error) = catch_unwind(AssertUnwindSafe(|| instance.component_did_mount()))
            && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            let return_fiber = finished_work.r#return.borrow().clone();
            capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
        }
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callComponentDidMountInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_COMPONENT_DID_MOUNT_IN_DEV: Option<CallComponentDidMount> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_COMPONENT_DID_MOUNT.react_stack_bottom_frame, Some(&CALL_COMPONENT_DID_MOUNT)))
    } else {
        None
    };
}

pub type CallComponentDidUpdate =
    fn(&Rc<Fiber>, &ClassInstance, &'static Unknown, &'static Unknown, &'static Unknown);

struct CallComponentDidUpdateFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallComponentDidUpdate,
}

#[cfg_attr(rust_js, rust_js::name = "callComponentDidUpdate")]
const CALL_COMPONENT_DID_UPDATE: CallComponentDidUpdateFrame = CallComponentDidUpdateFrame {
    react_stack_bottom_frame: |finished_work, instance, prev_props, prev_state, snapshot| {
        if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
            instance.component_did_update(prev_props, prev_state, snapshot)
        })) && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            let return_fiber = finished_work.r#return.borrow().clone();
            capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
        }
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callComponentDidUpdateInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_COMPONENT_DID_UPDATE_IN_DEV: Option<CallComponentDidUpdate> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_COMPONENT_DID_UPDATE.react_stack_bottom_frame, Some(&CALL_COMPONENT_DID_UPDATE)))
    } else {
        None
    };
}

pub type CallComponentDidCatch = fn(&ClassInstance, &CapturedValue<Option<&'static Unknown>>);

struct CallComponentDidCatchFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallComponentDidCatch,
}

#[cfg_attr(rust_js, rust_js::name = "callComponentDidCatch")]
const CALL_COMPONENT_DID_CATCH: CallComponentDidCatchFrame = CallComponentDidCatchFrame {
    react_stack_bottom_frame: |instance, error_info| {
        let error = error_info.value;
        let stack = error_info.stack.clone();
        instance.component_did_catch(
            error,
            &DidCatchInfo {
                component_stack: stack.unwrap_or_default(),
            },
        );
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callComponentDidCatchInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_COMPONENT_DID_CATCH_IN_DEV: Option<CallComponentDidCatch> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_COMPONENT_DID_CATCH.react_stack_bottom_frame, Some(&CALL_COMPONENT_DID_CATCH)))
    } else {
        None
    };
}

pub type CallComponentWillUnmount = fn(&Rc<Fiber>, Option<&Rc<Fiber>>, &ClassInstance);

struct CallComponentWillUnmountFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallComponentWillUnmount,
}

#[cfg_attr(rust_js, rust_js::name = "callComponentWillUnmount")]
const CALL_COMPONENT_WILL_UNMOUNT: CallComponentWillUnmountFrame = CallComponentWillUnmountFrame {
    react_stack_bottom_frame: |current, nearest_mounted_ancestor, instance| {
        if let Err(error) = catch_unwind(AssertUnwindSafe(|| instance.component_will_unmount()))
            && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            capture_commit_phase_error(current, nearest_mounted_ancestor, error.as_unknown());
        }
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callComponentWillUnmountInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_COMPONENT_WILL_UNMOUNT_IN_DEV: Option<CallComponentWillUnmount> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_COMPONENT_WILL_UNMOUNT.react_stack_bottom_frame, Some(&CALL_COMPONENT_WILL_UNMOUNT)))
    } else {
        None
    };
}

/// What an effect's `create` gives: its destroy function, or nothing.
pub type Destroy = Option<&'static Unknown>;

// ReactFiberHooks' `Effect`, which isn't ported: its `create`, and its
// `inst`, whose `destroy` is what `create` gave.
pub type CallCreate = fn(&'static Unknown) -> Destroy;

struct CallCreateFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallCreate,
}

#[cfg_attr(rust_js, rust_js::name = "callCreate")]
const CALL_CREATE: CallCreateFrame = CallCreateFrame {
    react_stack_bottom_frame: |effect| {
        let create: &dyn Fn() -> Destroy = unsafe { js::cast(js::get(effect, "create")) };
        let inst = js::get(effect, "inst");
        let destroy = create();
        js::set(inst, "destroy", destroy);
        destroy
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callCreateInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_CREATE_IN_DEV: Option<CallCreate> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_CREATE.react_stack_bottom_frame, Some(&CALL_CREATE)))
    } else {
        None
    };
}

pub type CallDestroy = fn(&Rc<Fiber>, Option<&Rc<Fiber>>, &dyn Fn());

struct CallDestroyFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallDestroy,
}

#[cfg_attr(rust_js, rust_js::name = "callDestroy")]
const CALL_DESTROY: CallDestroyFrame = CallDestroyFrame {
    react_stack_bottom_frame: |current, nearest_mounted_ancestor, destroy| {
        if let Err(error) = catch_unwind(AssertUnwindSafe(destroy))
            && let Some(error) = error.downcast_ref::<&'static JsError>()
        {
            capture_commit_phase_error(current, nearest_mounted_ancestor, error.as_unknown());
        }
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callDestroyInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_DESTROY_IN_DEV: Option<CallDestroy> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(CALL_DESTROY.react_stack_bottom_frame)
    } else {
        None
    };
}

pub type CallLazyInit = fn(&LazyComponent) -> Option<&'static Unknown>;

struct CallLazyInitFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallLazyInit,
}

#[cfg_attr(rust_js, rust_js::name = "callLazyInit")]
const CALL_LAZY_INIT: CallLazyInitFrame = CallLazyInitFrame {
    react_stack_bottom_frame: |lazy| {
        let payload = &lazy.payload;
        let init = lazy.init;
        init(payload)
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callLazyInitInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_LAZY_INIT_IN_DEV: Option<CallLazyInit> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_LAZY_INIT.react_stack_bottom_frame, Some(&CALL_LAZY_INIT)))
    } else {
        None
    };
}
