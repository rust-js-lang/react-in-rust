// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::react_feature_flags::CompilerLint;

// This fork provides www-specific feature flag values for
// eslint-plugin-react-hooks without pulling in the full
// ReactFeatureFlags.www.js fork. The full fork imports dynamic flags
// via require('ReactFeatureFlags'), which is a www Haste module that
// doesn't exist in Node.js environments where the ESLint plugin runs.
//
// Only eprh_* flags are needed by the ESLint plugin.

#[cfg_attr(rust_js, rust_js::name = "eprh_enableUseKeyedStateCompilerLint")]
pub const EPRH_ENABLE_USE_KEYED_STATE_COMPILER_LINT: bool = true;
#[cfg_attr(
    rust_js,
    rust_js::name = "eprh_enableVerboseNoSetStateInEffectCompilerLint"
)]
pub const EPRH_ENABLE_VERBOSE_NO_SET_STATE_IN_EFFECT_COMPILER_LINT: bool = true;
#[cfg_attr(
    rust_js,
    rust_js::name = "eprh_enableExhaustiveEffectDependenciesCompilerLint"
)]
pub const EPRH_ENABLE_EXHAUSTIVE_EFFECT_DEPENDENCIES_COMPILER_LINT: CompilerLint =
    CompilerLint::ExtraOnly;
