// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

use js::{JsObject, Unknown, function};
use react_isomorphic::react_lazy::LazyComponent;
use shared::environment::__DEV__;

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

// These indirections exists so we can exclude its stack frame in DEV (and anything below it).
// TODO: Consider marking the whole bundle instead of these boundaries.

#[cfg_attr(rust_js, rust_js::name = "callComponent")]
const CALL_COMPONENT: CallComponentFrame = CallComponentFrame {
    react_stack_bottom_frame: |#[cfg_attr(rust_js, rust_js::name = "Component")] component,
                               props,
                               second_arg| {
        component(props, second_arg)
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callComponentInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_COMPONENT_IN_DEV: Option<CallComponent> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_COMPONENT.react_stack_bottom_frame, Some(&CALL_COMPONENT), ()))
    } else {
        None
    };
}

/// What Fizz renders of a class component's instance.
pub struct ClassInstance(PhantomData<JsObject>);

unsafe impl js::Defined for ClassInstance {}

impl ClassInstance {
    #[cfg_attr(rust_js, rust_js::link_name = "render")]
    pub fn render(&self) -> Option<&'static Unknown> {
        unreachable!()
    }
}

pub type CallRender = fn(&ClassInstance) -> Option<&'static Unknown>;

struct CallRenderFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallRender,
}

#[cfg_attr(rust_js, rust_js::name = "callRender")]
const CALL_RENDER: CallRenderFrame = CallRenderFrame {
    react_stack_bottom_frame: |instance| instance.render(),
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callRenderInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_RENDER_IN_DEV: Option<CallRender> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_RENDER.react_stack_bottom_frame, Some(&CALL_RENDER), ()))
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
        Some(function::bind(CALL_LAZY_INIT.react_stack_bottom_frame, Some(&CALL_LAZY_INIT), ()))
    } else {
        None
    };
}
