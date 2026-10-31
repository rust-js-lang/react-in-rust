// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{SymbolFor, Unknown};
use shared::check_string_coercion::check_key_string_coercion;
use shared::environment::__DEV__;
use shared::react_symbols::{REACT_OPTIMISTIC_KEY, REACT_PORTAL_TYPE};
use shared::react_types::{ReactKey, ReactPortal};

pub fn create_portal(
    children: Option<&'static Unknown>,
    container_info: Option<&'static Unknown>,
    // TODO: figure out the API for cross-renderer implementation.
    implementation: Option<&'static Unknown>,
    key: Option<&'static Unknown>,
) -> ReactPortal {
    let resolved_key = if key.is_none() {
        None
    } else if SymbolFor::from_value(key) == Some(REACT_OPTIMISTIC_KEY) {
        Some(ReactKey::Optimistic(REACT_OPTIMISTIC_KEY))
    } else {
        if __DEV__ {
            check_key_string_coercion(key);
        }
        Some(ReactKey::Text(js::concat_text(key).unwrap()))
    };
    ReactPortal {
        // This tag allow us to uniquely identify this as a React Portal
        r#typeof: REACT_PORTAL_TYPE,
        key: resolved_key,
        children,
        container_info,
        implementation,
    }
}
