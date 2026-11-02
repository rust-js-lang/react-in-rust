// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use webapi::{Blob, File, FormData, FormDataEntryValue, form_data};

/// Backing FormData indexes the entries of a reply payload so that no lookup
/// ever scans a live FormData instance. FormData.get/getAll/delete are linear
/// in the number of entries in every major runtime, which could make decoding
/// quadratic in the payload size. Every structure here is either an append-only
/// array with a cursor or a map, so no operation is proportional to the total
/// number of stored entries.
///
/// Keys are classified by the character following the response prefix:
/// - '_' starts the namespace of referenced FormData entries. These are only
///   ever consumed in insertion order by the "$K" scan, so they go into a
///   plain array with a cursor.
/// - 'S' starts the namespace of stream/async-iterable chunks. These are the
///   only fields that can repeat, so they accumulate in an array per key.
/// - Everything else is a one-shot chunk or an outlined Blob, looked up by
///   exact field name.
pub struct BackingFormData {
    prefix: String,
    // Entries whose key is in the `prefix + '_' + ...` namespace, in insertion
    // order. The only structure the "$K" scan reads.
    #[cfg_attr(rust_js, rust_js::nullable)]
    referenced_entries: RefCell<Option<Vec<(String, BackingEntryValue)>>>,
    // Cursor for the "$K" scan; advances monotonically.
    key_pointer: Cell<usize>,
    // Field name -> string, for one-shot `prefix + <decimal id>` chunk fields.
    #[cfg_attr(rust_js, rust_js::nullable)]
    string_chunks: RefCell<Option<HashMap<String, String>>>,
    // Field name (`prefix + 'S' + <decimal id>`) -> buffered chunk strings, in
    // arrival order. Only actual streams/async iterables allocate an array.
    #[cfg_attr(rust_js, rust_js::nullable)]
    stream_chunks: RefCell<Option<HashMap<String, Vec<String>>>>,
    // Field name -> outlined Blob/File. The client outlines each Blob under a
    // fresh id, so keys don't repeat.
    #[cfg_attr(rust_js, rust_js::nullable)]
    blobs: RefCell<Option<HashMap<String, &'static Blob>>>,
}

/// A `FormDataEntryValue` the store keeps: its text, or its file.
// ReplyServer's resolveField gives the store text it's only lent, `&str`,
// which an owned `String` keeps.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum BackingEntryValue {
    Str(String),
    File(&'static File),
}

pub fn append_backing_entry(backing_store: &BackingFormData, value: BackingEntryValue, key: &str) {
    let prefix = &backing_store.prefix;
    if !key.starts_with(prefix.as_str()) {
        // Not owned by this reply (e.g. $ACTION_ fields). Never looked up through
        // the backing store, so don't index it.
        return;
    }
    let namespace = js::string::get(key, f64::from(js::string::length(prefix)));
    match namespace {
        Some("_") => {
            let mut referenced_entries = backing_store.referenced_entries.borrow_mut();
            let referenced_entries = referenced_entries.get_or_insert_with(Vec::new);
            referenced_entries.push((key.to_owned(), value));
            return;
        }
        Some("S") => {
            if let BackingEntryValue::Str(value) = value {
                let mut stream_chunks = backing_store.stream_chunks.borrow_mut();
                let stream_chunks = stream_chunks.get_or_insert_with(HashMap::new);
                let existing = stream_chunks.get_mut(key);
                match existing {
                    None => {
                        stream_chunks.insert(key.to_owned(), vec![value]);
                    }
                    Some(existing) => existing.push(value),
                }
            }
            // Files in the stream namespace are ignored, matching the filter in
            // resolveStream.
            return;
        }
        _ => {}
    }

    match value {
        BackingEntryValue::Str(value) => {
            let mut string_chunks = backing_store.string_chunks.borrow_mut();
            let string_chunks = string_chunks.get_or_insert_with(HashMap::new);
            string_chunks.insert(key.to_owned(), value);
        }
        BackingEntryValue::File(value) => {
            let mut blobs = backing_store.blobs.borrow_mut();
            let blobs = blobs.get_or_insert_with(HashMap::new);
            blobs.insert(key.to_owned(), value);
        }
    }
}

pub fn create_backing_form_data(form_data: &FormData, prefix: &str) -> BackingFormData {
    let backing_store = BackingFormData {
        prefix: prefix.to_owned(),
        referenced_entries: RefCell::new(None),
        key_pointer: Cell::new(0),
        string_chunks: RefCell::new(None),
        stream_chunks: RefCell::new(None),
        blobs: RefCell::new(None),
    };
    // $FlowFixMe[prop-missing]
    form_data.for_each(|value, key, _| {
        let value = match value {
            FormDataEntryValue::Str(value) => BackingEntryValue::Str(value.to_owned()),
            FormDataEntryValue::File(value) => BackingEntryValue::File(value),
        };
        append_backing_entry(&backing_store, value, key);
    });
    backing_store
}

pub fn consume_backing_entries(
    backing_store: &BackingFormData,
    form_prefix: &str,
) -> &'static FormData {
    // Consumes the consecutive run of entries at the cursor whose keys start
    // with `formPrefix` into a new FormData, stripping the prefix from the
    // keys. The values of a referenced FormData are serialized as one
    // continuous block before its reference, so once a key doesn't match we
    // are past the values for this reference.
    let data = form_data::new(None, None);
    let entries = backing_store.referenced_entries.borrow();
    let Some(entries) = entries.as_ref() else {
        return data;
    };
    let mut i = backing_store.key_pointer.get();
    while i < entries.len() {
        let entry = &entries[i];
        let key = &entry.0;
        let Some(name) = key.strip_prefix(form_prefix) else {
            break;
        };
        match &entry.1 {
            BackingEntryValue::Str(value) => data.append(name, value),
            BackingEntryValue::File(value) => data.append(name, *value),
        }
        i += 1;
    }
    backing_store.key_pointer.set(i);
    data
}

pub fn get_backing_string(backing_store: &BackingFormData, key: &str) -> Option<String> {
    let string_chunks = backing_store.string_chunks.borrow();
    match string_chunks.as_ref() {
        None => None,
        Some(string_chunks) => string_chunks.get(key).cloned(),
    }
}

pub fn get_stream_chunks(backing_store: &BackingFormData, key: &str) -> Option<Vec<String>> {
    let stream_chunks = backing_store.stream_chunks.borrow();
    match stream_chunks.as_ref() {
        None => None,
        Some(stream_chunks) => stream_chunks.get(key).cloned(),
    }
}

pub fn get_backing_blob(backing_store: &BackingFormData, key: &str) -> Option<&'static Blob> {
    let blobs = backing_store.blobs.borrow();
    match blobs.as_ref() {
        None => None,
        Some(blobs) => blobs.get(key).copied(),
    }
}
