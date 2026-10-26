// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::react_fiber_root::RootState;
use crate::react_internal_types::FiberRoot;

// This is imported by the event replaying implementation in React DOM. It's
// in a separate file to break a circular dependency between the renderer and
// the reconciler.
pub fn is_root_dehydrated(root: &FiberRoot) -> bool {
    let current_state: &RootState = unsafe { js::cast(root.current.borrow().memoized_state.get()) };
    current_state.is_dehydrated
}
