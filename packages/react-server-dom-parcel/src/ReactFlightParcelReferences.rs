// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, Error, IntoUnknown, PropertyDescriptor, Symbol, SymbolFor, Unknown, function};
use shared::environment::__DEV__;
use webapi::console;

use react_server::react_flight_hooks::ReactClientValue;

/// A server function, its own properties set by [`register_server_reference`].
pub struct ServerReference {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: &'static Symbol,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    pub id: String,
    #[cfg_attr(rust_js, rust_js::name = "$$bound")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub bound: Option<&'static [Option<&'static ReactClientValue>]>,
    #[cfg_attr(rust_js, rust_js::name = "$$location")]
    pub location: Option<&'static Error>,
}

/// A client module's export, as the server holds it: its module, its
/// name, the bundles that load it and the import map they need.
pub struct ClientReference {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    pub id: String,
    #[cfg_attr(rust_js, rust_js::name = "$$name")]
    pub name: String,
    #[cfg_attr(rust_js, rust_js::name = "$$bundles")]
    pub bundles: &'static [String],
    // GAP rd595: declared `| undefined` alone, though it holds the `null`
    // createClientReference may be given.
    #[cfg_attr(rust_js, rust_js::name = "$$importMap")]
    pub import_map: Option<&'static Dict<String>>,
}

const CLIENT_REFERENCE_TAG: SymbolFor = SymbolFor("react.client.reference");
const SERVER_REFERENCE_TAG: SymbolFor = SymbolFor("react.server.reference");

pub fn is_client_reference(reference: &Unknown) -> bool {
    js::strict_equals(js::get(reference, "$$typeof"), CLIENT_REFERENCE_TAG)
}

pub fn is_server_reference(reference: &Unknown) -> bool {
    js::strict_equals(js::get(reference, "$$typeof"), SERVER_REFERENCE_TAG)
}

pub fn create_client_reference(
    id: &str,
    export_name: &str,
    bundles: &'static [String],
    import_map: Option<&'static Dict<String>>,
) -> ClientReference {
    ClientReference {
        r#typeof: CLIENT_REFERENCE_TAG,
        id: id.to_owned(),
        name: export_name.to_owned(),
        bundles,
        import_map,
    }
}

#[cfg_attr(rust_js, rust_js::name = "ArraySlice")]
use js::array::SLICE as ARRAY_SLICE;
#[cfg_attr(rust_js, rust_js::name = "FunctionBind")]
use js::function::BIND as FUNCTION_BIND;

/// The properties `bind` gives a server reference's bound function in DEV.
struct DevBoundServerReferenceProperties<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    id: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$bound")]
    bound: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$location")]
    location: PropertyDescriptor<'a>,
    bind: PropertyDescriptor<'a>,
}

unsafe impl js::PropertyDescriptorMap for DevBoundServerReferenceProperties<'_> {}

/// The properties `bind` gives a server reference's bound function.
struct BoundServerReferenceProperties<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    id: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$bound")]
    bound: PropertyDescriptor<'a>,
    bind: PropertyDescriptor<'a>,
}

unsafe impl js::PropertyDescriptorMap for BoundServerReferenceProperties<'_> {}

