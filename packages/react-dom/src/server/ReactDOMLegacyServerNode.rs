// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use crate::react_dom_legacy_server_impl::VERSION;
use crate::react_dom_legacy_server_impl::{ServerOptions, render_to_string_impl};

pub fn render_to_string(
    children: Option<&'static Unknown>,
    options: Option<&ServerOptions>,
) -> String {
    render_to_string_impl(
        children,
        options,
        false,
        "The server used \"renderToString\" which does not support Suspense. If you intended for this Suspense boundary to render the fallback content on the server consider throwing an Error somewhere within the Suspense boundary. If you intended to have the server wait for the suspended component please switch to \"renderToPipeableStream\" which supports Suspense on the server",
    )
}

pub fn render_to_static_markup(
    children: Option<&'static Unknown>,
    options: Option<&ServerOptions>,
) -> String {
    render_to_string_impl(
        children,
        options,
        true,
        "The server used \"renderToStaticMarkup\" which does not support Suspense. If you intended to have the server wait for the suspended component please switch to \"renderToPipeableStream\" which supports Suspense on the server",
    )
}
