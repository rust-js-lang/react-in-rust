//! React's renderer for testing the reconciler, `react-noop-renderer`: its
//! modules, each its file's.

#[path = "src/createReactNoop.rs"]
pub mod create_react_noop;
#[path = "src/ReactFiberConfigNoop.rs"]
pub mod react_fiber_config_noop;
#[path = "src/ReactFiberConfigNoopHydration.rs"]
pub mod react_fiber_config_noop_hydration;
#[path = "src/ReactFiberConfigNoopNoMutation.rs"]
pub mod react_fiber_config_noop_no_mutation;
#[path = "src/ReactFiberConfigNoopNoPersistence.rs"]
pub mod react_fiber_config_noop_no_persistence;
#[path = "src/ReactFiberConfigNoopResources.rs"]
pub mod react_fiber_config_noop_resources;
#[path = "src/ReactFiberConfigNoopScopes.rs"]
pub mod react_fiber_config_noop_scopes;
#[path = "src/ReactFiberConfigNoopSingletons.rs"]
pub mod react_fiber_config_noop_singletons;
#[path = "src/ReactFiberConfigNoopTestSelectors.rs"]
pub mod react_fiber_config_noop_test_selectors;
