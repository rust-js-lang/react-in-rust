// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

use js::{JsError, JsObject, Promise, Unknown, function};
use react_isomorphic::react_lazy::LazyComponent;
use shared::environment::__DEV__;
use shared::react_types::ReactComponentInfo;

use crate::react_fizz_call_user_space::Component;
use crate::react_flight_current_owner::set_current_owner;

pub type CallComponent = fn(
    &Component,
    Option<&'static Unknown>,
    &'static ReactComponentInfo,
) -> Option<&'static Unknown>;

struct CallComponentFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallComponent,
}

/// What a component's call ends with, whether it gave or threw: no owner.
struct ClearOwner;

impl Drop for ClearOwner {
    fn drop(&mut self) {
        set_current_owner(None);
    }
}

// These indirections exists so we can exclude its stack frame in DEV (and anything below it).
// TODO: Consider marking the whole bundle instead of these boundaries.
#[cfg_attr(rust_js, rust_js::name = "callComponent")]
const CALL_COMPONENT: CallComponentFrame = CallComponentFrame {
    react_stack_bottom_frame: |#[cfg_attr(rust_js, rust_js::name = "Component")] component,
                               props,
                               component_debug_info| {
        // The secondArg is always undefined in Server Components since refs error early.
        let second_arg = None;
        set_current_owner(Some(component_debug_info));
        let _clear = ClearOwner;
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

/// An async iterator a server component gives, which Flight steps.
pub struct AsyncIterator(PhantomData<JsObject>);

unsafe impl js::Defined for AsyncIterator {}

impl AsyncIterator {
    #[cfg_attr(rust_js, rust_js::link_name = "next")]
    fn next(&self) -> Promise<&'static Unknown> {
        unreachable!()
    }
}

/// What a step of an async iterator is told of each entry it gave, `{ done,
/// value }`.
pub type Progress = dyn Fn(&'static Unknown);

/// What a step of an async iterator is told of what it threw.
pub type IteratorError = dyn Fn(&'static JsError);

pub type CallIterator = fn(&AsyncIterator, &'static Progress, &'static IteratorError);

struct CallIteratorFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CallIterator,
}

#[cfg_attr(rust_js, rust_js::name = "callIterator")]
const CALL_ITERATOR: CallIteratorFrame = CallIteratorFrame {
    react_stack_bottom_frame: |iterator, progress, error| {
        let _: Promise<()> = iterator.next().then_with(progress, error);
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "callIteratorInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CALL_ITERATOR_IN_DEV: Option<CallIterator> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(CALL_ITERATOR.react_stack_bottom_frame, Some(&CALL_ITERATOR), ()))
    } else {
        None
    };
}
