/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @emails react-core
 * @jest-environment node
 */

'use strict';

// react-in-rust: a reference that resolves after its model was parsed gives
// the model's handler the referenced value as it is, `handler.value =
// mappedValue`, `undefined` too: the root of a model that is only a
// reference to a row that resolves to `undefined` is `undefined`, never
// `null`. No upstream test checks it.
describe('ReactFlight references resolving to undefined', () => {
  let ReactFlightClient;

  beforeEach(() => {
    jest.resetModules();
    ReactFlightClient = require('react-client/flight');
  });

  // The noop renderer's client config, fed one row at a time.
  function createClient() {
    const decoderOptions = {stream: true};
    const {createResponse, createStreamState, processBinaryChunk, getRoot} =
      ReactFlightClient({
        createStringDecoder(preserveByteOrderMark = false) {
          return new TextDecoder('utf-8', {ignoreBOM: preserveByteOrderMark});
        },
        readPartialStringChunk(decoder, buffer) {
          return decoder.decode(buffer, decoderOptions);
        },
        readFinalStringChunk(decoder, buffer) {
          return decoder.decode(buffer);
        },
        resolveClientReference(bundlerConfig, idx) {
          return idx;
        },
        prepareDestinationForModule(moduleLoading, metadata) {},
        preloadModule(idx) {},
        requireModule(idx) {
          throw new Error('No modules in this test.');
        },
        bindToConsole(methodName, args, badgeName) {
          return Function.prototype.bind.apply(
            console[methodName],
            [console].concat(args),
          );
        },
        checkEvalAvailabilityOnceDev() {},
      });
    const response = createResponse(
      null,
      null,
      null,
      undefined,
      undefined,
      undefined,
      undefined,
      false,
      undefined,
      false, // replayConsole
      undefined,
      undefined,
      undefined,
      undefined,
    );
    const streamState = createStreamState(response, null);
    const encoder = new TextEncoder();
    return {
      root: getRoot(response),
      send(row) {
        processBinaryChunk(response, streamState, encoder.encode(row + '\n'));
      },
    };
  }

  async function settle() {
    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }
  }

  it('resolves a root that waits on a row resolving to undefined to undefined', async () => {
    const client = createClient();
    // The root is only a reference to row 1, which isn't here yet.
    client.send('0:"$1"');
    let resolved = 'not yet';
    client.root.then(value => {
      resolved = value;
    });
    await settle();
    expect(resolved).toBe('not yet');
    client.send('1:"$undefined"');
    await settle();
    expect(resolved).toBe(undefined);
  });

  it('resolves a root that waits on a row resolving to null to null', async () => {
    const client = createClient();
    client.send('0:"$1"');
    let resolved = 'not yet';
    client.root.then(value => {
      resolved = value;
    });
    await settle();
    client.send('1:null');
    await settle();
    expect(resolved).toBe(null);
  });
});
