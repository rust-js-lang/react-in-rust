// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};

use js::Unknown;
use react_dom_bindings::react_dom_legacy_server_stream_config::Destination;
use react_dom_bindings::react_fizz_config_dom_legacy::{
    create_render_state, create_resumable_state, create_root_format_context,
};
use react_server::react_fizz_current_task::{
    ErrorInfo, abort, create_request, start_flowing, start_work,
};

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

pub struct ServerOptions {
    pub identifier_prefix: Option<String>,
}

fn on_error(_error: Option<&'static Unknown>, _error_info: &ErrorInfo) -> Option<String> {
    // Non-fatal errors are ignored.
    None
}

pub fn render_to_string_impl(
    children: Option<&'static Unknown>,
    options: Option<&ServerOptions>,
    generate_static_markup: bool,
    abort_reason: &str,
) -> String {
    let did_fatal = Cell::new(false);
    let fatal_error = Cell::new(js::null::<&'static Unknown>());
    let result = RefCell::new(String::new());
    let destination = Destination {
        // $FlowFixMe[missing-local-annot]
        push: &|chunk| {
            if let Some(chunk) = chunk {
                result.borrow_mut().push_str(chunk);
            }
            true
        },
        // $FlowFixMe[missing-local-annot]
        destroy: &|error| {
            did_fatal.set(true);
            fatal_error.set(error);
        },
    };

    let ready_to_stream = Cell::new(false);
    let on_shell_ready = || {
        ready_to_stream.set(true);
    };
    let resumable_state = create_resumable_state(
        options.and_then(|options| options.identifier_prefix.as_deref()),
        None,
        None,
        None,
        None,
    );
    let request = create_request(
        children,
        &resumable_state,
        &create_render_state(&resumable_state, generate_static_markup),
        &create_root_format_context(None),
        Some(f64::INFINITY),
        Some(&on_error),
        None,
        None,
        Some(&on_shell_ready),
        None,
        None,
        None,
    );
    start_work(request);
    // If anything suspended and is still pending, we'll abort it before writing.
    // That way we write only client-rendered boundaries from the start.
    abort(request, Some(js::unknown(abort_reason)));
    start_flowing(request, &destination);
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

    result.into_inner()
}
