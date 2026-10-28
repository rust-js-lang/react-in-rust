// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::rc::Rc;

use js::{JsObject, Map, Set, WeakKey, WeakMap};

/// What a taint holds on to: an object, or a function, the value's
/// lifetime or the tainted object itself.
#[cfg_attr(rust_js, rust_js::types = "object")]
pub struct Reference(PhantomData<JsObject>);

unsafe impl js::Defined for Reference {}
unsafe impl WeakKey for Reference {}

/// A tainted value: text, or a `bigint`, binary data as the text it makes.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum EntryValue {
    String(String),
    BigInt(i64),
}

unsafe impl js::Defined for EntryValue {}

pub struct TaintEntry {
    pub message: String,
    pub count: Cell<u32>,
}

unsafe impl js::Defined for TaintEntry {}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryObjects")]
    pub static TAINT_REGISTRY_OBJECTS: &'static WeakMap<Reference, String> = js::weak_map::new();
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryValues")]
    pub static TAINT_REGISTRY_VALUES: &'static Map<EntryValue, Rc<TaintEntry>> = js::map::new(None);
    // Byte lengths of all binary values we've ever seen. We don't both refcounting this.
    // We expect to see only a few lengths here such as the length of token.
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryByteLengths")]
    pub static TAINT_REGISTRY_BYTE_LENGTHS: &'static Set<f64> = js::set::new(None);
}

/// `RequestCleanupQueue`: the values a request still has a handle on,
/// each kept tainted until it's done.
#[repr(transparent)]
pub struct RequestCleanupQueue {
    pub values: RefCell<Vec<EntryValue>>,
}

impl PartialEq for RequestCleanupQueue {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for RequestCleanupQueue {}

impl Hash for RequestCleanupQueue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

// When a value is finalized, it means that it has been removed from any global caches.
// No future requests can get a handle on it but any ongoing requests can still have
// a handle on it. It's still tainted until that happens.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryPendingRequests")]
    pub static TAINT_REGISTRY_PENDING_REQUESTS: Rc<RefCell<HashSet<&'static RequestCleanupQueue>>> =
        Rc::new(RefCell::new(HashSet::new()));
}
