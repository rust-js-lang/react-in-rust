// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, dict};
use webapi::{Element, html_input_element, html_select_element};

// Below code forked from dom-accessibility-api

thread_local! {
    static TAG_TO_ROLE_MAPPINGS: &'static Dict<&'static str> = dict::from_entries(vec![
        ("ARTICLE", "article"),
        ("ASIDE", "complementary"),
        ("BODY", "document"),
        ("BUTTON", "button"),
        ("DATALIST", "listbox"),
        ("DD", "definition"),
        ("DETAILS", "group"),
        ("DIALOG", "dialog"),
        ("DT", "term"),
        ("FIELDSET", "group"),
        ("FIGURE", "figure"),
        // WARNING: Only with an accessible name
        ("FORM", "form"),
        ("FOOTER", "contentinfo"),
        ("H1", "heading"),
        ("H2", "heading"),
        ("H3", "heading"),
        ("H4", "heading"),
        ("H5", "heading"),
        ("H6", "heading"),
        ("HEADER", "banner"),
        ("HR", "separator"),
        ("LEGEND", "legend"),
        ("LI", "listitem"),
        ("MATH", "math"),
        ("MAIN", "main"),
        ("MENU", "list"),
        ("NAV", "navigation"),
        ("OL", "list"),
        ("OPTGROUP", "group"),
        // WARNING: Only in certain context
        ("OPTION", "option"),
        ("OUTPUT", "status"),
        ("PROGRESS", "progressbar"),
        // WARNING: Only with an accessible name
        ("SECTION", "region"),
        ("SUMMARY", "button"),
        ("TABLE", "table"),
        ("TBODY", "rowgroup"),
        ("TEXTAREA", "textbox"),
        ("TFOOT", "rowgroup"),
        // WARNING: Only in certain context
        ("TD", "cell"),
        ("TH", "columnheader"),
        ("THEAD", "rowgroup"),
        ("TR", "row"),
        ("UL", "list"),
    ]);
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_implicit_role(element: &Element) -> Option<String> {
    // $FlowFixMe[invalid-computed-prop]
    let mapped_by_tag = TAG_TO_ROLE_MAPPINGS
        .with(|tag_to_role_mappings| dict::get(tag_to_role_mappings, &element.tag_name()).copied());
    if let Some(mapped_by_tag) = mapped_by_tag {
        return Some(mapped_by_tag.to_owned());
    }

    match element.tag_name().as_str() {
        "A" | "AREA" | "LINK" => {
            if element.has_attribute("href") {
                return Some("link".to_owned());
            }
        }
        "IMG" => {
            if js::string::length(
                &element
                    .get_attribute("alt")
                    .filter(|alt| !alt.is_empty())
                    .unwrap_or_default(),
            ) > 0
            {
                return Some("img".to_owned());
            }
        }
        "INPUT" => {
            let r#type = html_input_element::unchecked_from(element).r#type();
            return match r#type.as_str() {
                "button" | "image" | "reset" | "submit" => Some("button".to_owned()),
                "checkbox" | "radio" => Some(r#type),
                "range" => Some("slider".to_owned()),
                "email" | "tel" | "text" | "url" => {
                    if element.has_attribute("list") {
                        return Some("combobox".to_owned());
                    }
                    Some("textbox".to_owned())
                }
                "search" => {
                    if element.has_attribute("list") {
                        return Some("combobox".to_owned());
                    }
                    Some("searchbox".to_owned())
                }
                _ => None,
            };
        }

        "SELECT" => {
            if element.has_attribute("multiple")
                || html_select_element::unchecked_from(element).size() > 1.0
            {
                return Some("listbox".to_owned());
            }
            return Some("combobox".to_owned());
        }
        _ => {}
    }

    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_explicit_roles(element: &Element) -> Option<Vec<String>> {
    let role = element.get_attribute("role");
    if let Some(role) = role.filter(|role| !role.is_empty()) {
        return Some(js::string::split(&js::string::trim(&role), " "));
    }

    None
}

// https://w3c.github.io/html-aria/#document-conformance-requirements-for-use-of-aria-attributes-in-html
pub fn has_role(element: &Element, role: &str) -> bool {
    let explicit_roles = get_explicit_roles(element);
    if explicit_roles
        .is_some_and(|explicit_roles| explicit_roles.iter().any(|explicit| explicit == role))
    {
        return true;
    }

    Some(role) == get_implicit_role(element).as_deref()
}
