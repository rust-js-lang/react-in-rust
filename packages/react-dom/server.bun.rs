// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This file is only used for tests.
// It lazily loads the implementation so that we get the correct set of host configs.

use js::{IntoUnknown, Unknown, function};

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

mod react_dom_server_bun {
    use js::Unknown;

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.bun#renderToReadableStream"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn render_to_readable_stream() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.bun#resume"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn resume() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.bun#renderToPipeableStream"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn render_to_pipeable_stream() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.bun#resumeToPipeableStream"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn resume_to_pipeable_stream() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }
}

mod react_dom_legacy_server_browser {
    use js::Unknown;

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/ReactDOMLegacyServerBrowser#renderToString"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn render_to_string() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/ReactDOMLegacyServerBrowser#renderToStaticMarkup"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn render_to_static_markup() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }
}

pub fn render_to_readable_stream() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_bun::render_to_readable_stream(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn render_to_string() -> Option<&'static Unknown> {
    function::apply(
        react_dom_legacy_server_browser::render_to_string(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn render_to_static_markup() -> Option<&'static Unknown> {
    function::apply(
        react_dom_legacy_server_browser::render_to_static_markup(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn resume() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_bun::resume(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn render_to_pipeable_stream() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_bun::render_to_pipeable_stream(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn resume_to_pipeable_stream() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_bun::resume_to_pipeable_stream(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}
