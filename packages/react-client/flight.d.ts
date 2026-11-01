/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

import type * as FlightClientAPI from './src/ReactFlightClient.js';
import type * as HostConfig from './src/ReactFlightClientConfig.js';

export * from './src/ReactFlightClient.js';

// At build time, this module is wrapped as a factory function ($$$reconciler).
// Consumers pass a host config object and get back the Flight client API.
declare const createFlightClient: (hostConfig: typeof HostConfig) => typeof FlightClientAPI;
export default createFlightClient;
