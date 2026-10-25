// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::{Rc, Weak};

use js::{SymbolFor, Unknown};
use shared::environment::__DEV__;
use shared::react_symbols::{REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE};

/// A context: the value its provider gives what it renders, and the
/// consumer that reads it.
pub struct ReactContext<T: 'static> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_currentValue")]
    pub current_value: Cell<T>,
    #[cfg_attr(rust_js, rust_js::name = "_currentValue2")]
    pub current_value2: Cell<T>,
    #[cfg_attr(rust_js, rust_js::name = "_threadCount")]
    pub thread_count: Cell<f64>,
    #[cfg_attr(rust_js, rust_js::name = "Provider")]
    pub provider: Cell<Weak<ReactContext<T>>>,
    #[cfg_attr(rust_js, rust_js::name = "Consumer")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub consumer: Cell<Option<Rc<ReactConsumerType<T>>>>,
    // DEV only
    #[cfg_attr(rust_js, rust_js::name = "_currentRenderer")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub current_renderer: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::name = "_currentRenderer2")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub current_renderer2: Cell<Option<&'static Unknown>>,
}

/// What reads a context's value.
pub struct ReactConsumerType<T: 'static> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_context")]
    pub context: Weak<ReactContext<T>>,
}

pub fn create_context<T>(default_value: T) -> Rc<ReactContext<T>>
where
    T: Copy,
{
    // TODO: Second argument used to be an optional `calculateChangedBits`
    // function. Warn to reserve for future use?

    let context = Rc::new(ReactContext {
        r#typeof: REACT_CONTEXT_TYPE,
        // As a workaround to support multiple concurrent renderers, we categorize
        // some renderers as primary and others as secondary. We only expect
        // there to be two concurrent renderers at most: React Native (primary) and
        // Fabric (secondary); React DOM (primary) and React ART (secondary).
        // Secondary renderers store their context values on separate fields.
        current_value: Cell::new(default_value),
        current_value2: Cell::new(default_value),
        // Used to track how many concurrent renderers this context currently
        // supports within in a single renderer. Such as parallel server rendering.
        thread_count: Cell::new(0.0),
        // These are circular
        provider: Cell::new(Weak::new()),
        consumer: Cell::new(None),
        current_renderer: Cell::new(None),
        current_renderer2: Cell::new(None),
    });

    context.provider.set(Rc::downgrade(&context));
    context.consumer.set(Some(Rc::new(ReactConsumerType {
        r#typeof: REACT_CONSUMER_TYPE,
        context: Rc::downgrade(&context),
    })));
    if __DEV__ {
        context.current_renderer.set(None);
        context.current_renderer2.set(None);
    }

    context
}
