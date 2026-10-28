// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::collections::HashSet;

thread_local! {
    /// CSS properties which accept numbers but are not in units of "px".
    static UNITLESS_NUMBERS: HashSet<&'static str> = HashSet::from([
        "animationIterationCount",
        "aspectRatio",
        "borderImageOutset",
        "borderImageSlice",
        "borderImageWidth",
        "boxFlex",
        "boxFlexGroup",
        "boxOrdinalGroup",
        "columnCount",
        "columns",
        "flex",
        "flexGrow",
        "flexPositive",
        "flexShrink",
        "flexNegative",
        "flexOrder",
        "gridArea",
        "gridRow",
        "gridRowEnd",
        "gridRowSpan",
        "gridRowStart",
        "gridColumn",
        "gridColumnEnd",
        "gridColumnSpan",
        "gridColumnStart",
        "fontWeight",
        "lineClamp",
        "lineHeight",
        "opacity",
        "order",
        "orphans",
        "scale",
        "tabSize",
        "widows",
        "zIndex",
        "zoom",
        "fillOpacity", // SVG-related properties
        "floodOpacity",
        "stopOpacity",
        "strokeDasharray",
        "strokeDashoffset",
        "strokeMiterlimit",
        "strokeOpacity",
        "strokeWidth",
        "MozAnimationIterationCount", // Known Prefixed Properties
        "MozBoxFlex", // TODO: Remove these since they shouldn't be used in modern code
        "MozBoxFlexGroup",
        "MozLineClamp",
        "msAnimationIterationCount",
        "msFlex",
        "msZoom",
        "msFlexGrow",
        "msFlexNegative",
        "msFlexOrder",
        "msFlexPositive",
        "msFlexShrink",
        "msGridColumn",
        "msGridColumnSpan",
        "msGridRow",
        "msGridRowSpan",
        "WebkitAnimationIterationCount",
        "WebkitBoxFlex",
        "WebKitBoxFlexGroup",
        "WebkitBoxOrdinalGroup",
        "WebkitColumnCount",
        "WebkitColumns",
        "WebkitFlex",
        "WebkitFlexGrow",
        "WebkitFlexPositive",
        "WebkitFlexShrink",
        "WebkitLineClamp",
    ]);
}

js::export_default! {
    fn is_unitless_number(name: &str) -> bool {
        UNITLESS_NUMBERS.with(|unitless_numbers| unitless_numbers.contains(name))
    }
}
