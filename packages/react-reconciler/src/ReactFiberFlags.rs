// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::react_feature_flags::ENABLE_CREATE_EVENT_HANDLE_API;

pub type Flags = i32;

// Don't change these values. They're used by React Dev Tools.
#[cfg_attr(rust_js, rust_js::name = "NoFlags")]
pub const NO_FLAGS: Flags = /*                      */ 0b0000000000000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "PerformedWork")]
pub const PERFORMED_WORK: Flags = /*                */ 0b0000000000000000000000000000001;
#[cfg_attr(rust_js, rust_js::name = "Placement")]
pub const PLACEMENT: Flags = /*                    */ 0b0000000000000000000000000000010;
#[cfg_attr(rust_js, rust_js::name = "DidCapture")]
pub const DID_CAPTURE: Flags = /*                   */ 0b0000000000000000000000010000000;
#[cfg_attr(rust_js, rust_js::name = "Hydrating")]
pub const HYDRATING: Flags = /*                    */ 0b0000000000000000001000000000000;

// You can change the rest (and add more).
#[cfg_attr(rust_js, rust_js::name = "Update")]
pub const UPDATE: Flags = /*                       */ 0b0000000000000000000000000000100;
#[cfg_attr(rust_js, rust_js::name = "Cloned")]
pub const CLONED: Flags = /*                       */ 0b0000000000000000000000000001000;

#[cfg_attr(rust_js, rust_js::name = "ChildDeletion")]
pub const CHILD_DELETION: Flags = /*                */ 0b0000000000000000000000000010000;
#[cfg_attr(rust_js, rust_js::name = "ContentReset")]
pub const CONTENT_RESET: Flags = /*                 */ 0b0000000000000000000000000100000;
#[cfg_attr(rust_js, rust_js::name = "Callback")]
pub const CALLBACK: Flags = /*                     */ 0b0000000000000000000000001000000;
/* Used by DidCapture:                            0b0000000000000000000000010000000; */

#[cfg_attr(rust_js, rust_js::name = "ForceClientRender")]
pub const FORCE_CLIENT_RENDER: Flags = /*            */ 0b0000000000000000000000100000000;
#[cfg_attr(rust_js, rust_js::name = "Ref")]
pub const REF: Flags = /*                          */ 0b0000000000000000000001000000000;
#[cfg_attr(rust_js, rust_js::name = "Snapshot")]
pub const SNAPSHOT: Flags = /*                     */ 0b0000000000000000000010000000000;
#[cfg_attr(rust_js, rust_js::name = "Passive")]
pub const PASSIVE: Flags = /*                      */ 0b0000000000000000000100000000000;
/* Used by Hydrating:                             0b0000000000000000001000000000000; */

#[cfg_attr(rust_js, rust_js::name = "Visibility")]
pub const VISIBILITY: Flags = /*                   */ 0b0000000000000000010000000000000;
#[cfg_attr(rust_js, rust_js::name = "StoreConsistency")]
pub const STORE_CONSISTENCY: Flags = /*             */ 0b0000000000000000100000000000000;

// It's OK to reuse these bits because these flags are mutually exclusive for
// different fiber types. We should really be doing this for as many flags as
// possible, because we're about to run out of bits.
#[cfg_attr(rust_js, rust_js::name = "Hydrate")]
pub const HYDRATE: Flags = CALLBACK;
#[cfg_attr(rust_js, rust_js::name = "ScheduleRetry")]
pub const SCHEDULE_RETRY: Flags = STORE_CONSISTENCY;
#[cfg_attr(rust_js, rust_js::name = "ShouldSuspendCommit")]
pub const SHOULD_SUSPEND_COMMIT: Flags = VISIBILITY;
#[cfg_attr(rust_js, rust_js::name = "ViewTransitionNamedMount")]
pub const VIEW_TRANSITION_NAMED_MOUNT: Flags = SHOULD_SUSPEND_COMMIT;
#[cfg_attr(rust_js, rust_js::name = "DidDefer")]
pub const DID_DEFER: Flags = CONTENT_RESET;
#[cfg_attr(rust_js, rust_js::name = "FormReset")]
pub const FORM_RESET: Flags = SNAPSHOT;
#[cfg_attr(rust_js, rust_js::name = "AffectedParentLayout")]
pub const AFFECTED_PARENT_LAYOUT: Flags = CONTENT_RESET;

