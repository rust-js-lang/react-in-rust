// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, Unknown, object};
use shared::environment::__DEV__;
use webapi::console;

use shared::react_feature_flags::DISABLE_LEGACY_CONTEXT;

use shared::get_component_name_from_type::get_component_name_from_type;

#[cfg_attr(rust_js, rust_js::link_name = "getChildContext")]
fn get_child_context(this: &Unknown) -> &'static Context {
    let _ = this;
    unreachable!()
}

pub type Context = Dict<Option<&'static Unknown>>;

thread_local! {
    static WARNED_ABOUT_MISSING_GET_CHILD_CONTEXT: Option<&'static Dict<bool>> =
        if __DEV__ { Some(js::dict::new()) } else { None };

    pub static EMPTY_CONTEXT_OBJECT: &'static Context = js::dict::new();
}

js::on_load! {
    if __DEV__ {
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| object::freeze(*empty_context_object));
    }
}

pub fn get_masked_context(
    r#type: &Unknown,
    unmasked_context: &'static Context,
) -> &'static Context {
    if DISABLE_LEGACY_CONTEXT {
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
    } else {
        let context_types = js::get(r#type, "contextTypes");
        if !js::truthy(context_types) {
            return EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object);
        }

        let context: &'static Context = js::dict::new();
        for key in js::keys_in(context_types) {
            js::dict::set(context, &key, js::get(unmasked_context, key.as_str()));
        }

        context
    }
}

pub fn process_child_context(
    instance: &Unknown,
    r#type: &Unknown,
    parent_context: &'static Context,
    child_context_types: &Unknown,
) -> &'static Context {
    if DISABLE_LEGACY_CONTEXT {
        parent_context
    } else {
        // TODO (bvaughn) Replace this behavior with an invariant() in the future.
        // It has only been added in Fiber to match the (unintentional) behavior in Stack.
        if js::type_of(js::get(instance, "getChildContext")) != "function" {
            if __DEV__ {
                let component_name = get_component_name_from_type(Some(r#type))
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "Unknown".to_string());

                if let Some(warned_about_missing_get_child_context) =
                    WARNED_ABOUT_MISSING_GET_CHILD_CONTEXT.with(|warned| *warned)
                    && !js::dict::get(warned_about_missing_get_child_context, &component_name)
                        .is_some_and(|&warned| warned)
                {
                    js::dict::set(
                        warned_about_missing_get_child_context,
                        &component_name,
                        true,
                    );
                    console::error((
                        concat!(
                            "%s.childContextTypes is specified but there is no getChildContext() method ",
                            "on the instance. You can either define getChildContext() on %s or remove ",
                            "childContextTypes from it."
                        ),
                        component_name.as_str(),
                        component_name.as_str(),
                    ));
                }
            }
            return parent_context;
        }

        let child_context = get_child_context(instance);
        for context_key in js::keys_in(child_context) {
            if !js::has(child_context_types, context_key.as_str()) {
                panic!(
                    "{}.getChildContext(): key \"{context_key}\" is not defined in childContextTypes.",
                    get_component_name_from_type(Some(r#type))
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "Unknown".to_string())
                );
            }
        }
        object::spread((parent_context, child_context))
    }
}
