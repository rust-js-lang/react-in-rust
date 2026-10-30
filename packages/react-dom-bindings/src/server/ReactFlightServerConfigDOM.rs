// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::collections::HashSet;

use js::{Kind, Unknown, string};

use crate::cross_origin_strings::{CrossOriginString, get_cross_origin_string};

// This module registers the host dispatcher so it needs to be imported
// even if no exports are used.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/server/ReactDOMFlightServerHostDispatcher.js#preload"
)]
fn preload(href: Option<&Unknown>, r#as: Option<&Unknown>, options: &Unknown) {
    let _ = (href, r#as, options);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/server/ReactDOMFlightServerHostDispatcher.js#preloadModule"
)]
fn preload_module(href: Option<&Unknown>, options: &Unknown) {
    let _ = (href, options);
    unreachable!()
}

pub type Hints = HashSet<String>;

pub fn create_hints() -> Hints {
    HashSet::new()
}

const NO_SCOPE: FormatContext = /*         */ 0b000000;
const NOSCRIPT_SCOPE: FormatContext = /*   */ 0b000001;
const PICTURE_SCOPE: FormatContext = /*    */ 0b000010;

pub type FormatContext = i32;

pub fn create_root_format_context() -> FormatContext {
    NO_SCOPE
}

/// What `processImg` preloads an image with.
struct ImageOptions<'a> {
    image_src_set: Option<&'a Unknown>,
    image_sizes: Option<&'a Unknown>,
    cross_origin: Option<&'a CrossOriginString>,
    integrity: Option<&'a Unknown>,
    r#type: Option<&'a Unknown>,
    fetch_priority: Option<&'a Unknown>,
    referrer_policy: Option<&'a Unknown>,
}

unsafe impl js::Defined for ImageOptions<'_> {}

/// What a `<link rel="preload">` preloads with.
struct PreloadOptions<'a> {
    cross_origin: Option<&'a Unknown>,
    integrity: Option<&'a Unknown>,
    nonce: Option<&'a Unknown>,
    r#type: Option<&'a Unknown>,
    fetch_priority: Option<&'a Unknown>,
    referrer_policy: Option<&'a Unknown>,
    image_src_set: Option<&'a Unknown>,
    image_sizes: Option<&'a Unknown>,
    media: Option<&'a Unknown>,
}

unsafe impl js::Defined for PreloadOptions<'_> {}

/// What a `<link rel="modulepreload">` preloads with.
struct ModuleOptions<'a> {
    r#as: Option<&'a Unknown>,
    cross_origin: Option<&'a Unknown>,
    integrity: Option<&'a Unknown>,
    nonce: Option<&'a Unknown>,
}

unsafe impl js::Defined for ModuleOptions<'_> {}

/// What a `<link rel="stylesheet">` preloads with.
struct StyleOptions<'a> {
    cross_origin: Option<&'a Unknown>,
    integrity: Option<&'a Unknown>,
    nonce: Option<&'a Unknown>,
    r#type: Option<&'a Unknown>,
    fetch_priority: Option<&'a Unknown>,
    referrer_policy: Option<&'a Unknown>,
    media: Option<&'a Unknown>,
}

unsafe impl js::Defined for StyleOptions<'_> {}

