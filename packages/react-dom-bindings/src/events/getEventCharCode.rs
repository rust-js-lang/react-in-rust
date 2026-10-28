// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use webapi::KeyboardEvent;

js::export_default! {
    /// `charCode` represents the actual "character code" and is safe to use with
    /// `String.fromCharCode`. As such, only keys that correspond to printable
    /// characters produce a valid `charCode`, the only exception to this is Enter.
    /// The Tab-key is considered non-printable and does not have a `charCode`,
    /// presumably because it does not produce a tab-character in browsers.
    ///
    /// @param {object} nativeEvent Native browser event.
    /// @return {number} Normalized `charCode` property.
    pub fn get_event_char_code(native_event: &KeyboardEvent) -> f64 {
        let mut char_code;
        let key_code = native_event.key_code();

        if js::has(native_event, "charCode") {
            char_code = native_event.char_code();

            // FF does not set `charCode` for the Enter-key, check against `keyCode`.
            if char_code == 0.0 && key_code == 13.0 {
                char_code = 13.0;
            }
        } else {
            // IE8 does not implement `charCode`, but `keyCode` has the correct value.
            char_code = key_code;
        }

        // IE and Edge (on Windows) and Chrome / Safari (on Windows and Linux)
        // report Enter as charCode 10 when ctrl is pressed.
        if char_code == 10.0 {
            char_code = 13.0;
        }

        // Some non-printable keys are reported in `charCode`/`keyCode`, discard them.
        // Must not discard the (non-)printable Enter-key.
        if char_code >= 32.0 || char_code == 13.0 {
            return char_code;
        }

        0.0
    }
}
