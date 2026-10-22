//! React's shared modules, each its file's.

#[path = "assign.rs"]
pub mod assign;
pub mod environment;
#[path = "ExecutionEnvironment.rs"]
pub mod execution_environment;
#[path = "getPrototypeOf.rs"]
pub mod get_prototype_of;
#[path = "hasOwnProperty.rs"]
pub mod has_own_property;
#[path = "isArray.rs"]
pub mod is_array;
#[path = "noop.rs"]
pub mod noop;
#[path = "objectIs.rs"]
pub mod object_is;
#[path = "ReactSymbols.rs"]
pub mod react_symbols;
#[path = "ReactVersion.rs"]
pub mod react_version;
#[path = "shallowEqual.rs"]
pub mod shallow_equal;
