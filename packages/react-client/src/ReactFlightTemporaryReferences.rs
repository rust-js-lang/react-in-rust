// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use js::Unknown;

/// A temporary reference's value: any object (upstream's `interface
/// Reference {}`) or a symbol.
pub type Reference = &'static Unknown;

pub type TemporaryReferenceSet = Rc<RefCell<HashMap<String, Reference>>>;

pub fn create_temporary_reference_set() -> TemporaryReferenceSet {
    Rc::new(RefCell::new(HashMap::new()))
}

pub fn write_temporary_reference(
    set: &TemporaryReferenceSet,
    reference: String,
    object: Reference,
) {
    set.borrow_mut().insert(reference, object);
}

pub fn read_temporary_reference(set: &TemporaryReferenceSet, reference: &str) -> Option<Reference> {
    set.borrow().get(reference).copied()
}
