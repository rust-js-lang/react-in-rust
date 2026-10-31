// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::react_version::REACT_VERSION as REACT_DOM_PACKAGE_VERSION;

unsafe extern "Rust" {
    // The `react` package's own version, which may be another copy's.
    #[link_name = "react#* as IsomorphicReactPackage.version"]
    safe static ISOMORPHIC_REACT_PACKAGE_VERSION: &'static str;
}

pub fn ensure_correct_isomorphic_react_version() {
    let isomorphic_react_package_version = ISOMORPHIC_REACT_PACKAGE_VERSION;
    if isomorphic_react_package_version != REACT_DOM_PACKAGE_VERSION {
        panic!(
            "Incompatible React versions: The \"react\" and \"react-dom\" packages must have the exact same version. Instead got:\n  - react:      {isomorphic_react_package_version}\n  - react-dom:  {REACT_DOM_PACKAGE_VERSION}\nLearn more: https://react.dev/warnings/version-mismatch"
        );
    }
}
