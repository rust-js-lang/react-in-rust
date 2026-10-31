// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};
use react_dom_bindings::cross_origin_strings::{
    get_cross_origin_string, get_cross_origin_string_as,
};
use shared::environment::__DEV__;
use shared::react_dom_shared_internals::REACT_DOM_SHARED_INTERNALS;
use webapi::console;

use crate::react_dom_types::{
    PreinitModuleScriptOptions, PreinitScriptOptions, PreinitStyleOptions, PreloadImplOptions,
    PreloadModuleImplOptions,
};

// Each function is given what JS gives, a string or not, and tests it, as
// a caller may give anything: `href` a string and `options` one of
// ReactDOMTypes' options objects, by Flow's types.

#[cfg_attr(rust_js, rust_js::name = "prefetchDNS")]
pub fn prefetch_dns(
    href: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::rest)] args: &[Option<&'static Unknown>],
) {
    if __DEV__ {
        if !matches!(js::classify(href), Kind::String(href) if !href.is_empty()) {
            console::error((
                "ReactDOM.prefetchDNS(): Expected the `href` argument (first) to be a non-empty string but encountered %s instead.",
                get_value_descriptor_expecting_object_for_warning(href),
            ));
        } else if !args.is_empty() {
            let options = args[0];
            if matches!(
                js::classify(options),
                Kind::Null | Kind::Object(_) | Kind::Array(_)
            ) && options.unwrap().has_own_property("crossOrigin")
            {
                console::error((
                    "ReactDOM.prefetchDNS(): Expected only one argument, `href`, but encountered %s as a second argument instead. This argument is reserved for future options and is currently disallowed. It looks like the you are attempting to set a crossOrigin property for this DNS lookup hint. Browsers do not perform DNS queries using CORS and setting this attribute on the resource hint has no effect. Try calling ReactDOM.prefetchDNS() with just a single string argument, `href`.",
                    get_value_descriptor_expecting_enum_for_warning(options),
                ));
            } else {
                console::error((
                    "ReactDOM.prefetchDNS(): Expected only one argument, `href`, but encountered %s as a second argument instead. This argument is reserved for future options and is currently disallowed. Try calling ReactDOM.prefetchDNS() with just a single string argument, `href`.",
                    get_value_descriptor_expecting_enum_for_warning(options),
                ));
            }
        }
    }
    if let Kind::String(href) = js::classify(href) {
        (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
            .d()
            .prefetch_dns)(/* prefetchDNS */ href);
    }
    // We don't error because preconnect needs to be resilient to being called in a variety of scopes
    // and the runtime may not be capable of responding. The function is optimistic and not critical
    // so we favor silent bailout over warning or erroring.
}

pub fn preconnect(href: Option<&'static Unknown>, options: Option<&'static Unknown>) {
    if __DEV__ {
        if !matches!(js::classify(href), Kind::String(href) if !href.is_empty()) {
            console::error((
                "ReactDOM.preconnect(): Expected the `href` argument (first) to be a non-empty string but encountered %s instead.",
                get_value_descriptor_expecting_object_for_warning(href),
            ));
        } else if options.is_some()
            && !matches!(js::classify(options), Kind::Object(_) | Kind::Array(_))
        {
            console::error((
                "ReactDOM.preconnect(): Expected the `options` argument (second) to be an object but encountered %s instead. The only supported option at this time is `crossOrigin` which accepts a string.",
                get_value_descriptor_expecting_enum_for_warning(options),
            ));
        } else if options.is_some()
            && !matches!(
                js::classify(js::get(options, "crossOrigin")),
                Kind::String(_)
            )
        {
            console::error((
                "ReactDOM.preconnect(): Expected the `crossOrigin` option (second argument) to be a string but encountered %s instead. Try removing this option or passing a string value instead.",
                get_value_descriptor_expecting_object_for_warning(js::get(options, "crossOrigin")),
            ));
        }
    }
    if let Kind::String(href) = js::classify(href) {
        let cross_origin = if js::truthy(options) {
            get_cross_origin_string(js::get(options, "crossOrigin")).map(js::unknown)
        } else {
            js::null()
        };
        (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
            .d()
            .preconnect)(/* preconnect */ href, cross_origin);
    }
    // We don't error because preconnect needs to be resilient to being called in a variety of scopes
    // and the runtime may not be capable of responding. The function is optimistic and not critical
    // so we favor silent bailout over warning or erroring.
}

