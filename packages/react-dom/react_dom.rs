//! React DOM, `react-dom`: its modules, each its file's.

#[path = "src/shared/ensureCorrectIsomorphicReactVersion.rs"]
pub mod ensure_correct_isomorphic_react_version;
#[path = "src/shared/ReactDOM.rs"]
pub mod react_dom;
#[path = "src/shared/ReactDOMBrowser.rs"]
pub mod react_dom_browser;
#[path = "src/client/ReactDOMClient.rs"]
pub mod react_dom_client;
#[path = "src/shared/ReactDOMFloat.rs"]
pub mod react_dom_float;
#[path = "src/shared/ReactDOMFlushSync.rs"]
pub mod react_dom_flush_sync;
#[path = "src/client/ReactDOMRoot.rs"]
pub mod react_dom_root;
#[path = "src/ReactDOMSharedInternals.rs"]
pub mod react_dom_shared_internals;
#[path = "src/shared/ReactDOMTypes.rs"]
pub mod react_dom_types;
#[path = "src/test-utils/ReactTestUtils.rs"]
pub mod react_test_utils;
