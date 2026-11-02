// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use js::{Kind, Promise, Unknown, promise};
use react_dom_bindings::react_dom_legacy_server_stream_config::Destination;
use react_isomorphic::react_shared_internals_server::SharedStateServer;
use react_server::react_fizz_server::{ErrorInfo, OnError};
use react_server::react_flight_server::EnvironmentName;
use shared::environment::__DEV__;
use webapi::AbortSignal;

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

unsafe extern "Rust" {
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInternalsServer")]
    #[link_name = "react-server/src/ReactSharedInternalsServer#default"]
    safe static REACT_SHARED_INTERNALS_SERVER: &'static SharedStateServer;
}

use shared::react_shared_internals::REACT_SHARED_INTERNALS as REACT_SHARED_INTERNALS_CLIENT;

use react_server::react_flight_server::{
    abort as abort_flight, attach_abort_signal as attach_flight_abort_signal,
    create_request as create_flight_request, start_flowing as start_flight_flowing,
    start_work as start_flight_work,
};

use react_client::react_flight_client::{
    close as close_flight, create_response as create_flight_response,
    create_stream_state as create_flight_stream_state, get_root as get_flight_root,
    process_string_chunk as process_flight_string_chunk,
};

use react_server::react_fizz_server::{
    abort as abort_fizz, attach_abort_signal as attach_fizz_abort_signal,
    create_request as create_fizz_request, start_flowing as start_fizz_flowing,
    start_work as start_fizz_work,
};

use crate::react_fizz_config_markup::{
    create_render_state, create_resumable_state, create_root_format_context,
};

// This is the intersection of ReactNodeList and ReactClientValue minus
// Client/ServerReferences.
type ReactMarkupNodeList = Unknown;

pub struct MarkupOptions {
    pub identifier_prefix: Option<&'static str>,
    pub signal: Option<&'static AbortSignal>,
    pub on_error: Option<Rc<OnError>>,
}

fn no_server_call_or_form_action<Id, Args, Return>(_id: Id, _args: Args) -> Return {
    panic!("renderToHTML should not have emitted Server References. This is a bug in React.");
}