pub fn preload(href: Option<&'static Unknown>, options: Option<&'static Unknown>) {
    if __DEV__ {
        let mut encountered = String::new();
        if !matches!(js::classify(href), Kind::String(href) if !href.is_empty()) {
            encountered += &format!(
                " The `href` argument encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(href)
            );
        }
        if !matches!(js::classify(options), Kind::Object(_) | Kind::Array(_)) {
            encountered += &format!(
                " The `options` argument encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(options)
            );
        } else if !matches!(js::classify(js::get(options, "as")), Kind::String(r#as) if !r#as.is_empty())
        {
            encountered += &format!(
                " The `as` option encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(js::get(options, "as"))
            );
        }
        if !encountered.is_empty() {
            console::error((
                "ReactDOM.preload(): Expected two arguments, a non-empty `href` string and an `options` object with an `as` property valid for a `<link rel=\"preload\" as=\"...\" />` tag.%s",
                encountered,
            ));
        }
    }
    if let Kind::String(href) = js::classify(href)
        // We check existence because we cannot enforce this function is actually called with the stated type
        && let Some(options) = js::classify(options).object()
        && let Kind::String(r#as) = js::classify(js::get(options, "as"))
    {
        let cross_origin = get_cross_origin_string_as(Some(r#as), js::get(options, "crossOrigin"));
        (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
            .d()
            .preload)(
            /* preload */ href,
            r#as,
            Some(js::unknown_of(PreloadImplOptions {
                cross_origin,
                integrity: match js::classify(js::get(options, "integrity")) {
                    Kind::String(integrity) => Some(integrity),
                    _ => None,
                },
                nonce: match js::classify(js::get(options, "nonce")) {
                    Kind::String(nonce) => Some(nonce),
                    _ => None,
                },
                r#type: match js::classify(js::get(options, "type")) {
                    Kind::String(r#type) => Some(r#type),
                    _ => None,
                },
                fetch_priority: match js::classify(js::get(options, "fetchPriority")) {
                    Kind::String(fetch_priority) => Some(fetch_priority),
                    _ => None,
                },
                referrer_policy: match js::classify(js::get(options, "referrerPolicy")) {
                    Kind::String(referrer_policy) => Some(referrer_policy),
                    _ => None,
                },
                image_src_set: match js::classify(js::get(options, "imageSrcSet")) {
                    Kind::String(image_src_set) => Some(image_src_set),
                    _ => None,
                },
                image_sizes: match js::classify(js::get(options, "imageSizes")) {
                    Kind::String(image_sizes) => Some(image_sizes),
                    _ => None,
                },
                media: match js::classify(js::get(options, "media")) {
                    Kind::String(media) => Some(media),
                    _ => None,
                },
            })),
        );
    }
    // We don't error because preload needs to be resilient to being called in a variety of scopes
    // and the runtime may not be capable of responding. The function is optimistic and not critical
    // so we favor silent bailout over warning or erroring.
}

pub fn preload_module(href: Option<&'static Unknown>, options: Option<&'static Unknown>) {
    if __DEV__ {
        let mut encountered = String::new();
        if !matches!(js::classify(href), Kind::String(href) if !href.is_empty()) {
            encountered += &format!(
                " The `href` argument encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(href)
            );
        }
        if !matches!(
            js::classify(options),
            Kind::Undefined | Kind::Null | Kind::Object(_) | Kind::Array(_)
        ) {
            encountered += &format!(
                " The `options` argument encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(options)
            );
        } else if js::truthy(options)
            && js::has(options, "as")
            && !matches!(js::classify(js::get(options, "as")), Kind::String(_))
        {
            encountered += &format!(
                " The `as` option encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(js::get(options, "as"))
            );
        }
        if !encountered.is_empty() {
            console::error((
                "ReactDOM.preloadModule(): Expected two arguments, a non-empty `href` string and, optionally, an `options` object with an `as` property valid for a `<link rel=\"modulepreload\" as=\"...\" />` tag.%s",
                encountered,
            ));
        }
    }
    if let Kind::String(href) = js::classify(href) {
        if js::truthy(options) {
            let cross_origin = get_cross_origin_string_as(
                match js::classify(js::get(options, "as")) {
                    Kind::String(r#as) => Some(r#as),
                    _ => None,
                },
                js::get(options, "crossOrigin"),
            );
            (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
                .d()
                .m)(
                /* preloadModule */ href,
                Some(js::unknown_of(PreloadModuleImplOptions {
                    r#as: match js::classify(js::get(options, "as")) {
                        Kind::String(r#as) if r#as != "script" => Some(r#as),
                        _ => None,
                    },
                    cross_origin,
                    integrity: match js::classify(js::get(options, "integrity")) {
                        Kind::String(integrity) => Some(integrity),
                        _ => None,
                    },
                    nonce: match js::classify(js::get(options, "nonce")) {
                        Kind::String(nonce) => Some(nonce),
                        _ => None,
                    },
                    fetch_priority: match js::classify(js::get(options, "fetchPriority")) {
                        Kind::String(fetch_priority) => Some(fetch_priority),
                        _ => None,
                    },
                })),
            );
        } else {
            (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
                .d()
                .m)(/* preloadModule */ href, None);
        }
    }
    // We don't error because preload needs to be resilient to being called in a variety of scopes
    // and the runtime may not be capable of responding. The function is optimistic and not critical
    // so we favor silent bailout over warning or erroring.
}

pub fn preinit(href: Option<&'static Unknown>, options: Option<&'static Unknown>) {
    if __DEV__ {
        if !matches!(js::classify(href), Kind::String(href) if !href.is_empty()) {
            console::error((
                "ReactDOM.preinit(): Expected the `href` argument (first) to be a non-empty string but encountered %s instead.",
                get_value_descriptor_expecting_object_for_warning(href),
            ));
        } else if !matches!(js::classify(options), Kind::Object(_) | Kind::Array(_)) {
            console::error((
                "ReactDOM.preinit(): Expected the `options` argument (second) to be an object with an `as` property describing the type of resource to be preinitialized but encountered %s instead.",
                get_value_descriptor_expecting_enum_for_warning(options),
            ));
        } else if !matches!(
            js::classify(js::get(options, "as")),
            Kind::String("style" | "script")
        ) {
            console::error((
                "ReactDOM.preinit(): Expected the `as` property in the `options` argument (second) to contain a valid value describing the type of resource to be preinitialized but encountered %s instead. Valid values for `as` are \"style\" and \"script\".",
                get_value_descriptor_expecting_enum_for_warning(js::get(options, "as")),
            ));
        }
    }
    if let Kind::String(href) = js::classify(href)
        && js::truthy(options)
        && let Kind::String(r#as) = js::classify(js::get(options, "as"))
    {
        let cross_origin = get_cross_origin_string_as(Some(r#as), js::get(options, "crossOrigin"));
        let integrity = match js::classify(js::get(options, "integrity")) {
            Kind::String(integrity) => Some(integrity),
            _ => None,
        };
        let fetch_priority = match js::classify(js::get(options, "fetchPriority")) {
            Kind::String(fetch_priority) => Some(fetch_priority),
            _ => None,
        };
        if r#as == "style" {
            (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
                .d()
                .preinit_style)(
                /* preinitStyle */
                href,
                match js::classify(js::get(options, "precedence")) {
                    Kind::String(precedence) => Some(js::unknown(precedence)),
                    _ => None,
                },
                Some(js::unknown_of(PreinitStyleOptions {
                    cross_origin,
                    integrity,
                    fetch_priority,
                })),
            );
        } else if r#as == "script" {
            (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
                .d()
                .preinit_script)(
                /* preinitScript */ href,
                Some(js::unknown_of(PreinitScriptOptions {
                    cross_origin,
                    integrity,
                    fetch_priority,
                    nonce: match js::classify(js::get(options, "nonce")) {
                        Kind::String(nonce) => Some(nonce),
                        _ => None,
                    },
                })),
            );
        }
    }
    // We don't error because preinit needs to be resilient to being called in a variety of scopes
    // and the runtime may not be capable of responding. The function is optimistic and not critical
    // so we favor silent bailout over warning or erroring.
}

pub fn preinit_module(href: Option<&'static Unknown>, options: Option<&'static Unknown>) {
    if __DEV__ {
        let mut encountered = String::new();
        if !matches!(js::classify(href), Kind::String(href) if !href.is_empty()) {
            encountered += &format!(
                " The `href` argument encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(href)
            );
        }
        if !matches!(
            js::classify(options),
            Kind::Undefined | Kind::Null | Kind::Object(_) | Kind::Array(_)
        ) {
            encountered += &format!(
                " The `options` argument encountered was {}.",
                get_value_descriptor_expecting_object_for_warning(options)
            );
        } else if js::truthy(options)
            && js::has(options, "as")
            && !js::strict_equals(js::get(options, "as"), "script")
        {
            encountered += &format!(
                " The `as` option encountered was {}.",
                get_value_descriptor_expecting_enum_for_warning(js::get(options, "as"))
            );
        }
        if !encountered.is_empty() {
            console::error((
                "ReactDOM.preinitModule(): Expected up to two arguments, a non-empty `href` string and, optionally, an `options` object with a valid `as` property.%s",
                encountered,
            ));
        } else {
            let r#as = if js::truthy(options)
                && let Kind::String(r#as) = js::classify(js::get(options, "as"))
            {
                r#as
            } else {
                "script"
            };
            match r#as {
                "script" => {}

                // We have an invalid as type and need to warn
                _ => {
                    let type_of_as =
                        get_value_descriptor_expecting_enum_for_warning(Some(js::unknown(r#as)));
                    console::error((
                        concat!(
                            "ReactDOM.preinitModule(): Currently the only supported \"as\" type for this function is \"script\"",
                            " but received \"%s\" instead. This warning was generated for `href` \"%s\". In the future other",
                            " module types will be supported, aligning with the import-attributes proposal. Learn more here:",
                            " (https://github.com/tc39/proposal-import-attributes)"
                        ),
                        type_of_as,
                        href,
                    ));
                }
            }
        }
    }
    if let Kind::String(href) = js::classify(href) {
        if let Some(options) = js::classify(options).object() {
            let r#as = js::get(options, "as");
            if r#as.is_none() || js::strict_equals(r#as, "script") {
                let cross_origin = get_cross_origin_string_as(
                    match js::classify(r#as) {
                        Kind::String(r#as) => Some(r#as),
                        _ => None,
                    },
                    js::get(options, "crossOrigin"),
                );
                (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
                    .d()
                    .preinit_module_script)(
                    /* preinitModuleScript */ href,
                    Some(js::unknown_of(PreinitModuleScriptOptions {
                        cross_origin,
                        integrity: match js::classify(js::get(options, "integrity")) {
                            Kind::String(integrity) => Some(integrity),
                            _ => None,
                        },
                        nonce: match js::classify(js::get(options, "nonce")) {
                            Kind::String(nonce) => Some(nonce),
                            _ => None,
                        },
                        fetch_priority: match js::classify(js::get(options, "fetchPriority")) {
                            Kind::String(fetch_priority) => Some(fetch_priority),
                            _ => None,
                        },
                    })),
                );
            }
        } else if options.is_none() {
            (REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
                .d()
                .preinit_module_script)(/* preinitModuleScript */ href, None);
        }
    }
    // We don't error because preinit needs to be resilient to being called in a variety of scopes
    // and the runtime may not be capable of responding. The function is optimistic and not critical
    // so we favor silent bailout over warning or erroring.
}

fn get_value_descriptor_expecting_object_for_warning(thing: Option<&Unknown>) -> String {
    match js::classify(thing) {
        Kind::Null => "`null`".to_string(),
        Kind::Undefined => "`undefined`".to_string(),
        Kind::String("") => "an empty string".to_string(),
        _ => format!("something with type \"{}\"", js::type_of(thing)),
    }
}

fn get_value_descriptor_expecting_enum_for_warning(thing: Option<&Unknown>) -> String {
    match js::classify(thing) {
        Kind::Null => "`null`".to_string(),
        Kind::Undefined => "`undefined`".to_string(),
        Kind::String("") => "an empty string".to_string(),
        Kind::String(thing) => js::json::stringify(thing),
        Kind::Number(_) => format!("`{}`", js::string(thing)),
        _ => format!("something with type \"{}\"", js::type_of(thing)),
    }
}
