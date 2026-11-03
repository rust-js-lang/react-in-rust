//! React Server Components bindings for DOM using Webpack,
//! `react-server-dom-webpack`: its modules, each its file's.

pub mod client;
#[path = "ReactFlightWebpackPlugin.rs"]
pub mod react_flight_webpack_plugin;
#[path = "ReactFlightWebpackReferences.rs"]
pub mod react_flight_webpack_references;
pub mod server;
pub mod shared;
