// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Kind, Promise, Unknown, function};
use shared::react_types::ReactFormState;
use webapi::{FormData, FormDataEntryValue, form_data};

use crate::react_flight_reply_server::{
    ChunkState, MAX_BOUND_ARGS, close, create_response, get_root, preload_module, require_module,
    resolve_server_reference,
};

type ServerReferenceId = Unknown;

/// A Server Reference's metadata, as a form encodes it: its id, and a
/// promise of its bound arguments.
struct ServerReferenceMetaData {
    id: Option<&'static ServerReferenceId>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    bound: Option<&'static Unknown>,
}

fn bind_args(
    r#fn: Option<&'static Unknown>,
    args: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    // GAP rd562: upstream's `args.length > MAX_BOUND_ARGS`, JS's `>` of any
    // value, is taken apart from the `None` `js::get` gives first.
    if let Some(length) = js::get(args, "length")
        && *length > *js::unknown(&MAX_BOUND_ARGS)
    {
        panic!(
            "Server Function has too many bound arguments. Received {} but the limit is {}.",
            js::shown(length),
            MAX_BOUND_ARGS
        );
    }

    // GAP rd563: upstream's `[null].concat(args)`, JS's `concat` of any
    // value, an array's items or the value itself, is a match of its kind.
    let bound_args = match js::classify(args) {
        Kind::Array(args) => [&[js::null()], args].concat(),
        _ => vec![js::null(), args],
    };
    // GAP rd486: upstream's `fn.bind.apply(fn, ..)`, the value's own `bind`,
    // read and applied as upstream does, its function of any shape read by
    // a cast.
    let bind: &dyn Fn() -> Option<&'static Unknown> = unsafe { js::cast(js::get(r#fn, "bind")) };
    function::apply(bind, Some(&r#fn), Some(&bound_args))
}

fn load_server_reference<M: js::Defined + ?Sized>(
    bundler_config: &'static M,
    meta_data: &ServerReferenceMetaData,
) -> Promise<Option<&'static Unknown>> {
    let id = meta_data.id;
    let Kind::String(id) = js::classify(id) else {
        // Upstream's `null as any`: a caller's `.then` of it throws JS's
        // TypeError, as upstream's does.
        return unsafe { js::cast(js::null::<&Unknown>()) };
    };
    let server_reference = resolve_server_reference(bundler_config, id);
    // We expect most servers to not really need this because you'd just have all
    // the relevant modules already loaded but it allows for lazy loading of code
    // if needed.
    let preload_promise = preload_module(server_reference);
    let bound = meta_data.bound;
    if js::instance_of(bound, js::class::<Promise<()>>()) {
        // GAP rd485: `Promise.all([..])` of any values: js's promise
        // functions take and give `Promise`s, which a `Thenable` and a
        // reply's `ReactPromise` are none of.
        js::promise::all2((
            unsafe { js::cast::<Promise<Option<&'static Unknown>>>(bound) },
            unsafe {
                js::cast::<Promise<Option<&'static Unknown>>>(preload_promise.map(js::unknown))
            },
        ))
        .then(move |(args, _)| bind_args(require_module(server_reference), args))
    } else if js::truthy(preload_promise) {
        // GAP rd485: as above.
        js::promise::resolve(preload_promise).then(move |_| require_module(server_reference))
    } else {
        // GAP rd567: a comment before a block's tail expression, this
        // `else`'s, is lost.
        // Synchronously available
        js::promise::resolve(require_module(server_reference))
    }
}

fn decode_bound_action_meta_data<M: js::Defined + ?Sized>(
    body: &'static FormData,
    server_manifest: &'static M,
    form_field_prefix: &str,
    array_size_limit: Option<&'static Unknown>,
) -> &'static ServerReferenceMetaData {
    // The data for this reference is encoded in multiple fields under this prefix.
    let action_response = create_response(
        server_manifest,
        form_field_prefix,
        None,
        Some(js::unknown(body)),
        array_size_limit,
    );
    close(&action_response);
    let ref_promise = get_root(&action_response);
    // Force it to initialize
    // $FlowFixMe[incompatible-type]
    ref_promise.then(Some(Rc::new(|_| {})), None);
    let ChunkState::Initialized { value, .. } = *ref_promise.state.borrow() else {
        // $FlowFixMe[prop-missing]
        js::throw(js::get(js::unknown(&*ref_promise), "reason"));
    };
    // GAP rd483: the root is any value, read as the metadata upstream's
    // `getRoot<{id, bound}>` types it.
    unsafe { js::cast(value) }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn decode_action<M: js::Defined + ?Sized>(
    body: &'static FormData,
    server_manifest: &'static M,
) -> Option<Promise<&'static dyn Fn() -> Option<&'static Unknown>>> {
    // We're going to create a new formData object that holds all the fields except
    // the implementation details of the action data.
    let form_data = form_data::new(None, None);

    let mut maybe_action_key: Option<String> = js::null();

    // $FlowFixMe[prop-missing]
    body.for_each(|value, key, _| {
        if !key.starts_with("$ACTION_") {
            // $FlowFixMe[incompatible-type]
            form_data.append(key, value);
        } else if key.starts_with("$ACTION_REF_") {
            // Later actions may override earlier actions if a button is used to
            // override the default form action. However, we don't expect the same
            // action ref field to be sent multiple times in legitimate form data.
            maybe_action_key = Some(key.to_owned());
        } else if key.starts_with("$ACTION_ID_") {
            // A simple action with no bound arguments may appear twice in the form data
            // if a button specifies the same action as the default form action.
            maybe_action_key = Some(key.to_owned());
        }
    });

    let action_key = maybe_action_key?;

    let action = if action_key.starts_with("$ACTION_REF_") {
        let form_field_prefix = "$ACTION_".to_owned()
            + &js::string::slice(&action_key, js::string::length("$ACTION_REF_") as f64, None)
            + ":";
        let meta_data =
            decode_bound_action_meta_data(body, server_manifest, &form_field_prefix, None);
        load_server_reference(server_manifest, meta_data)
    } else if action_key.starts_with("$ACTION_ID_") {
        let id = js::string::slice(&action_key, js::string::length("$ACTION_ID_") as f64, None);
        load_server_reference(
            server_manifest,
            &ServerReferenceMetaData {
                id: Some(js::unknown_of(id)),
                bound: None,
            },
        )
    } else {
        panic!("Cannot handle action key. This is a bug in React.");
    };

    // Return the action with the remaining FormData bound to the first argument.
    Some(action.then(move |r#fn| {
        let r#fn: &dyn Fn(&FormData) -> Option<&'static Unknown> = unsafe { js::cast(r#fn) };
        function::bind(r#fn, Some(&Kind::Null), (form_data,))
    }))
}

pub fn decode_form_state<M: js::Defined + ?Sized>(
    action_result: Option<&'static Unknown>,
    body: &'static FormData,
    server_manifest: &'static M,
) -> Promise<Option<ReactFormState<Option<&'static Unknown>, Option<&'static ServerReferenceId>>>> {
    let key_path = body.get("$ACTION_KEY");
    let Some(FormDataEntryValue::Str(key_path)) = key_path else {
        // This form submission did not include any form state.
        return js::promise::resolve(js::null());
    };
    // Search through the form data object to get the reference id and the number
    // of bound arguments. This repeats some of the work done in decodeAction.
    let mut action_key: Option<String> = js::null();
    // $FlowFixMe[prop-missing]
    body.for_each(|_, key, _| {
        if key.starts_with("$ACTION_REF_") {
            action_key = Some(key.to_owned());
        }
        // We don't check for the simple $ACTION_ID_ case because form state actions
        // are always bound to the state argument.
    });
    let Some(action_key) = action_key else {
        // Should be unreachable.
        return js::promise::resolve(js::null());
    };

    let form_field_prefix = "$ACTION_".to_owned()
        + &js::string::slice(&action_key, js::string::length("$ACTION_REF_") as f64, None)
        + ":";
    let meta_data = decode_bound_action_meta_data(body, server_manifest, &form_field_prefix, None);

    let reference_id = meta_data.id;
    js::promise::resolve(meta_data.bound).then(move |bound| {
        if matches!(js::classify(bound), Kind::Null) {
            // Should be unreachable because form state actions are always bound to the
            // state argument.
            return js::null();
        }
        // The form action dispatch method is always bound to the initial state.
        // But when comparing signatures, we compare to the original unbound action.
        // Subtract one from the arity to account for this.
        let bound_arity = js::subtract(js::get(bound, "length"), 1.0).unwrap();
        Some((
            action_result,
            key_path.to_owned(),
            reference_id,
            bound_arity,
        ))
    })
}
