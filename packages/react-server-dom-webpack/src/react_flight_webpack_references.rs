//! ReactFlightWebpackReferences, its JS's until it's ported (held on rd580:
//! its client module proxy's `getPrototypeOf` gives `Promise.prototype`,
//! which js doesn't bind). What the server's modules read of it, and
//! re-export. Bindings alone, so no JS of its own.

use std::marker::PhantomData;

use js::{Error, JsObject, Unknown};

use crate::server::react_flight_server::ReactClientValue;

/// A server function, its own properties set by `registerServerReference`.
pub struct ServerReference(PhantomData<JsObject>);

unsafe impl js::Defined for ServerReference {}

impl ServerReference {
    #[cfg_attr(rust_js, rust_js::link_name = "get $$id")]
    pub fn id(&self) -> String {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get $$bound")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn bound(&self) -> Option<&'static [Option<&'static ReactClientValue>]> {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get $$location")]
    pub fn location(&self) -> Option<&'static Error> {
        unreachable!()
    }
}

/// A client module's export, or the module, as the server holds it.
pub struct ClientReference(PhantomData<JsObject>);

unsafe impl js::Defined for ClientReference {}

impl ClientReference {
    #[cfg_attr(rust_js, rust_js::link_name = "get $$id")]
    pub fn id(&self) -> String {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get $$async")]
    pub fn r#async(&self) -> bool {
        unreachable!()
    }
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./ReactFlightWebpackReferences.js#isClientReference"
)]
pub fn is_client_reference(reference: &Unknown) -> bool {
    let _ = reference;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./ReactFlightWebpackReferences.js#isServerReference"
)]
pub fn is_server_reference(reference: &Unknown) -> bool {
    let _ = reference;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./ReactFlightWebpackReferences.js#registerClientReference"
)]
pub fn register_client_reference(
    proxy_implementation: &'static Unknown,
    id: &str,
    export_name: &str,
) -> &'static ClientReference {
    let _ = (proxy_implementation, id, export_name);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./ReactFlightWebpackReferences.js#registerServerReference"
)]
pub fn register_server_reference(
    reference: &'static Unknown,
    id: &str,
    export_name: Option<&'static Unknown>,
) -> &'static ServerReference {
    let _ = (reference, id, export_name);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./ReactFlightWebpackReferences.js#createClientModuleProxy"
)]
pub fn create_client_module_proxy(module_id: &str) -> &'static ClientReference {
    let _ = module_id;
    unreachable!()
}
