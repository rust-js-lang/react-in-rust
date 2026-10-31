// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::react_symbols::REACT_RECOVERABLE_TYPE;
use shared::react_types::{ReactRecoverable, ReactRecoverableReason};

pub fn browser(reason: Option<ReactRecoverableReason>) -> ReactRecoverable {
    // This also runs in the browser, where the reason is never observed. Keep the
    // value cheap and let an SSR renderer initialize the error if it defers work.
    ReactRecoverable {
        r#typeof: REACT_RECOVERABLE_TYPE,
        reason,
    }
}
