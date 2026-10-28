// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use js::{
    ArrayBufferView, FinalizationRegistry, Kind, Map, Set, Unknown, WeakMap, finalization_registry,
};
use shared::react_feature_flags::ENABLE_TAINT;

use shared::get_prototype_of::GET_PROTOTYPE_OF;

use shared::binary_to_comparable_string::binary_to_comparable_string;

use crate::react_shared_internals_server::REACT_SHARED_INTERNALS;
use crate::react_taint_registry::{EntryValue, Reference, RequestCleanupQueue, TaintEntry};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryObjects")]
    static TAINT_REGISTRY_OBJECTS: Cell<Option<&'static WeakMap<Reference, String>>> =
        const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryValues")]
    static TAINT_REGISTRY_VALUES: Cell<Option<&'static Map<EntryValue, Rc<TaintEntry>>>> =
        const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryByteLengths")]
    static TAINT_REGISTRY_BYTE_LENGTHS: Cell<Option<&'static Set<f64>>> =
        const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryPendingRequests")]
    static TAINT_REGISTRY_PENDING_REQUESTS: RefCell<Option<Rc<RefCell<HashSet<&'static RequestCleanupQueue>>>>> =
        const { RefCell::new(None) };

    // This is the shared constructor of all typed arrays.
    #[cfg_attr(rust_js, rust_js::name = "TypedArrayConstructor")]
    static TYPED_ARRAY_CONSTRUCTOR: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
}

js::on_load! {
    REACT_SHARED_INTERNALS.with(|internals| {
        TAINT_REGISTRY_OBJECTS.set(internals.taint_registry_objects.get());
        TAINT_REGISTRY_VALUES.set(internals.taint_registry_values.get());
        TAINT_REGISTRY_BYTE_LENGTHS.set(internals.taint_registry_byte_lengths.get());
        let taint_registry_pending_requests =
            internals.taint_registry_pending_requests.borrow().clone();
        TAINT_REGISTRY_PENDING_REQUESTS.set(taint_registry_pending_requests);
    });

    TYPED_ARRAY_CONSTRUCTOR.set(js::get(
        GET_PROTOTYPE_OF.with(|get_prototype_of| {
            get_prototype_of(Some(js::unknown(js::uint32_array::prototype())))
        }),
        "constructor",
    ));
}

#[cfg_attr(rust_js, rust_js::name = "defaultMessage")]
const DEFAULT_MESSAGE: &str = concat!(
    "A tainted value was attempted to be serialized to a Client Component or Action closure. ",
    "This would leak it to the client."
);

fn cleanup(entry_value: EntryValue) {
    let entry = TAINT_REGISTRY_VALUES.get().unwrap().get(&entry_value);
    if let Some(entry) = entry {
        TAINT_REGISTRY_PENDING_REQUESTS.with_borrow(|pending_requests| {
            pending_requests
                .as_ref()
                .unwrap()
                .borrow()
                .iter()
                .for_each(|request_queue| {
                    request_queue.values.borrow_mut().push(entry_value.clone());
                    entry.count.set(entry.count.get() + 1);
                });
        });
        if entry.count.get() == 1 {
            TAINT_REGISTRY_VALUES.get().unwrap().delete(&entry_value);
        } else {
            entry.count.set(entry.count.get() - 1);
        }
    }
}

// If FinalizationRegistry doesn't exist, we assume that objects life forever.
// E.g. the whole VM is just the lifetime of a request.
thread_local! {
    static FINALIZATION_REGISTRY: Option<&'static FinalizationRegistry<EntryValue>> =
        js::is_defined(finalization_registry::new::<EntryValue>)
            .then(|| finalization_registry::new(Box::new(cleanup)));
}

pub fn taint_unique_value(
    message: Option<&str>,
    lifetime: Option<&Unknown>,
    value: Option<&Unknown>,
) {
    if !ENABLE_TAINT.with(|&enabled| enabled) {
        panic!("Not implemented.");
    }
    // eslint-disable-next-line react-internal/safe-string-coercion
    let message = js::concat_text(
        message
            .filter(|message| !message.is_empty())
            .unwrap_or(DEFAULT_MESSAGE),
    );
    if
    // $FlowFixMe[invalid-compare]
    matches!(js::classify(lifetime), Kind::Null)
        || (js::type_of(lifetime) != "object" && js::type_of(lifetime) != "function")
    {
        panic!(concat!(
            "To taint a value, a lifetime must be defined by passing an object that holds ",
            "the value."
        ));
    }
    let lifetime: &Reference = unsafe { js::cast(lifetime) };
    let entry_value: EntryValue;
    if let Kind::String(_) | Kind::BigInt(_) = js::classify(value) {
        // Use as is.
        entry_value = unsafe { js::cast(value) };
    } else if js::instance_of(value, TYPED_ARRAY_CONSTRUCTOR.get()) || js::data_view::is(value) {
        let value: &ArrayBufferView = unsafe { js::cast(value) };
        // For now, we just convert binary data to a string so that we can just use the native
        // hashing in the Map implementation. It doesn't really matter what form the string
        // take as long as it's the same when we look it up.
        // We're not too worried about collisions since this should be a high entropy value.
        TAINT_REGISTRY_BYTE_LENGTHS
            .get()
            .unwrap()
            .add(&value.byte_length());
        entry_value = EntryValue::String(binary_to_comparable_string(value));
    } else {
        // $FlowFixMe[invalid-compare]
        let kind = if let Kind::Null = js::classify(value) {
            "null".to_string()
        } else {
            js::type_of(value)
        };
        if kind == "object" || kind == "function" {
            panic!(
                "taintUniqueValue cannot taint objects or functions. Try taintObjectReference instead."
            );
        }
        panic!(
            "Cannot taint a {kind} because the value is too general and not unique enough to block globally."
        );
    }
    let existing_entry = TAINT_REGISTRY_VALUES.get().unwrap().get(&entry_value);
    match existing_entry {
        None => {
            TAINT_REGISTRY_VALUES.get().unwrap().set(
                &entry_value,
                Rc::new(TaintEntry {
                    message,
                    count: Cell::new(1),
                }),
            );
        }
        Some(existing_entry) => existing_entry.count.set(existing_entry.count.get() + 1),
    }
    if let Some(finalization_registry) = FINALIZATION_REGISTRY.with(|&registry| registry) {
        finalization_registry.register(lifetime, entry_value, None);
    }
}

pub fn taint_object_reference(message: Option<&str>, object: Option<&Unknown>) {
    if !ENABLE_TAINT.with(|&enabled| enabled) {
        panic!("Not implemented.");
    }
    // eslint-disable-next-line react-internal/safe-string-coercion
    let message = js::concat_text(
        message
            .filter(|message| !message.is_empty())
            .unwrap_or(DEFAULT_MESSAGE),
    );
    if let Kind::String(_) | Kind::BigInt(_) = js::classify(object) {
        panic!(
            "Only objects or functions can be passed to taintObjectReference. Try taintUniqueValue instead."
        );
    }
    if
    // $FlowFixMe[invalid-compare]
    matches!(js::classify(object), Kind::Null)
        || (js::type_of(object) != "object" && js::type_of(object) != "function")
    {
        panic!("Only objects or functions can be passed to taintObjectReference.");
    }
    let object: &Reference = unsafe { js::cast(object) };
    TAINT_REGISTRY_OBJECTS.get().unwrap().set(object, message);
}
