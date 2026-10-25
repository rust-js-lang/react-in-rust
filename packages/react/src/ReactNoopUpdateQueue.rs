// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, Unknown};
use shared::environment::__DEV__;
use webapi::console;

thread_local! {
    static DID_WARN_STATE_UPDATE_FOR_UNMOUNTED_COMPONENT: &'static Dict<bool> = js::dict::new();
}

fn warn_noop(public_instance: &Unknown, caller_name: &str) {
    if __DEV__ {
        let constructor = js::get(public_instance, "constructor");
        let component_name = unsafe {
            js::cast::<Option<String>>(
                constructor
                    .filter(|constructor| js::truthy(Some(*constructor)))
                    .and_then(|constructor| {
                        js::get(constructor, "displayName")
                            .filter(|name| js::truthy(Some(*name)))
                            .or_else(|| js::get(constructor, "name"))
                    })
                    .filter(|name| js::truthy(Some(*name))),
            )
        }
        .unwrap_or_else(|| "ReactClass".to_string());
        let warning_key = format!("{component_name}.{caller_name}");
        if DID_WARN_STATE_UPDATE_FOR_UNMOUNTED_COMPONENT
            .with(|did_warn| js::dict::get(did_warn, &warning_key).is_some_and(|&warned| warned))
        {
            return;
        }
        console::error((
            concat!(
                "Can't call %s on a component that is not yet mounted. ",
                "This is a no-op, but it might indicate a bug in your application. ",
                "Instead, assign to `this.state` directly or define a `state = {};` ",
                "class property with the desired state in the %s component."
            ),
            caller_name,
            component_name,
        ));
        DID_WARN_STATE_UPDATE_FOR_UNMOUNTED_COMPONENT
            .with(|did_warn| js::dict::set(did_warn, &warning_key, true));
    }
}

/// What an updater's `enqueueForceUpdate` is: given the instance, the
/// callback to call after it's updated and the public API's name of it.
pub type EnqueueForceUpdate = fn(&Unknown, Option<&'static dyn Fn()>, Option<&str>);

/// What an updater's `enqueueReplaceState` and `enqueueSetState` are: given
/// the instance, the state, the callback and the public API's name of it.
pub type EnqueueState =
    fn(&Unknown, Option<&'static Unknown>, Option<&'static dyn Fn()>, Option<&str>);

/// What a component's `updater` is: the renderer's, which the component's
/// `setState` and `forceUpdate` call.
pub struct Updater {
    pub is_mounted: fn(&Unknown) -> bool,
    pub enqueue_force_update: EnqueueForceUpdate,
    pub enqueue_replace_state: EnqueueState,
    pub enqueue_set_state: EnqueueState,
}

/// This is the abstract API for an update queue.
#[cfg_attr(rust_js, rust_js::name = "ReactNoopUpdateQueue")]
pub(crate) static REACT_NOOP_UPDATE_QUEUE: Updater = Updater {
    // Checks whether or not this composite component is mounted.
    // @param {ReactClass} publicInstance The instance we want to test.
    // @return {boolean} True if mounted, false otherwise.
    // @protected
    // @final
    is_mounted: |_public_instance| false,

    // Forces an update. This should only be invoked when it is known with
    // certainty that we are **not** in a DOM transaction.
    //
    // You may want to call this when you know that some deeper aspect of the
    // component's state has changed but `setState` was not called.
    //
    // This will not invoke `shouldComponentUpdate`, but it will invoke
    // `componentWillUpdate` and `componentDidUpdate`.
    //
    // @param {ReactClass} publicInstance The instance that should rerender.
    // @param {?function} callback Called after component is updated.
    // @param {?string} callerName name of the calling function in the public API.
    // @internal
    enqueue_force_update: |public_instance, _callback, _caller_name| {
        warn_noop(public_instance, "forceUpdate");
    },

    // Replaces all of the state. Always use this or `setState` to mutate state.
    // You should treat `this.state` as immutable.
    //
    // There is no guarantee that `this.state` will be immediately updated, so
    // accessing `this.state` after calling this method may return the old value.
    //
    // @param {ReactClass} publicInstance The instance that should rerender.
    // @param {object} completeState Next state.
    // @param {?function} callback Called after component is updated.
    // @param {?string} callerName name of the calling function in the public API.
    // @internal
    enqueue_replace_state: |public_instance, _complete_state, _callback, _caller_name| {
        warn_noop(public_instance, "replaceState");
    },

    // Sets a subset of the state. This only exists because _pendingState is
    // internal. This provides a merging strategy that is not available to deep
    // properties which is confusing. TODO: Expose pendingState or don't use it
    // during the merge.
    //
    // @param {ReactClass} publicInstance The instance that should rerender.
    // @param {object} partialState Next partial state to be merged with state.
    // @param {?function} callback Called after component is updated.
    // @param {?string} Name of the calling function in the public API.
    // @internal
    enqueue_set_state: |public_instance, _partial_state, _callback, _caller_name| {
        warn_noop(public_instance, "setState");
    },
};

js::export_default!(REACT_NOOP_UPDATE_QUEUE);