#[cfg_attr(rust_js, rust_js::name = "LifecycleEffectMask")]
pub const LIFECYCLE_EFFECT_MASK: Flags =
    PASSIVE | UPDATE | CALLBACK | REF | SNAPSHOT | STORE_CONSISTENCY;

// Union of all commit flags (flags with the lifetime of a particular commit)
#[cfg_attr(rust_js, rust_js::name = "HostEffectMask")]
pub const HOST_EFFECT_MASK: Flags = /*               */ 0b0000000000000000111111111111111;

// These are not really side effects, but we still reuse this field.
#[cfg_attr(rust_js, rust_js::name = "Incomplete")]
pub const INCOMPLETE: Flags = /*                   */ 0b0000000000000001000000000000000;
#[cfg_attr(rust_js, rust_js::name = "ShouldCapture")]
pub const SHOULD_CAPTURE: Flags = /*                */ 0b0000000000000010000000000000000;
#[cfg_attr(rust_js, rust_js::name = "ForceUpdateForLegacySuspense")]
pub const FORCE_UPDATE_FOR_LEGACY_SUSPENSE: Flags = /* */ 0b0000000000000100000000000000000;
#[cfg_attr(rust_js, rust_js::name = "DidPropagateContext")]
pub const DID_PROPAGATE_CONTEXT: Flags = /*          */ 0b0000000000001000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "NeedsPropagation")]
pub const NEEDS_PROPAGATION: Flags = /*             */ 0b0000000000010000000000000000000;

// Static tags describe aspects of a fiber that are not specific to a render,
// e.g. a fiber uses a passive effect (even if there are no updates on this particular render).
// This enables us to defer more work in the unmount case,
// since we can defer traversing the tree during layout to look for Passive effects,
// and instead rely on the static flag as a signal that there may be cleanup work.
#[cfg_attr(rust_js, rust_js::name = "Forked")]
pub const FORKED: Flags = /*                       */ 0b0000000000100000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "SnapshotStatic")]
pub const SNAPSHOT_STATIC: Flags = /*               */ 0b0000000001000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "LayoutStatic")]
pub const LAYOUT_STATIC: Flags = /*                 */ 0b0000000010000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "RefStatic")]
pub const REF_STATIC: Flags = LAYOUT_STATIC;
#[cfg_attr(rust_js, rust_js::name = "PassiveStatic")]
pub const PASSIVE_STATIC: Flags = /*                */ 0b0000000100000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "MaySuspendCommit")]
pub const MAY_SUSPEND_COMMIT: Flags = /*             */ 0b0000001000000000000000000000000;
// ViewTransitionNamedStatic tracks explicitly name ViewTransition components deeply
// that might need to be visited during clean up. This is similar to SnapshotStatic
// if there was any other use for it. It also needs to run in the same phase as
// MaySuspendCommit tracking.
#[cfg_attr(rust_js, rust_js::name = "ViewTransitionNamedStatic")]
pub const VIEW_TRANSITION_NAMED_STATIC: Flags =
    /*    */
    SNAPSHOT_STATIC | MAY_SUSPEND_COMMIT;
