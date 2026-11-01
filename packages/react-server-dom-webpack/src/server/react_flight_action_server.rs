//! ReactFlightActionServer's functions the server entries re-export, its
//! JS's until it's ported. Bindings alone, so no JS of its own.

use js::{Promise, Unknown};
use react_server::react_flight_reply_server::ServerManifest;
use shared::react_types::ReactFormState;
use webapi::FormData;

use super::react_flight_server_config_webpack_bundler::ServerReferenceId;

/// The action a form's data names, its remaining fields bound to its first
/// argument, or `None` where it names none.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightActionServer#decodeAction"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn decode_action(
    body: &FormData,
    server_manifest: &ServerManifest,
) -> Option<Promise<&'static dyn Fn() -> Option<&'static Unknown>>> {
    let _ = (body, server_manifest);
    unreachable!()
}

/// The form state an action's result is of, where the form's data has its
/// key.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightActionServer#decodeFormState"
)]
pub fn decode_form_state(
    action_result: Option<&'static Unknown>,
    body: &FormData,
    server_manifest: &ServerManifest,
) -> Promise<Option<ReactFormState<Option<&'static Unknown>, ServerReferenceId>>> {
    let _ = (action_result, body, server_manifest);
    unreachable!()
}
