// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

pub fn is_custom_element(tag_name: &str, _props: &Unknown) -> bool {
    if !tag_name.contains('-') {
        return false;
    }
    match tag_name {
        // These are reserved SVG and MathML elements.
        // We don't mind this list too much because we expect it to never grow.
        // The alternative is to track the namespace in a few places which is convoluted.
        // https://html.spec.whatwg.org/multipage/custom-elements.html#custom-elements-core-concepts
        "annotation-xml" | "color-profile" | "font-face" | "font-face-src" | "font-face-uri"
        | "font-face-format" | "font-face-name" | "missing-glyph" => false,
        _ => true,
    }
}

js::export_default!(is_custom_element);
