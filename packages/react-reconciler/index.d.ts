/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

import type * as ReconcilerAPI from './src/ReactFiberReconciler.js';
import type * as HostConfig from './src/ReactFiberConfig.js';

export * from './src/ReactFiberReconciler.js';

// At build time, this module is wrapped as a factory function ($$$reconciler).
// Consumers pass a host config object and get back the reconciler API.
declare const createReconciler: (hostConfig: typeof HostConfig) => typeof ReconcilerAPI;
export default createReconciler;
