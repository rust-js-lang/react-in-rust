// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{
    Error, IntoUnknown, Kind, Promise, PropertyDescriptor, ProxyHandler, Symbol, SymbolFor,
    Unknown, WeakKey, function,
};
use shared::environment::__DEV__;
use webapi::console;

use react_server::react_flight_server::ReactClientValue;

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

/// A client module's export, or the module, as the server holds it.
pub struct ClientReference {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: &'static Symbol,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    pub id: String,
    #[cfg_attr(rust_js, rust_js::name = "$$async")]
    pub r#async: bool,
}

unsafe impl js::Defined for ClientReference {}
unsafe impl WeakKey for ClientReference {}

const CLIENT_REFERENCE_TAG: SymbolFor = SymbolFor("react.client.reference");
const SERVER_REFERENCE_TAG: SymbolFor = SymbolFor("react.server.reference");

pub fn is_client_reference(reference: &Unknown) -> bool {
    js::strict_equals(js::get(reference, "$$typeof"), CLIENT_REFERENCE_TAG)
}

pub fn is_server_reference(reference: &Unknown) -> bool {
    js::strict_equals(js::get(reference, "$$typeof"), SERVER_REFERENCE_TAG)
}

pub fn register_client_reference(
    proxy_implementation: &'static Unknown,
    id: &str,
    export_name: &str,
) -> &'static ClientReference {
    register_client_reference_impl(
        proxy_implementation,
        &(id.to_owned() + "#" + export_name),
        false,
    )
}

/// The properties a client reference is given.
struct ClientReferenceProperties<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    id: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$async")]
    r#async: PropertyDescriptor<'a>,
}

unsafe impl js::PropertyDescriptorMap for ClientReferenceProperties<'_> {}

fn register_client_reference_impl<T: js::AnyShape<M> + ?Sized, M>(
    proxy_implementation: &'static T,
    id: &str,
    r#async: bool,
) -> &'static ClientReference {
    // SAFETY: it has a client reference's properties now.
    unsafe {
        js::cast(Some(js::unknown(js::object::define_properties(
            proxy_implementation,
            &ClientReferenceProperties {
                r#typeof: PropertyDescriptor {
                    value: Some(&CLIENT_REFERENCE_TAG),
                    ..Default::default()
                },
                id: PropertyDescriptor {
                    value: Some(&id),
                    ..Default::default()
                },
                r#async: PropertyDescriptor {
                    value: Some(&r#async),
                    ..Default::default()
                },
            },
        ))))
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
    export_name: Option<&'static Unknown>,
) -> &'static ServerReference {
    let r#typeof = PropertyDescriptor {
        value: Some(&SERVER_REFERENCE_TAG),
        ..Default::default()
    };
    let id = PropertyDescriptor {
        value: Some(&match js::classify(export_name) {
            Kind::Null => id.to_owned(),
            _ => id.to_owned() + "#" + &js::string(export_name),
        }),
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

/// A server object, its own properties set by [`register_server_object_reference`].
pub struct ServerObjectReference {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: &'static Symbol,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    pub id: String,
}

/// The properties a server object reference is given.
struct ServerObjectReferenceProperties<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$id")]
    id: PropertyDescriptor<'a>,
}

unsafe impl js::PropertyDescriptorMap for ServerObjectReferenceProperties<'_> {}

pub fn register_server_object_reference(
    reference: &'static Unknown,
    id: &str,
    export_name: Option<&'static Unknown>,
) -> &'static ServerObjectReference {
    if __DEV__ && let Kind::Function(_) = js::classify(Some(reference)) {
        console::error((concat!(
            "registerServerObjectReference: The reference is a function. Use ",
            "registerServerReference to register a function as a Server ",
            "Reference."
        ),));
    }
    // SAFETY: it has a server object reference's properties now.
    unsafe {
        js::cast(Some(js::unknown(js::object::define_properties(
            reference,
            &ServerObjectReferenceProperties {
                r#typeof: PropertyDescriptor {
                    value: Some(&SERVER_REFERENCE_TAG),
                    ..Default::default()
                },
                id: PropertyDescriptor {
                    // GAP rd594: a conditional in a call's object literal is
                    // a `const value = ..` before the call, where upstream
                    // writes it in the literal.
                    value: Some(&match js::classify(export_name) {
                        Kind::Null => id.to_owned(),
                        _ => id.to_owned() + "#" + &js::string(export_name),
                    }),
                    configurable: Some(true),
                    ..Default::default()
                },
            },
        ))))
    }
}

