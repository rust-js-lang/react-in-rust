// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! React DOM's Fizz instruction set: what the inline scripts of a streamed
//! response run, and the external runtime that runs them where those aren't
//! sent. Upstream builds each as a bundle of its own, Closure's and rollup's,
//! apart from react-dom-bindings, so it's a crate of its own. Its modules
//! are its files, each beside the JS rust-js writes from it; its root
//! declares them and writes no JS of its own.

#[path = "ReactDOMFizzInlineClientRenderBoundary.rs"]
pub mod react_dom_fizz_inline_client_render_boundary;
#[path = "ReactDOMFizzInlineCompleteBoundary.rs"]
pub mod react_dom_fizz_inline_complete_boundary;
#[path = "ReactDOMFizzInlineCompleteBoundaryUpgradeToViewTransitions.rs"]
pub mod react_dom_fizz_inline_complete_boundary_upgrade_to_view_transitions;
#[path = "ReactDOMFizzInlineCompleteBoundaryWithStyles.rs"]
pub mod react_dom_fizz_inline_complete_boundary_with_styles;
#[path = "ReactDOMFizzInlineCompleteSegment.rs"]
pub mod react_dom_fizz_inline_complete_segment;
#[path = "ReactDOMFizzInlineFormReplaying.rs"]
pub mod react_dom_fizz_inline_form_replaying;
#[path = "ReactDOMFizzInlineShellTime.rs"]
pub mod react_dom_fizz_inline_shell_time;
#[path = "ReactDOMFizzInstructionSetExternalRuntime.rs"]
pub mod react_dom_fizz_instruction_set_external_runtime;
#[path = "ReactDOMFizzInstructionSetShared.rs"]
pub mod react_dom_fizz_instruction_set_shared;
#[path = "../ReactDOMServerExternalRuntime.rs"]
pub mod react_dom_server_external_runtime;
