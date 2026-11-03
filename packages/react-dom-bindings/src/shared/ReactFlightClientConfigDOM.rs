// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This client file is in the shared folder because it applies to both SSR and browser contexts.
// It is the configuration of the FlightClient behavior which can run in either environment.

use js::Unknown;

use shared::react_dom_shared_internals::REACT_DOM_SHARED_INTERNALS;

use crate::cross_origin_strings::{CrossOriginString, get_cross_origin_string};

/// A hint's model, the server's `HintModel<Code>`: what its code says it is.
pub type HintModel = Unknown;

/// `'C'`'s model: `href`, or `[href, crossOrigin]`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Preconnect {
    Href(&'static str),
    WithCrossOrigin((&'static str, &'static Unknown)),
}

/// `'L'`'s model: `[href, as]`, or `[href, as, options]`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Preload {
    Plain((&'static str, &'static str)),
    WithOptions((&'static str, &'static str, &'static Unknown)),
}

/// `'m'`'s, `'X'`'s and `'M'`'s model: `href`, or `[href, options]`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum WithOptions {
    Href(&'static str),
    WithOptions((&'static str, &'static Unknown)),
}

/// `'S'`'s model: `href`, or `[href, precedence]` or `[href, precedence | 0,
/// options]`, whose `options` is there when it's three long.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum PreinitStyle {
    Href(&'static str),
    WithPrecedence((&'static str, &'static Unknown, Option<&'static Unknown>)),
}

pub fn dispatch_hint(code: &str, model: Option<&'static HintModel>) {
    let dispatcher = REACT_DOM_SHARED_INTERNALS.d(); /* ReactDOMCurrentDispatcher */
    match code {
        "D" => {
            let refined: &str = refine_model(code, model);
            let href = refined;
            (dispatcher.prefetch_dns)(/* prefetchDNS */ href);
        }
        "C" => {
            let refined: Preconnect = refine_model(code, model);
            match refined {
                Preconnect::Href(href) => {
                    (dispatcher.preconnect)(/* preconnect */ href, None);
                }
                Preconnect::WithCrossOrigin((href, cross_origin)) => {
                    (dispatcher.preconnect)(/* preconnect */ href, Some(cross_origin));
                }
            }
        }
        "L" => {
            let refined: Preload = refine_model(code, model);
            match refined {
                Preload::WithOptions((href, r#as, options)) => {
                    (dispatcher.preload)(/* preload */ href, r#as, Some(options));
                }
                Preload::Plain((href, r#as)) => {
                    (dispatcher.preload)(/* preload */ href, r#as, None);
                }
            }
        }
        "m" => {
            let refined: WithOptions = refine_model(code, model);
            match refined {
                WithOptions::Href(href) => {
                    (dispatcher.m)(/* preloadModule */ href, None);
                }
                WithOptions::WithOptions((href, options)) => {
                    (dispatcher.m)(/* preloadModule */ href, Some(options));
                }
            }
        }
        "X" => {
            let refined: WithOptions = refine_model(code, model);
            match refined {
                WithOptions::Href(href) => {
                    (dispatcher.preinit_script)(/* preinitScript */ href, None);
                }
                WithOptions::WithOptions((href, options)) => {
                    (dispatcher.preinit_script)(/* preinitScript */ href, Some(options));
                }
            }
        }
        "S" => {
            let refined: PreinitStyle = refine_model(code, model);
            match refined {
                PreinitStyle::Href(href) => {
                    (dispatcher.preinit_style)(/* preinitStyle */ href, None, None);
                }
                PreinitStyle::WithPrecedence((href, precedence, options)) => {
                    let precedence = if js::strict_equals(precedence, 0.0) {
                        None
                    } else {
                        Some(precedence)
                    };
                    (dispatcher.preinit_style)(/* preinitStyle */ href, precedence, options);
                }
            }
        }
        "M" => {
            let refined: WithOptions = refine_model(code, model);
            match refined {
                WithOptions::Href(href) => {
                    (dispatcher.preinit_module_script)(/* preinitModuleScript */ href, None);
                }
                WithOptions::WithOptions((href, options)) => {
                    (dispatcher.preinit_module_script)(
                        /* preinitModuleScript */ href,
                        Some(options),
                    );
                }
            }
        }
        _ => {}
    }
}

// Flow is having trouble refining the HintModels so we help it a bit.
// This should be compiled out in the production build.
fn refine_model<T>(code: &str, model: Option<&'static HintModel>) -> T {
    let _ = code;
    // SAFETY: the server sends each hint's model in the shape its code names.
    unsafe { js::cast(model) }
}

/// What `preinitModuleForSSR` preinits a module script with.
struct PreinitModuleScriptOptions<'a> {
    cross_origin: Option<&'a CrossOriginString>,
    integrity: Option<&'a str>,
    nonce: Option<&'a str>,
    fetch_priority: Option<&'a str>,
}

unsafe impl js::Defined for PreinitModuleScriptOptions<'_> {}

/// What `preinitScriptForSSR` preinits a script with.
struct PreinitScriptOptions<'a> {
    cross_origin: Option<&'a CrossOriginString>,
    integrity: Option<&'a str>,
    fetch_priority: Option<&'a str>,
    nonce: Option<&'a str>,
}

unsafe impl js::Defined for PreinitScriptOptions<'_> {}

#[cfg_attr(rust_js, rust_js::name = "preinitModuleForSSR")]
pub fn preinit_module_for_ssr(
    href: &str,
    nonce: Option<&'static str>,
    cross_origin: Option<&'static str>,
) {
    (REACT_DOM_SHARED_INTERNALS
        .d() /* ReactDOMCurrentDispatcher */
        .preinit_module_script)(
        /* preinitModuleScript */ href,
        Some(js::unknown_of(PreinitModuleScriptOptions {
            cross_origin: get_cross_origin_string(cross_origin.map(js::unknown)),
            integrity: None,
            nonce,
            fetch_priority: None,
        })),
    );
}

#[cfg_attr(rust_js, rust_js::name = "preinitScriptForSSR")]
pub fn preinit_script_for_ssr(
    href: &str,
    nonce: Option<&'static str>,
    cross_origin: Option<&'static str>,
) {
    (REACT_DOM_SHARED_INTERNALS
        .d() /* ReactDOMCurrentDispatcher */
        .preinit_script)(
        /* preinitScript */ href,
        Some(js::unknown_of(PreinitScriptOptions {
            cross_origin: get_cross_origin_string(cross_origin.map(js::unknown)),
            integrity: None,
            fetch_priority: None,
            nonce,
        })),
    );
}
