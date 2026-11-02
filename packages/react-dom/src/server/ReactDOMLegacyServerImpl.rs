// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_dom_bindings::react_dom_legacy_server_stream_config::Destination;
use shared::react_types::ReactNodeList;

use react_server::react_fizz_server::{
    ErrorInfo, abort, create_request, start_flowing, start_work,
};

use react_dom_bindings::react_fizz_config_dom_legacy::{
    create_render_state, create_resumable_state, create_root_format_context,
};

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

pub struct ServerOptions {
    pub identifier_prefix: Option<String>,
}

fn on_error(_error: Option<&'static Unknown>, _error_info: &ErrorInfo) -> Option<&'static Unknown> {
    // Non-fatal errors are ignored.
    None
}

pub fn render_to_string_impl(
    children: Option<&'static ReactNodeList>,
    options: Option<&ServerOptions>,
    generate_static_markup: bool,
    abort_reason: &'static str,
) -> String {
    let did_fatal = Rc::new(Cell::new(false));
    let fatal_error = Rc::new(Cell::new(js::null::<&'static Unknown>()));
    let result = Rc::new(RefCell::new(String::new()));
    let destination = Destination {
        // $FlowFixMe[missing-local-annot]
        push: {
            let result = Rc::clone(&result);
            Rc::new(move |chunk| {
                if let Some(chunk) = chunk {
                    result.borrow_mut().push_str(chunk);
                }
                true
            })
        },
        // $FlowFixMe[missing-local-annot]
        destroy: {
            let did_fatal = Rc::clone(&did_fatal);
            let fatal_error = Rc::clone(&fatal_error);
            Rc::new(move |error| {
                did_fatal.set(true);
                fatal_error.set(error);
            })
        },
    };

    let ready_to_stream = Rc::new(Cell::new(false));
    let on_shell_ready = {
        let ready_to_stream = Rc::clone(&ready_to_stream);
        move || {
            ready_to_stream.set(true);
        }
    };
    let resumable_state = Rc::new(create_resumable_state(
        options.and_then(|options| options.identifier_prefix.as_deref()),
        None,
        None,
        None,
        None,
    ));
    let request = create_request(
        children,
        js::unknown_of(Rc::clone(&resumable_state)),
        js::unknown_of(create_render_state(
            &resumable_state,
            generate_static_markup,
        )),
        js::unknown_of(create_root_format_context(None)),
        Some(f64::INFINITY),
        Some(Rc::new(on_error)),
        None,
        None,
        Some(Rc::new(on_shell_ready)),
        None,
        None,
        None,
    );
    start_work(&request);
    // If anything suspended and is still pending, we'll abort it before writing.
    // That way we write only client-rendered boundaries from the start.
    abort(&request, Some(js::unknown(abort_reason)));
    start_flowing(&request, js::unknown_of(destination));
    if did_fatal.get() && !js::strict_equals(fatal_error.get(), Some(js::unknown(abort_reason))) {
        js::throw(fatal_error.get());
    }

    if !ready_to_stream.get() {
        // Note: This error message is the one we use on the client. It doesn't
        // really make sense here. But this is the legacy server renderer, anyway.
        // We're going to delete it soon.
        panic!(
            "{}",
            concat!(
                "A component suspended while responding to synchronous input. This ",
                "will cause the UI to be replaced with a loading indicator. To fix, ",
                "updates that suspend should be wrapped with startTransition."
            )
        );
    }

    result.take()
}
