// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{RegExp, Unknown, reg_exp};

// A javascript: URL can contain leading C0 control or \u0020 SPACE,
// and any newline or tab are filtered out as if they're not part of the URL.
// https://url.spec.whatwg.org/#url-parsing
// Tab or newline are defined as \r\n\t:
// https://infra.spec.whatwg.org/#ascii-tab-or-newline
// A C0 control is a code point in the range \u0000 NULL to \u001F
// INFORMATION SEPARATOR ONE, inclusive:
// https://infra.spec.whatwg.org/#c0-control-or-space

thread_local! {
    static IS_JAVA_SCRIPT_PROTOCOL: &'static RegExp = reg_exp::new(
        "^[\\u0000-\\u001F ]*j[\\r\\n\\t]*a[\\r\\n\\t]*v[\\r\\n\\t]*a[\\r\\n\\t]*s[\\r\\n\\t]*c[\\r\\n\\t]*r[\\r\\n\\t]*i[\\r\\n\\t]*p[\\r\\n\\t]*t[\\r\\n\\t]*\\:",
        "i",
    );
}

#[cfg_attr(rust_js, rust_js::name = "sanitizeURL")]
pub fn sanitize_url(url: Option<&'static Unknown>) -> Option<&'static Unknown> {
    // We should never have symbols here because they get filtered out elsewhere.
    // eslint-disable-next-line react-internal/safe-string-coercion
    if IS_JAVA_SCRIPT_PROTOCOL.with(|is_java_script_protocol| {
        is_java_script_protocol.test(&js::concat_text(url).unwrap())
    }) {
        // Return a different javascript: url that doesn't cause any side-effects and just
        // throws if ever visited.
        // eslint-disable-next-line no-script-url
        return Some(js::unknown(
            "javascript:throw new Error('React has blocked a javascript: URL as a security precaution.')",
        ));
    }
    url
}

js::export_default!(sanitize_url);
