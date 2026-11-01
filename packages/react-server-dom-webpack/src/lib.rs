//! React Server Components bindings for DOM using Webpack,
//! `react-server-dom-webpack`: its modules, each its file's.

pub mod client;
// GAP rd580: ReactFlightWebpackReferences is still JS, its client module
// proxy's `Promise.prototype` not in js: its bindings, until it's Rust.
pub mod react_flight_webpack_references;
pub mod server;
pub mod shared;
