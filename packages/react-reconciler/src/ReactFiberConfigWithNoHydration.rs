// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Renderers that don't support hydration
// can re-export everything from this module.

use js::Unknown;

fn shim() -> ! {
    panic!(concat!(
        "The current renderer does not support hydration. ",
        "This error is likely caused by a bug in React. ",
        "Please file an issue."
    ));
}

/// What this module gives for each function the renderer hasn't: it
/// throws, whatever it's given.
type Shim = fn() -> !;

// Hydration (when unsupported)
pub type ActivityInstance = Unknown;
pub type SuspenseInstance = Unknown;
#[cfg_attr(rust_js, rust_js::name = "supportsHydration")]
pub const SUPPORTS_HYDRATION: bool = false;
#[cfg_attr(rust_js, rust_js::name = "isSuspenseInstancePending")]
pub const IS_SUSPENSE_INSTANCE_PENDING: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "isSuspenseInstanceFallback")]
pub const IS_SUSPENSE_INSTANCE_FALLBACK: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getSuspenseInstanceFallbackErrorDetails")]
pub const GET_SUSPENSE_INSTANCE_FALLBACK_ERROR_DETAILS: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "registerSuspenseInstanceRetry")]
pub const REGISTER_SUSPENSE_INSTANCE_RETRY: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "canHydrateFormStateMarker")]
pub const CAN_HYDRATE_FORM_STATE_MARKER: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "isFormStateMarkerMatching")]
pub const IS_FORM_STATE_MARKER_MATCHING: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getNextHydratableSibling")]
pub const GET_NEXT_HYDRATABLE_SIBLING: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getNextHydratableSiblingAfterSingleton")]
pub const GET_NEXT_HYDRATABLE_SIBLING_AFTER_SINGLETON: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getFirstHydratableChild")]
pub const GET_FIRST_HYDRATABLE_CHILD: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getFirstHydratableChildWithinContainer")]
pub const GET_FIRST_HYDRATABLE_CHILD_WITHIN_CONTAINER: Shim = shim;
#[cfg_attr(
    rust_js,
    rust_js::name = "getFirstHydratableChildWithinActivityInstance"
)]
pub const GET_FIRST_HYDRATABLE_CHILD_WITHIN_ACTIVITY_INSTANCE: Shim = shim;
#[cfg_attr(
    rust_js,
    rust_js::name = "getFirstHydratableChildWithinSuspenseInstance"
)]
pub const GET_FIRST_HYDRATABLE_CHILD_WITHIN_SUSPENSE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "getFirstHydratableChildWithinSingleton")]
pub const GET_FIRST_HYDRATABLE_CHILD_WITHIN_SINGLETON: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "canHydrateInstance")]
pub const CAN_HYDRATE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "canHydrateTextInstance")]
pub const CAN_HYDRATE_TEXT_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "canHydrateActivityInstance")]
pub const CAN_HYDRATE_ACTIVITY_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "canHydrateSuspenseInstance")]
pub const CAN_HYDRATE_SUSPENSE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hydrateInstance")]
pub const HYDRATE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hydrateTextInstance")]
pub const HYDRATE_TEXT_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hydrateActivityInstance")]
pub const HYDRATE_ACTIVITY_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hydrateSuspenseInstance")]
pub const HYDRATE_SUSPENSE_INSTANCE: Shim = shim;
#[cfg_attr(
    rust_js,
    rust_js::name = "getNextHydratableInstanceAfterActivityInstance"
)]
pub const GET_NEXT_HYDRATABLE_INSTANCE_AFTER_ACTIVITY_INSTANCE: Shim = shim;
#[cfg_attr(
    rust_js,
    rust_js::name = "getNextHydratableInstanceAfterSuspenseInstance"
)]
pub const GET_NEXT_HYDRATABLE_INSTANCE_AFTER_SUSPENSE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "finalizeHydratedChildren")]
pub const FINALIZE_HYDRATED_CHILDREN: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "commitHydratedInstance")]
pub const COMMIT_HYDRATED_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "commitHydratedContainer")]
pub const COMMIT_HYDRATED_CONTAINER: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "commitHydratedActivityInstance")]
pub const COMMIT_HYDRATED_ACTIVITY_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "commitHydratedSuspenseInstance")]
pub const COMMIT_HYDRATED_SUSPENSE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "flushHydrationEvents")]
pub const FLUSH_HYDRATION_EVENTS: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "clearActivityBoundary")]
pub const CLEAR_ACTIVITY_BOUNDARY: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "clearSuspenseBoundary")]
pub const CLEAR_SUSPENSE_BOUNDARY: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "clearActivityBoundaryFromContainer")]
pub const CLEAR_ACTIVITY_BOUNDARY_FROM_CONTAINER: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "clearSuspenseBoundaryFromContainer")]
pub const CLEAR_SUSPENSE_BOUNDARY_FROM_CONTAINER: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "hideDehydratedBoundary")]
pub const HIDE_DEHYDRATED_BOUNDARY: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "unhideDehydratedBoundary")]
pub const UNHIDE_DEHYDRATED_BOUNDARY: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "shouldDeleteUnhydratedTailInstances")]
pub const SHOULD_DELETE_UNHYDRATED_TAIL_INSTANCES: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "diffHydratedPropsForDevWarnings")]
pub const DIFF_HYDRATED_PROPS_FOR_DEV_WARNINGS: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "diffHydratedTextForDevWarnings")]
pub const DIFF_HYDRATED_TEXT_FOR_DEV_WARNINGS: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "describeHydratableInstanceForDevWarnings")]
pub const DESCRIBE_HYDRATABLE_INSTANCE_FOR_DEV_WARNINGS: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "validateHydratableInstance")]
pub const VALIDATE_HYDRATABLE_INSTANCE: Shim = shim;
#[cfg_attr(rust_js, rust_js::name = "validateHydratableTextInstance")]
pub const VALIDATE_HYDRATABLE_TEXT_INSTANCE: Shim = shim;
