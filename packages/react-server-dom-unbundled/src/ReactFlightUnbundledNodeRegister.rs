// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{IntoUnknown, Kind, Unknown, function, object};
use node::module::{MODULE, Module};
use node::url;
use webapi::console;

use acorn_loose::{EcmaVersion, Node, Options, SourceType};

/// The server's `registerServerReference`, any function's: of the function,
/// its module's id and its export's name, `null` of the whole module.
type RegisterServerReference = dyn Fn(&'static Unknown, &str, Option<&Unknown>);

/// The server's `createClientModuleProxy`: the proxy of a client module's
/// exports, of its id.
type CreateClientModuleProxy = dyn Fn(&str) -> &'static Unknown;

/// `require('react-server-dom-unbundled/server').registerServerReference`,
/// read where `register` is called, as upstream requires the server then.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "get react-server-dom-unbundled/server#registerServerReference"
)]
#[cfg_attr(rust_js, rust_js::require)]
fn server_register_server_reference() -> &'static RegisterServerReference {
    unreachable!()
}

/// `require('react-server-dom-unbundled/server').createClientModuleProxy`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "get react-server-dom-unbundled/server#createClientModuleProxy"
)]
#[cfg_attr(rust_js, rust_js::require)]
fn server_create_client_module_proxy() -> &'static CreateClientModuleProxy {
    unreachable!()
}

js::export_default! {
    fn register() {
        let register_server_reference = server_register_server_reference();
        let create_client_module_proxy = server_create_client_module_proxy();

        // $FlowFixMe[prop-missing] found when upgrading Flow
        // SAFETY: Node's `Module.prototype._compile` is a function.
        let original_compile: &'static dyn Fn() -> Option<&'static Unknown> =
            unsafe { js::cast(js::get(MODULE.prototype(), "_compile")) };

        // $FlowFixMe[prop-missing] found when upgrading Flow
        js::set(
            MODULE.prototype(),
            "_compile",
            function::with_this(move |this, (content, filename): (&str, &str)| {
                let this_arg = this.map(|this| this as &dyn IntoUnknown);
                // Do a quick check for the exact string. If it doesn't exist, don't
                // bother parsing.
                if !content.contains("use client") && !content.contains("use server") {
                    return function::apply(original_compile, this_arg, Some(js::arguments()));
                }

                let body = match acorn_loose::parse(
                    content,
                    &Options {
                        ecma_version: EcmaVersion::Version(2024),
                        source_type: Some(SourceType::Script),
                        locations: None,
                        on_comment: None,
                        preserve_parens: None,
                    },
                ) {
                    Ok(program) => program.body,
                    Err(x) => {
                        console::error(("Error parsing %s %s", filename, &*x.message()));
                        return function::apply(original_compile, this_arg, Some(js::arguments()));
                    }
                };

                let mut use_client = false;
                let mut use_server = false;
                for node in body {
                    let Node::ExpressionStatement {
                        directive: Some(directive),
                    } = node
                    else {
                        break;
                    };
                    if directive == "use client" {
                        use_client = true;
                    }
                    if directive == "use server" {
                        use_server = true;
                    }
                }

                if !use_client && !use_server {
                    return function::apply(original_compile, this_arg, Some(js::arguments()));
                }

                if use_client && use_server {
                    panic!(
                        "Cannot have both \"use client\" and \"use server\" directives in the same file."
                    );
                }

                if use_client {
                    let module_id = url::path_to_file_url(filename, None).href();
                    // SAFETY: Node calls `_compile` on the module it compiles.
                    unsafe { js::cast::<&Module>(this) }
                        .set_exports(Some(create_client_module_proxy(&module_id)));
                }

                if use_server {
                    function::apply(original_compile, this_arg, Some(js::arguments()));

                    let module_id = url::path_to_file_url(filename, None).href();

                    // SAFETY: Node calls `_compile` on the module it compiles.
                    let exports = unsafe { js::cast::<&Module>(this) }.exports();

                    // This module is imported server to server, but opts in to exposing functions by
                    // reference. If there are any functions in the export.
                    if let Kind::Function(_) = js::classify(exports) {
                        // The module exports a function directly,
                        register_server_reference(
                            exports.unwrap(),
                            &module_id,
                            // Represents the whole Module object instead of a particular import.
                            js::null(),
                        );
                    } else {
                        let keys = object::keys(exports.unwrap());
                        for key in &keys {
                            let value = js::get(exports, key.as_str());
                            if let Kind::Function(_) = js::classify(value) {
                                register_server_reference(
                                    value.unwrap(),
                                    &module_id,
                                    Some(js::unknown(key.as_str())),
                                );
                            }
                        }
                    }
                }
                None
            }),
        );
    }
}