/// Puts the server's `getCurrentStack` back when the user's `onError` is
/// done with the client's.
struct RestoreGetCurrentStack(Option<&'static dyn Fn() -> String>);

impl Drop for RestoreGetCurrentStack {
    fn drop(&mut self) {
        REACT_SHARED_INTERNALS_SERVER.get_current_stack.set(self.0);
    }
}

#[cfg_attr(rust_js, rust_js::name = "experimental_renderToHTML")]
pub fn experimental_render_to_html(
    children: Option<&'static ReactMarkupNodeList>,
    options: Option<&'static MarkupOptions>,
) -> Promise<String> {
    promise::new(move |resolve, reject| {
        let flight_response = create_flight_response(
            None,
            None,
            None,
            Some(&no_server_call_or_form_action),
            Some(&no_server_call_or_form_action),
            None,
            None,
            false,
            None,
            false,
            None,
            None,
            None,
            None,
        );
        let stream_state = create_flight_stream_state(&flight_response, None);
        let stash_error_idx = Cell::new(1.0);
        let stashed_errors: Rc<RefCell<HashMap<String, Option<&'static Unknown>>>> =
            Rc::new(RefCell::new(HashMap::new()));

        // GAP rd557: a closure printed as a function declaration prints its
        // body's comments above it, or inside its first statement's call.
        let handle_flight_error = {
            let stashed_errors = Rc::clone(&stashed_errors);
            move |error: Option<&'static Unknown>| -> Option<String> {
                // For Flight errors we don't immediately reject, because they might not matter
                // to the output of the HTML. We stash the error with a digest in case we need
                // to get to the original error from the Fizz render.
                let id = stash_error_idx.get().to_string();
                stash_error_idx.set(stash_error_idx.get() + 1.0);
                stashed_errors.borrow_mut().insert(id.clone(), error);
                Some(id)
            }
        };

        // GAP rd557: these closures' comments print above the function.
        // GAP rd558: an `Option` a `HashMap` holds is boxed, so
        // `stashed_errors[id]` reads `$someValue($unwrap($someGet(..)))`
        // where upstream reads `stashedErrors.get(id)`.
        let handle_error =
            move |error: Option<&'static Unknown>, error_info: &ErrorInfo| -> Option<String> {
                let mut error = error;
                if js::type_of(error) == "object" && !matches!(js::classify(error), Kind::Null) {
                    let id = js::get(error, "digest");
                    // Note that the original error might be `undefined` so we need a has check.
                    if let Kind::String(id) = js::classify(id)
                        && stashed_errors.borrow().contains_key(id)
                    {
                        // Get the original error thrown inside Flight.
                        error = stashed_errors.borrow()[id];
                    }
                }

                // Any error rejects the promise, regardless of where it happened.
                // Unlike other React SSR we don't want to put Suspense boundaries into
                // client rendering mode because there's no client rendering here.
                reject(error);

                let on_error = options.and_then(|options| options.on_error.as_ref());
                if let Some(on_error) = on_error {
                    if __DEV__ {
                        let prev_get_current_stack_impl =
                            REACT_SHARED_INTERNALS_SERVER.get_current_stack.get();
                        // We're inside a "client" callback from Fizz but we only have access to the
                        // "server" runtime so to get access to a stack trace within this callback we
                        // need to override it to get it from the client runtime.
                        REACT_SHARED_INTERNALS_SERVER
                            .get_current_stack
                            .set(REACT_SHARED_INTERNALS_CLIENT.get_current_stack());
                        let _restore = RestoreGetCurrentStack(prev_get_current_stack_impl);
                        on_error(error, error_info);
                    } else {
                        on_error(error, error_info);
                    }
                }
                None
            };
        let flight_request = create_flight_request(
            // $FlowFixMe[incompatible-type]: This should be a subtype but not everything is typed covariant.
            children,
            None,
            Some(Rc::new(handle_flight_error)),
            options.and_then(|options| options.identifier_prefix),
            None,
            None,
            Some(EnvironmentName::Str("Markup".to_string())),
            None,
            false,
        );
        let resumable_state = Rc::new(create_resumable_state(
            options.and_then(|options| options.identifier_prefix),
            None,
            None,
            None,
            None,
        ));
        let root = get_flight_root(&flight_response);
        let fizz_request = create_fizz_request(
            // $FlowFixMe[incompatible-type]: Thenables as children are supported.
            Some(js::unknown_of(root)),
            js::unknown_of(Rc::clone(&resumable_state)),
            js::unknown_of(create_render_state(
                &resumable_state,
                None,
                None,
                None,
                None,
                None,
            )),
            js::unknown_of(create_root_format_context(None)),
            Some(f64::INFINITY),
            Some(Rc::new(handle_error)),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let flight_destination = Destination {
            push: {
                let flight_response = Rc::clone(&flight_response);
                Rc::new(move |chunk: Option<&str>| {
                    if let Some(chunk) = chunk {
                        process_flight_string_chunk(&flight_response, &stream_state, chunk);
                    } else {
                        close_flight(&flight_response);
                    }
                    true
                })
            },
            destroy: {
                let fizz_request = Rc::clone(&fizz_request);
                Rc::new(move |error| {
                    abort_fizz(&fizz_request, error);
                    reject(error);
                })
            },
        };
        let buffer = Rc::new(RefCell::new(String::new()));
        let fizz_destination = Destination {
            // $FlowFixMe[missing-local-annot]
            push: Rc::new(move |chunk: Option<&str>| {
                if let Some(chunk) = chunk {
                    buffer.borrow_mut().push_str(chunk);
                } else {
                    // null indicates that we finished
                    resolve(buffer.take());
                }
                true
            }),
            // $FlowFixMe[missing-local-annot]
            destroy: {
                let flight_request = Rc::clone(&flight_request);
                Rc::new(move |error| {
                    abort_flight(&flight_request, error);
                    reject(error);
                })
            },
        };

        let signal = options.and_then(|options| options.signal);
        if let Some(signal) = signal {
            attach_flight_abort_signal(&flight_request, signal);
            attach_fizz_abort_signal(&fizz_request, signal);
        }
        start_flight_work(&flight_request);
        start_flight_flowing(&flight_request, js::unknown_of(flight_destination));
        start_fizz_work(&fizz_request);
        start_fizz_flowing(&fizz_request, js::unknown_of(fizz_destination));
    })
}