// ViewTransitionStatic tracks whether there are an ViewTransition components from
// the nearest HostComponent down. It resets at every HostComponent level.
#[cfg_attr(rust_js, rust_js::name = "ViewTransitionStatic")]
pub const VIEW_TRANSITION_STATIC: Flags = /*         */ 0b0000010000000000000000000000000;
// ViewTransitionStaticParent tracks whether there are ViewTransition components
// with parentEnter/parentExit props. Unlike ViewTransitionStatic, this is NOT
// cleared by HostComponents so it can be used to skip subtrees in parent walks.
#[cfg_attr(rust_js, rust_js::name = "ViewTransitionStaticParent")]
pub const VIEW_TRANSITION_STATIC_PARENT: Flags = /*   */ 0b1000000000000000000000000000000;
// Tracks whether a HostPortal is present in the tree.
#[cfg_attr(rust_js, rust_js::name = "PortalStatic")]
pub const PORTAL_STATIC: Flags = /*                 */ 0b0000100000000000000000000000000;

// Flag used to identify newly inserted fibers. It isn't reset after commit unlike `Placement`.
#[cfg_attr(rust_js, rust_js::name = "PlacementDEV")]
pub const PLACEMENT_DEV: Flags = /*                 */ 0b0001000000000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "MountLayoutDev")]
pub const MOUNT_LAYOUT_DEV: Flags = /*               */ 0b0010000000000000000000000000000;
#[cfg_attr(rust_js, rust_js::name = "MountPassiveDev")]
pub const MOUNT_PASSIVE_DEV: Flags = /*              */ 0b0100000000000000000000000000000;

// Groups of flags that are used in the commit phase to skip over trees that
// don't contain effects, by checking subtreeFlags.

#[cfg_attr(rust_js, rust_js::name = "BeforeMutationMask")]
pub const BEFORE_MUTATION_MASK: Flags = SNAPSHOT
    | if ENABLE_CREATE_EVENT_HANDLE_API {
        // createEventHandle needs to visit deleted and hidden trees to
        // fire beforeblur
        // TODO: Only need to visit Deletions during BeforeMutation phase if an
        // element is focused.
        UPDATE | CHILD_DELETION | VISIBILITY
    } else {
        0
    };

// For View Transition support we use the snapshot phase to scan the tree for potentially
// affected ViewTransition components.
#[cfg_attr(rust_js, rust_js::name = "BeforeAndAfterMutationTransitionMask")]
pub const BEFORE_AND_AFTER_MUTATION_TRANSITION_MASK: Flags =
    SNAPSHOT | UPDATE | PLACEMENT | CHILD_DELETION | VISIBILITY | CONTENT_RESET;

#[cfg_attr(rust_js, rust_js::name = "MutationMask")]
pub const MUTATION_MASK: Flags =
    PLACEMENT | UPDATE | CHILD_DELETION | CONTENT_RESET | REF | HYDRATING | VISIBILITY | FORM_RESET;
#[cfg_attr(rust_js, rust_js::name = "LayoutMask")]
pub const LAYOUT_MASK: Flags = UPDATE | CALLBACK | REF | VISIBILITY;

// TODO: Split into PassiveMountMask and PassiveUnmountMask
#[cfg_attr(rust_js, rust_js::name = "PassiveMask")]
pub const PASSIVE_MASK: Flags = PASSIVE | VISIBILITY | CHILD_DELETION;

// For View Transitions we need to visit anything we visited in the snapshot phase to
// restore the view-transition-name after committing the transition.
#[cfg_attr(rust_js, rust_js::name = "PassiveTransitionMask")]
pub const PASSIVE_TRANSITION_MASK: Flags = PASSIVE_MASK | UPDATE | PLACEMENT;

// Union of tags that don't get reset on clones.
// This allows certain concepts to persist without recalculating them,
// e.g. whether a subtree contains passive effects or portals.
#[cfg_attr(rust_js, rust_js::name = "StaticMask")]
pub const STATIC_MASK: Flags = LAYOUT_STATIC
    | PASSIVE_STATIC
    | REF_STATIC
    | MAY_SUSPEND_COMMIT
    | VIEW_TRANSITION_STATIC
    | VIEW_TRANSITION_STATIC_PARENT
    | VIEW_TRANSITION_NAMED_STATIC
    | PORTAL_STATIC
    | FORKED;