thread_local! {
    static DEEP_PROXY_HANDLERS: ProxyHandler = ProxyHandler {
        get: Some(Box::new(|target, name, receiver| {
            if let Kind::String(name) = js::classify(Some(name)) {
                match name {
                    // These names are read by the Flight runtime if you end up using the exports object.
                    "$$typeof" => {
                        // These names are a little too common. We should probably have a way to
                        // have the Flight runtime extract the inner target instead.
                        return js::get(target, "$$typeof");
                    }
                    "$$id" => return js::get(target, "$$id"),
                    "$$async" => return js::get(target, "$$async"),
                    "name" => return js::get(target, "name"),
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
                    "then" => panic!(
                        "Cannot await or return from a thenable. You cannot await a client module from a server component."
                    ),
                    _ => {}
                }
            } else if js::strict_equals(name, js::symbol::to_primitive) {
                return js::get(js::object::prototype, js::symbol::to_primitive);
            } else if js::strict_equals(name, js::symbol::to_string_tag) {
                return js::get(js::object::prototype, js::symbol::to_string_tag);
            }
            // eslint-disable-next-line react-internal/safe-string-coercion
            let expression = js::string(js::get(target, "name")) + "." + &js::string(Some(name));
            panic!(
                "Cannot access {expression} on the server. You cannot dot into a client module from a server component. You can only pass the imported name through."
            );
        })),
        set: Some(Box::new(|_, _, _, _| {
            panic!("Cannot assign to a client module from a server module.");
        })),
        ..Default::default()
    };
}

// GAP rd589: upstream's one `switch (name)` of a string or a symbol is a
// `match` of the string's cases and an `if` of each symbol's, `if (typeof
// name === 'string') { switch (name) {..} } else if (name ===
// Symbol.toPrimitive) {..}`; `then`'s function has no name, `function
// (resolve, _reject)` where upstream's is `function then(resolve, reject)`.
fn get_reference(
    target: &'static ClientReference,
    name: &'static Unknown,
) -> Option<&'static Unknown> {
    if let Kind::String(name) = js::classify(Some(name)) {
        match name {
            // These names are read by the Flight runtime if you end up using the exports object.
            "$$typeof" => return Some(js::unknown(target.r#typeof)),
            "$$id" => return Some(js::unknown(&target.id)),
            "$$async" => return Some(js::unknown(&target.r#async)),
            "name" => return js::get(target, "name"),
            // We need to special case this because createElement reads it if we pass this
            // reference.
            "defaultProps" => return None,
            // React looks for debugInfo on thenables.
            "_debugInfo" => return None,
            // Avoid this attempting to be serialized.
            "toJSON" => return None,
            "__esModule" => {
                // Something is conditionally checking which export to use. We'll pretend to be
                // an ESM compat module but then we'll check again on the client.
                let module_id = target.id.clone();
                js::set(
                    target,
                    "default",
                    register_client_reference_impl(
                        function::with_this(move |_, ()| -> Option<&'static Unknown> {
                            panic!(
                                "Attempted to call the default export of {module_id} from the server but it's on the client. It's not possible to invoke a client function from the server, it can only be rendered as a Component or passed to props of a Client Component."
                            );
                        }),
                        &(target.id.clone() + "#"),
                        target.r#async,
                    ),
                );
                return Some(js::unknown(&true));
            }
            "then" => {
                if js::truthy(js::get(target, "then")) {
                    // Use a cached value
                    return js::get(target, "then");
                }
                if !target.r#async {
                    // If this module is expected to return a Promise (such as an AsyncModule) then
                    // we should resolve that with a client reference that unwraps the Promise on
                    // the client.

                    let client_reference: &ClientReference =
                        register_client_reference_impl(&ClientModule {}, &target.id, true);
                    let proxy = PROXY_HANDLERS
                        .with(|proxy_handlers| js::proxy::new(client_reference, proxy_handlers));

                    // Treat this as a resolved Promise for React's use()
                    js::set(target, "status", "fulfilled");
                    js::set(target, "value", proxy);

                    let then = register_client_reference_impl(
                        function::with_this(
                            move |_,
                                  (resolve, _reject): (
                                &'static Resolve,
                                Option<&'static Unknown>,
                            )| {
                                // Expose to React.
                                js::promise::resolve(resolve(proxy))
                            },
                        ),
                        // If this is not used as a Promise but is treated as a reference to a `.then`
                        // export then we should treat it as a reference to that name.
                        &(target.id.clone() + "#then"),
                        false,
                    );
                    js::set(target, "then", then);
                    return Some(js::unknown(then));
                } else {
                    // Since typeof .then === 'function' is a feature test we'd continue recursing
                    // indefinitely if we return a function. Instead, we return an object reference
                    // if we check further.
                    return None;
                }
            }
            _ => {}
        }
    } else if js::strict_equals(name, js::symbol::to_primitive) {
        return js::get(js::object::prototype, js::symbol::to_primitive);
    } else if js::strict_equals(name, js::symbol::to_string_tag) {
        return js::get(js::object::prototype, js::symbol::to_string_tag);
    }
    let Kind::String(name) = js::classify(Some(name)) else {
        panic!(
            "Cannot read Symbol exports. Only named exports are supported on a client module imported on the server."
        );
    };
    let mut cached_reference = js::get(target, name);
    if !js::truthy(cached_reference) {
        let reference = register_client_reference_impl(
            function::with_this(move |_, ()| -> Option<&'static Unknown> {
                // eslint-disable-next-line react-internal/safe-string-coercion
                panic!(
                    "Attempted to call {name}() from the server but {name} is on the client. It's not possible to invoke a client function from the server, it can only be rendered as a Component or passed to props of a Client Component."
                );
            }),
            &(target.id.clone() + "#" + name),
            target.r#async,
        );
        js::object::define_property(
            reference,
            "name",
            &PropertyDescriptor {
                value: Some(&name),
                ..Default::default()
            },
        );
        let proxy = DEEP_PROXY_HANDLERS
            .with(|deep_proxy_handlers| js::proxy::new(reference, deep_proxy_handlers));
        js::set(target, name, proxy);
        cached_reference = Some(js::unknown(proxy));
    }
    cached_reference
}

