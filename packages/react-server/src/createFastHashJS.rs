// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// A pure JS implementation of a string hashing function. We do not use it for
// security or obfuscation purposes, only to create compact hashes. So we
// prioritize speed over collision avoidance. For example, we use this to hash
// the component key path used by useActionState for MPA-style submissions.
//
// In environments where built-in hashing functions are available, we prefer
// those instead. Like Node's crypto module, or Bun.hash. Unfortunately this
// does not include the web standard crypto API because those methods are all
// async. For our purposes, we need it to be sync because the cost of context
// switching is too high to be worth it.
//
// The most popular hashing algorithm that meets these requirements in the JS
// ecosystem is MurmurHash3, and almost all implementations I could find used
// some version of the implementation by Gary Court inlined below.

#[cfg_attr(rust_js, rust_js::name = "createFastHashJS")]
pub fn create_fast_hash_js(key: &str) -> u32 {
    murmurhash3_32_gc(key, 0)
}

// @license
//
// JS Implementation of MurmurHash3 (r136) (as of May 20, 2011)
//
// Copyright (c) 2011 Gary Court
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//

#[cfg_attr(rust_js, rust_js::name = "murmurhash3_32_gc")]
fn murmurhash3_32_gc(key: &str, seed: u32) -> u32 {
    let key: Vec<u16> = key.encode_utf16().collect();
    let remainder = key.len() & 3; // key.length % 4
    let bytes = key.len() - remainder;
    let mut h1 = seed;
    let c1: u32 = 0xcc9e2d51;
    let c2: u32 = 0x1b873593;
    let mut i = 0;

    while i < bytes {
        let mut k1 = u32::from(key[i] & 0xff)
            | (u32::from(key[i + 1] & 0xff) << 8)
            | (u32::from(key[i + 2] & 0xff) << 16)
            | (u32::from(key[i + 3] & 0xff) << 24);
        i += 4;

        k1 = k1.wrapping_mul(c1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(c2);

        h1 ^= k1;
        h1 = h1.rotate_left(13);
        let h1b = h1.wrapping_mul(5);
        h1 = h1b.wrapping_add(0xe6546b64);
    }

    let mut k1 = 0;

    if remainder > 0 {
        if remainder == 3 {
            k1 ^= u32::from(key[i + 2] & 0xff) << 16;
        }
        if remainder >= 2 {
            k1 ^= u32::from(key[i + 1] & 0xff) << 8;
        }
        k1 ^= u32::from(key[i] & 0xff);

        k1 = k1.wrapping_mul(c1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(c2);
        h1 ^= k1;
    }

    h1 ^= key.len() as u32;

    h1 ^= h1 >> 16;
    h1 = h1.wrapping_mul(0x85ebca6b);
    h1 ^= h1 >> 13;
    h1 = h1.wrapping_mul(0xc2b2ae35);
    h1 ^= h1 >> 16;

    h1
}
