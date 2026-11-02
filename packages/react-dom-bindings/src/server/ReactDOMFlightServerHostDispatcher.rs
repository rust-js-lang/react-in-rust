// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, Kind, Unknown, dict};

use react_server::react_flight_server::{emit_hint, get_hints, resolve_request};

use shared::react_dom_shared_internals::{HostDispatcher, REACT_DOM_SHARED_INTERNALS};

use crate::react_flight_server_config_dom::Hints;

thread_local! {
    static PREVIOUS_DISPATCHER: &'static HostDispatcher =
        REACT_DOM_SHARED_INTERNALS.d(); /* ReactDOMCurrentDispatcher */
}

js::on_load! {
    REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
        .set_d(HostDispatcher {
            f /* flushSyncWork */: PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.f), /* flushSyncWork */
            r /* requestFormReset */: PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.r), /* requestFormReset */
            prefetch_dns: &prefetch_dns,
            preconnect: &preconnect,
            preload: &|href, r#as, options| {
                preload(Some(js::unknown(href)), Some(js::unknown_of(r#as.to_owned())), options);
            },
            m: &|href, options| preload_module(Some(js::unknown(href)), options),
            preinit_script: &preinit_script,
            preinit_style: &preinit_style,
            preinit_module_script: &preinit_module_script,
        });
}

#[cfg_attr(rust_js, rust_js::name = "prefetchDNS")]
fn prefetch_dns(href: &str) {
    if !href.is_empty() {
        let request = resolve_request();
        if let Some(request) = request {
            // SAFETY: ReactFlightServer keeps the hints this config's createHints made.
            let hints: &Hints = unsafe { js::cast(Some(get_hints(&request))) };
            let key = "D|".to_owned() + href;
            if hints.borrow().contains(&key) {
                // duplicate hint
                return;
            }
            hints.borrow_mut().insert(key);
            emit_hint(&request, "D", Some(js::unknown_of(href.to_owned())));
        } else {
            (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.prefetch_dns))(
                /* prefetchDNS */ href,
            );
        }
    }
}

fn preconnect(href: &str, cross_origin: Option<&'static Unknown>) {
    let request = resolve_request();
    if let Some(request) = request {
        // SAFETY: ReactFlightServer keeps the hints this config's createHints made.
        let hints: &Hints = unsafe { js::cast(Some(get_hints(&request))) };

        let key = format!(
            "C|{}|{href}",
            js::shown(cross_origin.unwrap_or(js::unknown("null")))
        );
        if hints.borrow().contains(&key) {
            // duplicate hint
            return;
        }
        hints.borrow_mut().insert(key);
        if let Kind::String(cross_origin) = js::classify(cross_origin) {
            emit_hint(
                &request,
                "C",
                Some(js::unknown_of((href.to_owned(), cross_origin.to_owned()))),
            );
        } else {
            emit_hint(&request, "C", Some(js::unknown_of(href.to_owned())));
        }
    } else {
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preconnect))(
            /* preconnect */ href,
            cross_origin,
        );
    }
}

