// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use webapi::Event;

/// A native event a plugin is given: an `Event`, a `KeyboardEvent`, a
/// `MouseEvent` or a `TouchEvent`.
pub type AnyNativeEvent = Event;

pub type PluginName = String;

pub use crate::event_system_flags::EventSystemFlags;
