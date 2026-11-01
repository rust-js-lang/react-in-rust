// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::marker::PhantomData;

use js::JsObject;
use shared::react_types::ReactComponentInfo;

/// Where a host keeps the component a server component renders in, its
/// Flight config's: an `AsyncLocalStorage`, or none.
pub struct ComponentStorage(PhantomData<JsObject>);

unsafe impl js::Defined for ComponentStorage {}

impl ComponentStorage {
    #[cfg_attr(rust_js, rust_js::link_name = "getStore")]
    fn get_store(&self) -> Option<&'static ReactComponentInfo> {
        unreachable!()
    }
}

unsafe extern "Rust" {
    #[link_name = "./src/ReactFlightServerConfig.js#supportsComponentStorage"]
    safe static SUPPORTS_COMPONENT_STORAGE: bool;
    #[link_name = "./src/ReactFlightServerConfig.js#componentStorage"]
    safe static COMPONENT_STORAGE: &'static ComponentStorage;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_OWNER: Cell<Option<&'static ReactComponentInfo>> = const { Cell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable(component_info))]
pub fn set_current_owner(component_info: Option<&'static ReactComponentInfo>) {
    CURRENT_OWNER.set(component_info);
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn resolve_owner() -> Option<&'static ReactComponentInfo> {
    if let Some(current_owner) = CURRENT_OWNER.get() {
        return Some(current_owner);
    }
    // $FlowFixMe[constant-condition]
    if SUPPORTS_COMPONENT_STORAGE {
        let owner = COMPONENT_STORAGE.get_store();
        if let Some(owner) = owner {
            return Some(owner);
        }
    }
    None
}
