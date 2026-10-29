/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

import type * as FizzAPI from './src/ReactFizzServer.js';
import type * as HostConfig from './src/ReactFizzConfig.js';

export * from './src/ReactFizzServer.js';

// At build time, this module is wrapped as a factory function ($$$reconciler).
// Consumers pass a host config object and get back the Fizz server API.
declare const createFizzServer: (hostConfig: typeof HostConfig) => typeof FizzAPI;
export default createFizzServer;
