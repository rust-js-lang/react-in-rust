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

// react-in-rust: upstream's resolveConsoleEntry replays console entries in
// the order they arrive. An entry that arrives while another is blocked
// queues behind the last one, and the unblock of an earlier entry clears
// `_blockedConsole` only while it is still that entry's chunk
// (`if (response._blockedConsole === chunk)`). No upstream test checks it.
describe('ReactFlight console replay order', () => {
  let ReactFlightClient;

  beforeEach(() => {
    jest.resetModules();
    ReactFlightClient = require('react-client/flight');
  });

  afterEach(() => {
    jest.restoreAllMocks();
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
      true, // replayConsole
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

  // @gate __DEV__
  it('replays an entry that arrives while an earlier one is still blocked after it', async () => {
    const replayed = [];
    jest.spyOn(console, 'log').mockImplementation((...args) => {
      replayed.push(args.join(' '));
    });
    const client = createClient();
    // The first entry waits on row a, which isn't here yet.
    client.send(':W["log",[],null,"Server","first","$a"]');
    // The second queues behind the first, and waits on row b too.
    client.send(':W["log",[],null,"Server","second","$b"]');
    // The third queues behind the second.
    client.send(':W["log",[],null,"Server","third"]');
    // Row a unblocks the first; the second still waits on row b, so the
    // third is still the last blocked entry.
    client.send('a:"A"');
    await settle();
    expect(replayed).toEqual(['first A']);
    // The fourth queues behind the third.
    client.send(':W["log",[],null,"Server","fourth"]');
    await settle();
    expect(replayed).toEqual(['first A']);
    client.send('b:"B"');
    client.send('0:"done"');
    expect(await client.root).toBe('done');
    await settle();
    expect(replayed).toEqual(['first A', 'second B', 'third', 'fourth']);
  });
});
