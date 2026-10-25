// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Kind, PropertyDescriptor, SymbolFor, Unknown, object};
use shared::environment::__DEV__;
use shared::react_symbols::REACT_MEMO_TYPE;
use webapi::console;

/// Whether a memo's props are equal, so that it needn't render again.
pub type Compare = dyn Fn(Option<&Unknown>, Option<&Unknown>) -> bool;

/// A component that renders again only when its props change.
pub struct Memo {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub r#type: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub compare: Option<&'static Compare>,
}

pub fn memo(r#type: Option<&'static Unknown>, compare: Option<&'static Compare>) -> Memo {
    if __DEV__ && r#type.is_none() {
        console::error((
            concat!(
                "memo: The first argument must be a component. Instead ",
                "received: %s"
            ),
            if let Kind::Null = js::classify(r#type) {
                "null".to_string()
            } else {
                js::type_of(r#type)
            },
        ));
    }
    let element_type = Memo {
        r#typeof: REACT_MEMO_TYPE,
        r#type,
        compare,
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
                    //   React.memo((props) => {...});
                    // This kind of inner function is not used elsewhere so the side effect is okay.
                    if !js::truthy(js::get(r#type, "name"))
                        && !js::truthy(js::get(r#type, "displayName"))
                    {
                        object::define_property(
                            &r#type,
                            "name",
                            &PropertyDescriptor {
                                value: Some(&name),
                                ..Default::default()
                            },
                        );
                        js::set(r#type, "displayName", name);
                    }
                }),
                ..Default::default()
            },
        );
    }
    element_type
}
