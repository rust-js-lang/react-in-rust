// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Dict;
use react_dom_bindings::cross_origin_strings::CrossOriginString;

pub use shared::react_dom_shared_internals::HostDispatcher;

pub struct PrefetchDNSOptions {}

pub struct PreconnectOptions {
    pub cross_origin: Option<String>,
}

pub struct PreloadOptions {
    pub r#as: String,
    pub cross_origin: Option<String>,
    pub integrity: Option<String>,
    pub r#type: Option<String>,
    pub media: Option<String>,
    pub nonce: Option<String>,
    pub fetch_priority: Option<FetchPriorityEnum>,
    pub image_src_set: Option<String>,
    pub image_sizes: Option<String>,
    pub referrer_policy: Option<String>,
}

pub struct PreloadModuleOptions {
    pub r#as: Option<String>,
    pub cross_origin: Option<String>,
    pub integrity: Option<String>,
    pub nonce: Option<String>,
    pub fetch_priority: Option<FetchPriorityEnum>,
}

pub struct PreinitOptions {
    pub r#as: String,
    pub precedence: Option<String>,
    pub cross_origin: Option<String>,
    pub integrity: Option<String>,
    pub nonce: Option<String>,
    pub fetch_priority: Option<FetchPriorityEnum>,
}

pub struct PreinitModuleOptions {
    pub r#as: Option<String>,
    pub cross_origin: Option<String>,
    pub integrity: Option<String>,
    pub nonce: Option<String>,
    pub fetch_priority: Option<FetchPriorityEnum>,
}

/// `'' | 'use-credentials' | CrossOriginString`.
pub type CrossOriginEnum = CrossOriginString;

pub enum FetchPriorityEnum {
    #[cfg_attr(rust_js, rust_js::name = "high")]
    High,
    #[cfg_attr(rust_js, rust_js::name = "low")]
    Low,
    #[cfg_attr(rust_js, rust_js::name = "auto")]
    Auto,
}

pub struct PreloadImplOptions<'a> {
    pub cross_origin: Option<&'a CrossOriginEnum>,
    pub integrity: Option<&'a str>,
    pub nonce: Option<&'a str>,
    pub r#type: Option<&'a str>,
    pub fetch_priority: Option<&'a str>,
    pub referrer_policy: Option<&'a str>,
    pub image_src_set: Option<&'a str>,
    pub image_sizes: Option<&'a str>,
    pub media: Option<&'a str>,
}

unsafe impl js::Defined for PreloadImplOptions<'_> {}

pub struct PreloadModuleImplOptions<'a> {
    pub r#as: Option<&'a str>,
    pub cross_origin: Option<&'a CrossOriginEnum>,
    pub integrity: Option<&'a str>,
    pub nonce: Option<&'a str>,
    pub fetch_priority: Option<&'a str>,
}

unsafe impl js::Defined for PreloadModuleImplOptions<'_> {}

pub struct PreinitStyleOptions<'a> {
    pub cross_origin: Option<&'a CrossOriginEnum>,
    pub integrity: Option<&'a str>,
    pub fetch_priority: Option<&'a str>,
}

unsafe impl js::Defined for PreinitStyleOptions<'_> {}

pub struct PreinitScriptOptions<'a> {
    pub cross_origin: Option<&'a CrossOriginEnum>,
    pub integrity: Option<&'a str>,
    pub fetch_priority: Option<&'a str>,
    pub nonce: Option<&'a str>,
}

unsafe impl js::Defined for PreinitScriptOptions<'_> {}

pub struct PreinitModuleScriptOptions<'a> {
    pub cross_origin: Option<&'a CrossOriginEnum>,
    pub integrity: Option<&'a str>,
    pub nonce: Option<&'a str>,
    pub fetch_priority: Option<&'a str>,
}

unsafe impl js::Defined for PreinitModuleScriptOptions<'_> {}

pub struct ImportMap {
    pub imports: Option<&'static Dict<String>>,
    pub scopes: Option<&'static Dict<&'static Dict<String>>>,
}
