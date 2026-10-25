// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Kind, PropertyDescriptor, SymbolFor, Unknown, function, object};
use shared::environment::__DEV__;
use shared::react_symbols::{REACT_FORWARD_REF_TYPE, REACT_MEMO_TYPE};
use webapi::console;

/// A component made of a render function of its props and its ref.
pub struct ForwardRef {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub render: Option<&'static Unknown>,
}

pub fn forward_ref(render: Option<&'static Unknown>) -> ForwardRef {
    if __DEV__ {
        if let Some(render) = render
            && SymbolFor::from_value(js::get(render, "$$typeof")) == Some(REACT_MEMO_TYPE)
        {
            console::error((concat!(
                "forwardRef requires a render function but received a `memo` ",
                "component. Instead of forwardRef(memo(...)), use ",
                "memo(forwardRef(...))."
            ),));
        } else if js::type_of(render) != "function" {
            console::error((
                "forwardRef requires a render function but was given %s.",
                if let Kind::Null = js::classify(render) {
                    "null".to_string()
                } else {
                    js::type_of(render)
                },
            ));
        } else if let Kind::Function(render) = js::classify(render)
            && function::length(render) != 0
            && function::length(render) != 2
        {
            console::error((
                "forwardRef render functions accept exactly two parameters: props and ref. %s",
                if function::length(render) == 1 {
                    "Did you forget to use the ref parameter?"
                } else {
                    "Any additional parameter will be undefined."
                },
            ));
        }

        if let Some(render) = render
            && js::get(render, "defaultProps").is_some()
        {
            console::error((concat!(
                "forwardRef render functions do not support defaultProps. ",
                "Did you accidentally pass a React component?"
            ),));
        }
    }

    let element_type = ForwardRef {
        r#typeof: REACT_FORWARD_REF_TYPE,
        render,
    };
    if __DEV__ {
        let own_name = Cell::new(None);
        object::define_property(
            &element_type,
            "displayName",
            &PropertyDescriptor {
                enumerable: Some(false),
                configurable: Some(true),
                get: Some(&|| own_name.get()),
                set: Some(&|name| {
                    own_name.set(unsafe { js::cast(name) });

                    // The inner component shouldn't inherit this display name in most cases,
                    // because the component may be used elsewhere.
                    // But it's nice for anonymous functions to inherit the name,
                    // so that our component-stack generation logic will display their frames.
                    // An anonymous function generally suggests a pattern like:
                    //   React.forwardRef((props, ref) => {...});
                    // This kind of inner function is not used elsewhere so the side effect is okay.
                    if !js::truthy(js::get(render, "name"))
                        && !js::truthy(js::get(render, "displayName"))
                    {
                        object::define_property(
                            &render,
                            "name",
                            &PropertyDescriptor {
                                value: Some(&name),
                                ..Default::default()
                            },
                        );
                        js::set(render, "displayName", name);
                    }
                }),
                ..Default::default()
            },
        );
    }
    element_type
}