/// What a client module's `then` is given to resolve with.
type Resolve = dyn Fn(&ClientReference) -> Option<&'static Unknown>;

/// `{}`, what a client module's reference is made of.
struct ClientModule {}

unsafe impl js::Defined for ClientModule {}

thread_local! {
    static PROXY_HANDLERS: ProxyHandler = ProxyHandler {
        get: Some(Box::new(|target, name, _receiver| {
            // SAFETY: what's proxied is a client reference.
            get_reference(unsafe { js::cast(Some(target)) }, name)
        })),
        get_own_property_descriptor: Some(Box::new(|target, name| {
            let mut descriptor = js::object::get_own_property_descriptor(Some(target), name);
            if descriptor.is_none() {
                // SAFETY: what's proxied is a client reference.
                let value = get_reference(unsafe { js::cast(Some(target)) }, name);
                js::object::define_property(
                    target,
                    name,
                    descriptor.insert(PropertyDescriptor {
                        value: value.map(|value| value as &dyn IntoUnknown),
                        writable: Some(false),
                        configurable: Some(false),
                        enumerable: Some(false),
                        ..Default::default()
                    }),
                );
            }
            descriptor
        })),
        get_prototype_of: Some(Box::new(|_target| {
            // Pretend to be a Promise in case anyone asks.
            js::class::<Promise<()>>().prototype()
        })),
        set: Some(Box::new(|_, _, _, _| {
            panic!("Cannot assign to a client module from a server module.");
        })),
        ..Default::default()
    };
}

pub fn create_client_module_proxy(module_id: &str) -> &'static ClientReference {
    let client_reference: &ClientReference = register_client_reference_impl(
        &ClientModule {},
        // Represents the whole Module object instead of a particular import.
        module_id,
        false,
    );
    PROXY_HANDLERS.with(|proxy_handlers| js::proxy::new(client_reference, proxy_handlers))
}
