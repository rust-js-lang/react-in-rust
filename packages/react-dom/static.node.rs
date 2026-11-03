// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This file is only used for tests.
// It lazily loads the implementation so that we get the correct set of host configs.

use js::{IntoUnknown, Unknown, function};

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

mod react_dom_server_node {
    use js::Unknown;

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.node#prerenderToNodeStream"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn prerender_to_node_stream() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.node#prerender"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn prerender() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.node#resumeAndPrerenderToNodeStream"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn resume_and_prerender_to_node_stream() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get ./src/server/react-dom-server.node#resumeAndPrerender"
    )]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn resume_and_prerender() -> &'static dyn Fn() -> Option<&'static Unknown> {
        unreachable!()
    }
}

pub fn prerender_to_node_stream() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_node::prerender_to_node_stream(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn prerender() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_node::prerender(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn resume_and_prerender_to_node_stream() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_node::resume_and_prerender_to_node_stream(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}

pub fn resume_and_prerender() -> Option<&'static Unknown> {
    function::apply(
        react_dom_server_node::resume_and_prerender(),
        js::this().map(|this| this as &dyn IntoUnknown),
        Some(js::arguments()),
    )
}