thread_local! {
    static BIND: &'static dyn Fn() -> Option<&'static Unknown> = function::with_this(|this, ()| {
        // SAFETY: `bind` makes a function.
        let new_fn: &'static Unknown = unsafe {
            js::cast(function::apply(FUNCTION_BIND, this.map(|this| this as &dyn IntoUnknown), Some(js::arguments())))
        };
        // SAFETY: it's only a function's `bind`, called on it: a server
        // reference's where its `$$typeof` says so.
        let reference: &ServerReference = unsafe { js::cast(this) };
        if js::strict_equals(reference.r#typeof, SERVER_REFERENCE_TAG) {
            if __DEV__ {
                let this_bind = js::get(js::arguments(), &0.0);
                if !js::loose_equals(this_bind, js::null::<&Unknown>()) {
                    console::error((
                        "Cannot bind \"this\" of a Server Action. Pass null or undefined as the first argument to .bind().",
                    ));
                }
            }
            // SAFETY: `slice` makes an array.
            let args: Vec<Option<&'static Unknown>> =
                unsafe { js::cast(function::call(ARRAY_SLICE, Some(js::arguments()), (1.0,))) };
            let r#typeof = PropertyDescriptor { value: Some(&SERVER_REFERENCE_TAG), ..Default::default() };
            let id = PropertyDescriptor { value: Some(&reference.id), ..Default::default() };
            let bound = PropertyDescriptor {
                value: Some(&match reference.bound {
                    Some(bound) => [bound, &args].concat(),
                    None => args,
                }),
                ..Default::default()
            };
            return Some(if __DEV__ {
                js::object::define_properties(
                    new_fn,
                    &DevBoundServerReferenceProperties {
                        r#typeof,
                        id,
                        bound,
                        location: PropertyDescriptor {
                            value: Some(&reference.location),
                            configurable: Some(true),
                            ..Default::default()
                        },
                        bind: PropertyDescriptor {
                            value: Some(&BIND.with(|&bind| bind)),
                            configurable: Some(true),
                            ..Default::default()
                        },
                    },
                )
            } else {
                js::object::define_properties(
                    new_fn,
                    &BoundServerReferenceProperties {
                        r#typeof,
                        id,
                        bound,
                        bind: PropertyDescriptor {
                            value: Some(&BIND.with(|&bind| bind)),
                            configurable: Some(true),
                            ..Default::default()
                        },
                    },
                )
            });
        }
        Some(new_fn)
    });
}

thread_local! {
    static SERVER_REFERENCE_TO_STRING: PropertyDescriptor<'static> = PropertyDescriptor {
        value: Some(&|| "function () { [omitted code] }"),
        configurable: Some(true),
        writable: Some(true),
        ..Default::default()
    };
}

/// The properties a server reference is given in DEV.
struct DevServerReferenceProperties<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    id: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$bound")]
    bound: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$location")]
    location: PropertyDescriptor<'a>,
    bind: PropertyDescriptor<'a>,
    to_string: PropertyDescriptor<'a>,
}

unsafe impl js::PropertyDescriptorMap for DevServerReferenceProperties<'_> {}

/// The properties a server reference is given.
struct ServerReferenceProperties<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    id: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$bound")]
    bound: PropertyDescriptor<'a>,
    bind: PropertyDescriptor<'a>,
    to_string: PropertyDescriptor<'a>,
}

unsafe impl js::PropertyDescriptorMap for ServerReferenceProperties<'_> {}

pub fn register_server_reference(
    reference: &'static Unknown,
    id: &str,
    export_name: &str,
) -> &'static ServerReference {
    let r#typeof = PropertyDescriptor {
        value: Some(&SERVER_REFERENCE_TAG),
        ..Default::default()
    };
    let id = PropertyDescriptor {
        value: Some(&(id.to_owned() + "#" + export_name)),
        configurable: Some(true),
        ..Default::default()
    };
    let bound = PropertyDescriptor {
        value: Some(&js::null::<&Unknown>()),
        configurable: Some(true),
        ..Default::default()
    };
    SERVER_REFERENCE_TO_STRING.with(|&to_string| {
        // SAFETY: it has a server reference's properties now.
        unsafe {
            js::cast(Some(js::unknown(if __DEV__ {
                js::object::define_properties(
                    reference,
                    &DevServerReferenceProperties {
                        r#typeof,
                        id,
                        bound,
                        location: PropertyDescriptor {
                            value: Some(&js::error::new(Some("react-stack-top-frame"), None)),
                            configurable: Some(true),
                            ..Default::default()
                        },
                        bind: PropertyDescriptor {
                            value: Some(&BIND.with(|&bind| bind)),
                            configurable: Some(true),
                            ..Default::default()
                        },
                        to_string,
                    },
                )
            } else {
                js::object::define_properties(
                    reference,
                    &ServerReferenceProperties {
                        r#typeof,
                        id,
                        bound,
                        bind: PropertyDescriptor {
                            value: Some(&BIND.with(|&bind| bind)),
                            configurable: Some(true),
                            ..Default::default()
                        },
                        to_string,
                    },
                )
            })))
        }
    })
}
