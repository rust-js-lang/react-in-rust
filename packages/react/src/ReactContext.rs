// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use shared::environment::__DEV__;
use shared::react_symbols::{REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE};
use shared::react_types::{ReactConsumerType, ReactContext};

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
        current_value: RefCell::new(default_value),
        current_value2: RefCell::new(default_value),
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