fn process_img(props: &Unknown, format_context: FormatContext) {
    // This should mirror the logic of pushImg in ReactFizzConfigDOM.
    let picture_or_no_script_tag_in_scope = format_context & (PICTURE_SCOPE | NOSCRIPT_SCOPE);
    let src = js::get(props, "src");
    let src_set = js::get(props, "srcSet");
    if !matches!(js::classify(js::get(props, "loading")), Kind::String("lazy"))
        && (js::truthy(src) || js::truthy(src_set))
        && (matches!(js::classify(src), Kind::String(_)) || src.is_none())
        && (matches!(js::classify(src_set), Kind::String(_)) || src_set.is_none())
        && !matches!(js::classify(js::get(props, "fetchPriority")), Kind::String("low"))
        && !(picture_or_no_script_tag_in_scope != 0)
        // We exclude data URIs in src and srcSet since these should not be preloaded
        && !(if let Kind::String(src) = js::classify(src) {
            string::get(src, 4.0) == Some(":")
                && (string::get(src, 0.0) == Some("d") || string::get(src, 0.0) == Some("D"))
                && (string::get(src, 1.0) == Some("a") || string::get(src, 1.0) == Some("A"))
                && (string::get(src, 2.0) == Some("t") || string::get(src, 2.0) == Some("T"))
                && (string::get(src, 3.0) == Some("a") || string::get(src, 3.0) == Some("A"))
        } else {
            false
        })
        && !(if let Kind::String(src_set) = js::classify(src_set) {
            string::get(src_set, 4.0) == Some(":")
                && (string::get(src_set, 0.0) == Some("d") || string::get(src_set, 0.0) == Some("D"))
                && (string::get(src_set, 1.0) == Some("a") || string::get(src_set, 1.0) == Some("A"))
                && (string::get(src_set, 2.0) == Some("t") || string::get(src_set, 2.0) == Some("T"))
                && (string::get(src_set, 3.0) == Some("a") || string::get(src_set, 3.0) == Some("A"))
        } else {
            false
        })
    {
        // We have a suspensey image and ought to preload it to optimize the loading of display blocking
        // resumableState.
        let sizes = match js::classify(js::get(props, "sizes")) {
            Kind::String(_) => js::get(props, "sizes"),
            _ => None,
        };

        let cross_origin = get_cross_origin_string(js::get(props, "crossOrigin"));

        preload(
            // The preload() API requires a href but if we have an imageSrcSet then that will take precedence.
            // We already remove the href anyway in both Fizz and Fiber due to a Safari bug so the empty string
            // will never actually appear in the DOM.
            js::or(src, Some(js::unknown(""))),
            Some(js::unknown("image")),
            js::unknown(&ImageOptions {
                image_src_set: src_set,
                image_sizes: sizes,
                cross_origin,
                integrity: js::get(props, "integrity"),
                r#type: js::get(props, "type"),
                fetch_priority: js::get(props, "fetchPriority"),
                referrer_policy: js::get(props, "referrerPolicy"),
            }),
        );
    }
}

fn process_link(props: &Unknown, format_context: FormatContext) {
    let noscript_tag_in_scope = format_context & NOSCRIPT_SCOPE;
    let rel = js::get(props, "rel");
    let href = js::get(props, "href");
    if noscript_tag_in_scope != 0
        || js::get(props, "itemProp").is_some()
        || !matches!(js::classify(rel), Kind::String(_))
        || !matches!(js::classify(href), Kind::String(_))
        || matches!(js::classify(href), Kind::String(""))
    {
        // We shouldn't preload resources that are in noscript or have no configuration.
        return;
    }

    match js::classify(rel) {
        Kind::String("preload") => {
            preload(
                href,
                js::get(props, "as"),
                js::unknown(&PreloadOptions {
                    cross_origin: js::get(props, "crossOrigin"),
                    integrity: js::get(props, "integrity"),
                    nonce: js::get(props, "nonce"),
                    r#type: js::get(props, "type"),
                    fetch_priority: js::get(props, "fetchPriority"),
                    referrer_policy: js::get(props, "referrerPolicy"),
                    image_src_set: js::get(props, "imageSrcSet"),
                    image_sizes: js::get(props, "imageSizes"),
                    media: js::get(props, "media"),
                }),
            );
        }
        Kind::String("modulepreload") => {
            preload_module(
                href,
                js::unknown(&ModuleOptions {
                    r#as: js::get(props, "as"),
                    cross_origin: js::get(props, "crossOrigin"),
                    integrity: js::get(props, "integrity"),
                    nonce: js::get(props, "nonce"),
                }),
            );
        }
        Kind::String("stylesheet") => {
            preload(
                href,
                Some(js::unknown("style")),
                js::unknown(&StyleOptions {
                    cross_origin: js::get(props, "crossOrigin"),
                    integrity: js::get(props, "integrity"),
                    nonce: js::get(props, "nonce"),
                    r#type: js::get(props, "type"),
                    fetch_priority: js::get(props, "fetchPriority"),
                    referrer_policy: js::get(props, "referrerPolicy"),
                    media: js::get(props, "media"),
                }),
            );
        }
        _ => {}
    }
}

pub fn get_child_format_context(
    parent_context: FormatContext,
    r#type: &str,
    props: &Unknown,
) -> FormatContext {
    match r#type {
        "img" => {
            process_img(props, parent_context);
            parent_context
        }
        "link" => {
            process_link(props, parent_context);
            parent_context
        }
        "picture" => parent_context | PICTURE_SCOPE,
        "noscript" => parent_context | NOSCRIPT_SCOPE,
        _ => parent_context,
    }
}
