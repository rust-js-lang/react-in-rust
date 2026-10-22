// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

use crate::has_own_property::HAS_OWN_PROPERTY;
use crate::object_is::OBJECT_IS as IS;

js::export_default! {
    /// Performs equality by iterating through keys on an object and returning false
    /// when any key has values which are not strictly equal between the arguments.
    /// Returns true when the values of all keys are strictly equal.
    fn shallow_equal(obj_a: Option<&'static Unknown>, obj_b: Option<&'static Unknown>) -> bool {
        let is = IS.with(|is| *is);
        if is(&obj_a, &obj_b) {
            return true;
        }

        let (Some(obj_a), Some(obj_b)) = (obj_a, obj_b) else {
            return false;
        };
        if js::type_of(Some(obj_a)) != "object" || js::type_of(Some(obj_b)) != "object" {
            return false;
        }

        let keys_a = js::object::keys(obj_a);
        let keys_b = js::object::keys(obj_b);

        if keys_a.len() != keys_b.len() {
            return false;
        }

        // Test for A's keys different from B.
        let has_own_property = HAS_OWN_PROPERTY.with(|has_own_property| *has_own_property);
        for current_key in &keys_a {
            if !has_own_property.call(obj_b, current_key)
                || !is(&js::get(obj_a, current_key), &js::get(obj_b, current_key))
            {
                return false;
            }
        }

        true
    }
}
