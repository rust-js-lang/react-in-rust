// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, PropertyDescriptor, ProxyHandler, SymbolFor, Unknown, WeakMap, function};

const TEMPORARY_REFERENCE_TAG: SymbolFor = SymbolFor("react.temporary.reference");

/// A value the client gave the server and only the client can read: a
/// function that throws if it's called, behind a proxy that throws if
/// it's read.
pub type TemporaryReference = js::Function;

pub type TemporaryReferenceSet = WeakMap<TemporaryReference, String>;

pub fn create_temporary_reference_set() -> &'static TemporaryReferenceSet {
    js::weak_map::new()
}

pub fn is_opaque_temporary_reference(reference: &'static Unknown) -> bool {
    SymbolFor::from_value(js::get(reference, "$$typeof")) == Some(TEMPORARY_REFERENCE_TAG)
}

pub fn resolve_temporary_reference(
    temporary_references: &TemporaryReferenceSet,
    temporary_reference: &TemporaryReference,
) -> Option<String> {
    temporary_references.get(temporary_reference)
}

thread_local! {
    static PROXY_HANDLERS: ProxyHandler = ProxyHandler {
        get: Some(Box::new(|target, name, receiver| {
            if let Kind::String(name) = js::classify(Some(name)) {
                match name {
                    // These names are read by the Flight runtime if you end up using the exports object.
                    "$$typeof" => {
                        // These names are a little too common. We should probably have a way to
                        // have the Flight runtime extract the inner target instead.
                        return js::get(target, "$$typeof");
                    }
                    "name" => return None,
                    "displayName" => return None,
                    // We need to special case this because createElement reads it if we pass this
                    // reference.
                    "defaultProps" => return None,
                    // React looks for debugInfo on thenables.
                    "_debugInfo" => return None,
                    // Avoid this attempting to be serialized.
                    "toJSON" => return None,
                    "Provider" => {
                        // Context.Provider === Context in React, so return the same reference.
                        // This allows server components to render <ClientContext.Provider>
                        // which will be serialized and executed on the client.
                        return Some(receiver);
                    }
                    "then" => {
                        // Allow returning a temporary reference from an async function
                        // Unlike regular Client References, a Promise would never have been serialized as
                        // an opaque Temporary Reference, but instead would have been serialized as a
                        // Promise on the server and so doesn't hit this path. So we can assume this wasn't
                        // a Promise on the client.
                        return None;
                    }
                    _ => {}
                }
            } else if js::strict_equals(Some(name), Some(js::unknown(js::symbol::to_primitive))) {
                // $FlowFixMe[prop-missing]
                return js::get(js::object::prototype, js::symbol::to_primitive);
            } else if js::strict_equals(Some(name), Some(js::unknown(js::symbol::to_string_tag))) {
                // $FlowFixMe[prop-missing]
                return js::get(js::object::prototype, js::symbol::to_string_tag);
            }
            panic!(
                // eslint-disable-next-line react-internal/safe-string-coercion
                "Cannot access {} on the server. You cannot dot into a temporary client reference from a server component. You can only pass the value through to the client.",
                js::string(Some(name))
            );
        })),
        set: Some(Box::new(|_, _, _, _| {
            panic!("Cannot assign to a temporary client reference from a server module.");
        })),
        ..Default::default()
    };
}

/// The descriptor of a temporary reference's tag, `{ $$typeof: { value } }`.
struct TagDescriptors<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: PropertyDescriptor<'a>,
}

// Safety: a struct of descriptors.
unsafe impl js::PropertyDescriptorMap for TagDescriptors<'_> {}

pub fn create_temporary_reference(
    temporary_references: &TemporaryReferenceSet,
    id: &str,
) -> &'static TemporaryReference {
    let reference: &'static TemporaryReference = js::object::define_properties(
        js::Function::of(function::with_this(|_, ()| -> Option<&'static Unknown> {
            panic!(
                "{}",
                concat!(
                    "Attempted to call a temporary Client Reference from the server but it is on the client. ",
                    "It's not possible to invoke a client function from the server, it can ",
                    "only be rendered as a Component or passed to props of a Client Component."
                )
            );
        })),
        &TagDescriptors {
            r#typeof: PropertyDescriptor {
                value: Some(&TEMPORARY_REFERENCE_TAG),
                ..Default::default()
            },
        },
    );
    let wrapper = PROXY_HANDLERS.with(|proxy_handlers| js::proxy::new(reference, proxy_handlers));
    register_temporary_reference(temporary_references, wrapper, id);
    wrapper
}

pub fn register_temporary_reference(
    temporary_references: &TemporaryReferenceSet,
    object: &'static TemporaryReference,
    id: &str,
) {
    temporary_references.set(object, id.to_owned());
}
