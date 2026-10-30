// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! This file is compiled to a standalone browser script by rollup and loaded by Fizz
//!  clients. Therefore, it should be fast and not have many external dependencies.

use webapi::events::DOMContentLoaded;
use webapi::{
    EventTargetExt, HTMLElement, MutationObserverInit, MutationRecord, Node, document,
    html_element, mutation_observer, window,
};

use crate::react_dom_fizz_instruction_set_shared::{RC, RR, RS, RX};

// Imports are resolved statically by the closure compiler in release bundles
// and by rollup in jest unit tests
js::import!("./src/server/fizz-instruction-set/ReactDOMFizzInstructionSetExternalRuntime.js");

js::on_load! {
    if let Some(body) = document.body() {
        if document.ready_state() == "loading" {
            install_fizz_instr_observer(body);
        }
        handle_existing_nodes(body);
    } else {
        // Document must be loading -- body may not exist yet if the fizz external
        // runtime is sent in <head> (e.g. as a preinit resource)
        let dom_body_observer = mutation_observer::new(Box::new(|_, dom_body_observer| {
            // We expect the body node to be stable once parsed / created
            if let Some(body) = document.body() {
                if document.ready_state() == "loading" {
                    install_fizz_instr_observer(body);
                }
                handle_existing_nodes(body);

                // We can call disconnect without takeRecord here,
                // since we only expect a single document.body
                dom_body_observer.disconnect();
            }
        }));
        // documentElement must already exist at this point
        dom_body_observer.observe(
            document.document_element().expect("a loading document has its element"),
            Some(MutationObserverInit {
                child_list: Some(true),
                ..Default::default()
            }),
        );
    }
}

fn handle_existing_nodes(target: &HTMLElement) {
    let existing_nodes = target.query_selector_all("template");
    for existing_node in webapi::node_list::iter(existing_nodes) {
        handle_node(existing_node);
    }
}

fn install_fizz_instr_observer(target: &Node) {
    fn handle_mutations(mutations: &[&MutationRecord]) {
        for mutation in mutations {
            let added_nodes = mutation.added_nodes();
            for added_node in webapi::node_list::iter(added_nodes) {
                if added_node.parent_node().is_some() {
                    handle_node(added_node);
                }
            }
        }
    }

    let fizz_instr_observer =
        mutation_observer::new(Box::new(|mutations, _| handle_mutations(mutations)));
    // We assume that instruction data nodes are eventually appended to the
    // body, even if Fizz is streaming to a shell / subtree.
    fizz_instr_observer.observe(
        target,
        Some(MutationObserverInit {
            child_list: Some(true),
            ..Default::default()
        }),
    );
    window.add_event_listener(
        DOMContentLoaded,
        move |_: &webapi::Event| {
            handle_mutations(&fizz_instr_observer.take_records());
            fizz_instr_observer.disconnect();
        },
        None,
    );
}

fn handle_node(node: &Node) {
    if node.node_type() != 1.0 || !js::truthy(js::get(node, "dataset")) {
        return;
    }
    let node = html_element::unchecked_from(node);
    let dataset = node.dataset();
    let bid = || {
        dataset
            .get("bid")
            .expect("an instruction names its boundary")
    };
    if dataset.get("rxi").is_some() {
        RX(
            &bid(),
            dataset.get("dgst").as_deref(),
            dataset.get("msg").as_deref(),
            dataset.get("stck").as_deref(),
            dataset.get("cstck").as_deref(),
        );
        node.remove();
    } else if dataset.get("rri").is_some() {
        // Convert styles here, since its type is Array<Array<string>>
        let styles = js::json::parse(&dataset.get("sty").unwrap_or_default()).unwrap();
        let styles: &[Vec<String>] = unsafe { js::cast(styles) };
        RR(&bid(), &dataset.get("sid").unwrap_or_default(), styles);
        node.remove();
    } else if dataset.get("rci").is_some() {
        RC(&bid(), &dataset.get("sid").unwrap_or_default());
        node.remove();
    } else if dataset.get("rsi").is_some() {
        RS(
            &dataset.get("sid").unwrap_or_default(),
            &dataset.get("pid").unwrap_or_default(),
        );
        node.remove();
    }
}
