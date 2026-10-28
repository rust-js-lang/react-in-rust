// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::{AssertUnwindSafe, catch_unwind};

use webapi::{Document, Element};

js::export_default! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    fn get_active_element(doc: Option<&'static Document>) -> Option<&'static Element> {
        let doc = doc.or_else(|| js::is_defined(webapi::document).then_some(webapi::document))?;
        match catch_unwind(AssertUnwindSafe(|| {
            doc.active_element()
                .or_else(|| doc.body().map(|body| &**body))
        })) {
            Ok(active_element) => active_element,
            Err(_) => doc.body().map(|body| &**body),
        }
    }
}
