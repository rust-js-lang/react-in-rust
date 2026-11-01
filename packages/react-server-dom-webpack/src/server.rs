//! The server side, `src/server/`: its modules, each its file's, and the
//! bindings of the server's modules that are still JS.

// GAP rd588: ReactFlightDOMServerNode is still JS: node's `Readable` has no
// `"data"`, `"error"` nor `"end"` events, node has no `Duplex`, and busboy
// has no binding.

#[path = "server/react_flight_action_server.rs"]
pub mod react_flight_action_server;
#[path = "server/ReactFlightDOMServerBrowser.rs"]
pub mod react_flight_dom_server_browser;
#[path = "server/ReactFlightDOMServerEdge.rs"]
pub mod react_flight_dom_server_edge;
#[path = "server/react_flight_server.rs"]
pub mod react_flight_server;
#[path = "server/ReactFlightServerConfigWebpackBundler.rs"]
pub mod react_flight_server_config_webpack_bundler;
