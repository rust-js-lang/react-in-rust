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

// The port's own test: no upstream test runs ReactServerStreamConfigBun, the
// stream config of react-dom/server.bun's renderToReadableStream. Its
// destination is a Bun direct stream's controller or a Node Writable, each
// faked here, and `Bun.hash` a global this test defines.
let StreamConfig;

describe('ReactServerStreamConfigBun', () => {
  beforeEach(() => {
    jest.resetModules();
    jest.useRealTimers();
    global.Bun = {hash: jest.fn(input => BigInt(input.length) * 31n)};
    StreamConfig = require('react-server/src/ReactServerStreamConfigBun');
  });

  afterEach(() => {
    delete global.Bun;
  });

  function controller() {
    const calls = [];
    return {
      calls,
      write(chunk) {
        calls.push(['write', chunk]);
        return typeof chunk === 'string' ? chunk.length : chunk.byteLength;
      },
      end() {
        calls.push(['end']);
      },
      error(error) {
        calls.push(['error', error]);
      },
    };
  }

  it('schedules work on a timer and microtasks on queueMicrotask', async () => {
    const setTimeoutSpy = jest.spyOn(global, 'setTimeout');
    const callback = jest.fn();
    StreamConfig.scheduleWork(callback);
    expect(setTimeoutSpy).toHaveBeenCalledWith(callback, 0);
    setTimeoutSpy.mockRestore();
    expect(StreamConfig.scheduleMicrotask).toBe(queueMicrotask);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('flushes only a destination that has a flush function', () => {
    const destination = controller();
    StreamConfig.flushBuffered(destination);
    destination.flush = jest.fn();
    StreamConfig.flushBuffered(destination);
    expect(destination.flush).toHaveBeenCalledTimes(1);
    destination.flush = true;
    StreamConfig.flushBuffered(destination);
  });

  it('writes each chunk but an empty one, as it is', () => {
    const destination = controller();
    const bytes = new Uint8Array([104, 105]);
    const view = new DataView(new ArrayBuffer(0));
    StreamConfig.beginWriting(destination);
    StreamConfig.writeChunk(destination, '');
    StreamConfig.writeChunk(destination, new Uint8Array(0));
    StreamConfig.writeChunk(destination, 'hi');
    StreamConfig.writeChunk(destination, bytes);
    // A DataView has no length, so even an empty one is written.
    StreamConfig.writeChunk(destination, view);
    StreamConfig.completeWriting(destination);
    expect(destination.calls).toEqual([
      ['write', 'hi'],
      ['write', bytes],
      ['write', view],
    ]);
    expect(destination.calls[1][1]).toBe(bytes);
    expect(destination.calls[2][1]).toBe(view);
  });

  it('tells whether a write was taken by its result', () => {
    const results = [0, 5, Promise.resolve(3), undefined, false, true, ''];
    const destination = {write: () => results.shift()};
    const taken = [];
    for (let i = 0; i < 7; i++) {
      taken.push(StreamConfig.writeChunkAndReturn(destination, 'x'));
    }
    expect(taken).toEqual([false, true, true, false, false, true, false]);
    // Even an empty chunk is written.
    const written = [];
    StreamConfig.writeChunkAndReturn({write: c => written.push(c)}, '');
    expect(written).toEqual(['']);
  });

  it('ends the destination as it closes', () => {
    const destination = controller();
    StreamConfig.close(destination);
    expect(destination.calls).toEqual([['end']]);
  });

  it('keeps text and views as they are', () => {
    expect(StreamConfig.stringToChunk('<p>')).toBe('<p>');
    expect(StreamConfig.stringToPrecomputedChunk('</p>')).toBe('</p>');
    const view = new Uint16Array([1, 2, 3]);
    expect(StreamConfig.typedArrayToBinaryChunk(view)).toBe(view);
    expect(StreamConfig.byteLengthOfBinaryChunk(view)).toBe(6);
  });

  it('measures text in UTF-8 bytes', () => {
    expect(StreamConfig.byteLengthOfChunk('abc')).toBe(3);
    expect(StreamConfig.byteLengthOfChunk('é')).toBe(2);
    expect(StreamConfig.byteLengthOfChunk('😀')).toBe(4);
  });

  it('closes with an error by error, else destroy, else close', () => {
    const error = new Error('boom');

    const direct = controller();
    direct.destroy = jest.fn();
    StreamConfig.closeWithError(direct, error);
    expect(direct.calls).toEqual([['error', error]]);
    expect(direct.destroy).not.toHaveBeenCalled();

    const writable = {destroy: jest.fn(), close: jest.fn()};
    StreamConfig.closeWithError(writable, error);
    expect(writable.destroy).toHaveBeenCalledWith(error);
    expect(writable.close).not.toHaveBeenCalled();

    const older = {close: jest.fn(), error: 'not a function'};
    StreamConfig.closeWithError(older, error);
    expect(older.close).toHaveBeenCalledWith();

    // Nothing to call: nothing happens.
    StreamConfig.closeWithError({}, error);

    const undefinedError = controller();
    StreamConfig.closeWithError(undefinedError, undefined);
    expect(undefinedError.calls).toEqual([['error', undefined]]);
  });

  it('hashes with Bun.hash, its bigint as it is', () => {
    expect(StreamConfig.createFastHash('abcd')).toBe(124n);
    expect(global.Bun.hash).toHaveBeenCalledWith('abcd');
    expect(global.Bun.hash.mock.calls[0].length).toBe(1);
  });

  it('reads a blob as a data URL, of its type or octet-stream', async () => {
    const typed = new Blob(['hi'], {type: 'text/plain'});
    expect(await StreamConfig.readAsDataURL(typed)).toBe(
      'data:text/plain;base64,aGk=',
    );
    const untyped = new Blob([new Uint8Array([1, 2, 3])]);
    expect(await StreamConfig.readAsDataURL(untyped)).toBe(
      'data:application/octet-stream;base64,AQID',
    );
  });
});