pub fn preload(
    href: Option<&Unknown>,
    r#as: Option<&'static Unknown>,
    options: Option<&'static Unknown>,
) {
    if let Kind::String(href) = js::classify(href) {
        let request = resolve_request();
        if let Some(request) = request {
            // SAFETY: ReactFlightServer keeps the hints this config's createHints made.
            let hints: &Hints = unsafe { js::cast(Some(get_hints(&request))) };
            let mut key = String::from("L");
            if js::strict_equals(r#as, "image") && js::truthy(options) {
                key += &get_image_preload_key(
                    href,
                    js::get(options, "imageSrcSet"),
                    js::get(options, "imageSizes"),
                );
            } else {
                key += &format!("[{}]{href}", js::shown(r#as));
            }
            if hints.borrow().contains(&key) {
                // duplicate hint
                return;
            }
            hints.borrow_mut().insert(key);

            let trimmed = trim_options(options);
            if let Some(trimmed) = trimmed {
                emit_hint(
                    &request,
                    "L",
                    Some(js::unknown_of((href.to_owned(), r#as, trimmed))),
                );
            } else {
                emit_hint(&request, "L", Some(js::unknown_of((href.to_owned(), r#as))));
            }
        } else if let Kind::String(r#as) = js::classify(r#as) {
            (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preload))(
                /* preload */ href, r#as, options,
            );
        }
    }
}

pub fn preload_module(href: Option<&Unknown>, options: Option<&'static Unknown>) {
    if let Kind::String(href) = js::classify(href) {
        let request = resolve_request();
        if let Some(request) = request {
            // SAFETY: ReactFlightServer keeps the hints this config's createHints made.
            let hints: &Hints = unsafe { js::cast(Some(get_hints(&request))) };
            let key = "m|".to_owned() + href;
            if hints.borrow().contains(&key) {
                // duplicate hint
                return;
            }
            hints.borrow_mut().insert(key);

            let trimmed = trim_options(options);
            if let Some(trimmed) = trimmed {
                emit_hint(
                    &request,
                    "m",
                    Some(js::unknown_of((href.to_owned(), trimmed))),
                )
            } else {
                emit_hint(&request, "m", Some(js::unknown_of(href.to_owned())))
            }
        } else {
            (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.m))(
                /* preloadModule */ href, options,
            );
        }
    }
}

fn preinit_style(
    href: &str,
    precedence: Option<&'static Unknown>,
    options: Option<&'static Unknown>,
) {
    let request = resolve_request();
    if let Some(request) = request {
        // SAFETY: ReactFlightServer keeps the hints this config's createHints made.
        let hints: &Hints = unsafe { js::cast(Some(get_hints(&request))) };
        let key = "S|".to_owned() + href;
        if hints.borrow().contains(&key) {
            // duplicate hint
            return;
        }
        hints.borrow_mut().insert(key);

        let trimmed = trim_options(options);
        if let Some(trimmed) = trimmed {
            emit_hint(
                &request,
                "S",
                Some(js::unknown_of((
                    href.to_owned(),
                    match js::classify(precedence) {
                        Kind::String(precedence) => js::unknown_of(precedence.to_owned()),
                        _ => js::unknown_of(0.0),
                    },
                    trimmed,
                ))),
            )
        } else if let Kind::String(precedence) = js::classify(precedence) {
            emit_hint(
                &request,
                "S",
                Some(js::unknown_of((href.to_owned(), precedence.to_owned()))),
            )
        } else {
            emit_hint(&request, "S", Some(js::unknown_of(href.to_owned())))
        }
    } else {
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preinit_style))(
            /* preinitStyle */ href, precedence, options,
        );
    }
}

fn preinit_script(src: &str, options: Option<&'static Unknown>) {
    let request = resolve_request();
    if let Some(request) = request {
        // SAFETY: ReactFlightServer keeps the hints this config's createHints made.
        let hints: &Hints = unsafe { js::cast(Some(get_hints(&request))) };
        let key = "X|".to_owned() + src;
        if hints.borrow().contains(&key) {
            // duplicate hint
            return;
        }
        hints.borrow_mut().insert(key);

        let trimmed = trim_options(options);
        if let Some(trimmed) = trimmed {
            emit_hint(
                &request,
                "X",
                Some(js::unknown_of((src.to_owned(), trimmed))),
            )
        } else {
            emit_hint(&request, "X", Some(js::unknown_of(src.to_owned())))
        }
    } else {
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preinit_script))(
            /* preinitScript */ src, options,
        );
    }
}

fn preinit_module_script(src: &str, options: Option<&'static Unknown>) {
    let request = resolve_request();
    if let Some(request) = request {
        // SAFETY: ReactFlightServer keeps the hints this config's createHints made.
        let hints: &Hints = unsafe { js::cast(Some(get_hints(&request))) };
        let key = "M|".to_owned() + src;
        if hints.borrow().contains(&key) {
            // duplicate hint
            return;
        }
        hints.borrow_mut().insert(key);

        let trimmed = trim_options(options);
        if let Some(trimmed) = trimmed {
            emit_hint(
                &request,
                "M",
                Some(js::unknown_of((src.to_owned(), trimmed))),
            )
        } else {
            emit_hint(&request, "M", Some(js::unknown_of(src.to_owned())))
        }
    } else {
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preinit_module_script))(
            /* preinitModuleScript */ src, options,
        );
    }
}

// Flight normally encodes undefined as a special character however for directive option
// arguments we don't want to send unnecessary keys and bloat the payload so we create a
// trimmed object which omits any keys with null or undefined values.
// This is only typesafe because these option objects have entirely optional fields where
// null and undefined represent the same thing as no property.
#[cfg_attr(rust_js, rust_js::nullable)]
fn trim_options(options: Option<&Unknown>) -> Option<&'static Unknown> {
    let options = options?;
    let mut has_properties = false;
    let trimmed: &Dict<Option<&Unknown>> = dict::new();
    for key in js::keys_in(options) {
        // $FlowFixMe[invalid-computed-prop]
        if js::get(options, &key).is_some() {
            has_properties = true;
            dict::set(trimmed, &key, js::get(options, &key));
        }
    }
    if has_properties {
        Some(js::unknown(trimmed))
    } else {
        None
    }
}

fn get_image_preload_key(
    href: &str,
    image_src_set: Option<&Unknown>,
    image_sizes: Option<&Unknown>,
) -> String {
    let mut unique_part = String::new();
    if let Kind::String(image_src_set) = js::classify(image_src_set)
        && !image_src_set.is_empty()
    {
        unique_part += &("[".to_owned() + image_src_set + "]");
        if let Kind::String(image_sizes) = js::classify(image_sizes) {
            unique_part += &("[".to_owned() + image_sizes + "]");
        }
    } else {
        unique_part += &("[][]".to_owned() + href);
    }
    format!("[image]{unique_part}")
}
