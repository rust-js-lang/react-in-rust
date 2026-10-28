/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow
 */

export * from './src/ReactFiberReconciler';

// At build time, this module is wrapped as a factory function ($$$reconciler).
// Consumers pass a host config object and get back the reconciler API.
