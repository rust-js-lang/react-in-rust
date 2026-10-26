// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use shared::react_feature_flags::{ENABLE_DEFAULT_TRANSITION_INDICATOR, ENABLE_VIEW_TRANSITION};

thread_local! {
    pub static ROOT_MUTATION_CONTEXT: Cell<bool> = const { Cell::new(false) };
    pub static VIEW_TRANSITION_MUTATION_CONTEXT: Cell<bool> = const { Cell::new(false) };
}

pub fn push_root_mutation_context() {
    if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        ROOT_MUTATION_CONTEXT.set(false);
    }
    if ENABLE_VIEW_TRANSITION {
        VIEW_TRANSITION_MUTATION_CONTEXT.set(false);
    }
}

pub fn push_mutation_context() -> bool {
    if !ENABLE_VIEW_TRANSITION {
        return false;
    }
    let prev = VIEW_TRANSITION_MUTATION_CONTEXT.get();
    VIEW_TRANSITION_MUTATION_CONTEXT.set(false);
    prev
}

pub fn pop_mutation_context(prev: bool) {
    if ENABLE_VIEW_TRANSITION {
        if VIEW_TRANSITION_MUTATION_CONTEXT.get() {
            ROOT_MUTATION_CONTEXT.set(true);
        }
        VIEW_TRANSITION_MUTATION_CONTEXT.set(prev);
    }
}

pub fn track_host_mutation() {
    // This is extremely hot function that must be inlined. Don't add more stuff.
    if ENABLE_VIEW_TRANSITION {
        VIEW_TRANSITION_MUTATION_CONTEXT.set(true);
    } else if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        // We only set this if enableViewTransition is not on. Otherwise we track
        // it on the viewTransitionMutationContext and collect it when we pop
        // to avoid more than a single operation in this hot path.
        ROOT_MUTATION_CONTEXT.set(true);
    }
}
