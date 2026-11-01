// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

// Ids are base 32 strings whose binary representation corresponds to the
// position of a node in a tree.

// Every time the tree forks into multiple children, we add additional bits to
// the left of the sequence that represent the position of the child within the
// current level of children.
//
//      00101       00010001011010101
//      ╰─┬─╯       ╰───────┬───────╯
//   Fork 5 of 20       Parent id
//
// The leading 0s are important. In the above example, you only need 3 bits to
// represent slot 5. However, you need 5 bits to represent all the forks at
// the current level, so we must account for the empty bits at the end.
//
// For this same reason, slots are 1-indexed instead of 0-indexed. Otherwise,
// the zeroth id at a level would be indistinguishable from its parent.
//
// If a node has only one child, and does not materialize an id (i.e. does not
// contain a useId hook), then we don't need to allocate any space in the
// sequence. It's treated as a transparent indirection. For example, these two
// trees produce the same ids:
//
// <>                          <>
//   <Indirection>               <A />
//     <A />                     <B />
//   </Indirection>            </>
//   <B />
// </>
//
// However, we cannot skip any node that materializes an id. Otherwise, a parent
// id that does not fork would be indistinguishable from its child id. For
// example, this tree does not fork, but the parent and child must have
// different ids.
//
// <Parent>
//   <Child />
// </Parent>
//
// To handle this scenario, every time we materialize an id, we allocate a
// new level with a single slot. You can think of this as a fork with only one
// prong, or an array of children with length 1.
//
// It's possible for the size of the sequence to exceed 32 bits, the max
// size for bitwise operations. When this happens, we make more room by
// converting the right part of the id to a string and storing it in an overflow
// variable. We use a base 32 string representation, because 32 is the largest
// power of 2 that is supported by toString(). We want the base to be large so
// that the resulting ids are compact, and we want the base to be a power of 2
// because every log2(base) bits corresponds to a single character, i.e. every
// log2(32) = 5 bits. That means we can lop bits off the end 5 at a time without
// affecting the final result.

pub struct TreeContext {
    pub id: i32,
    pub overflow: String,
}

thread_local! {
    pub static EMPTY_TREE_CONTEXT: Rc<TreeContext> = Rc::new(TreeContext {
        id: 1,
        overflow: String::new(),
    });
}

pub fn get_tree_id(context: &TreeContext) -> String {
    let overflow = &context.overflow;
    let id_with_leading_bit = context.id;
    let id = id_with_leading_bit & !get_leading_bit(id_with_leading_bit);
    js::number::to_string(id.into(), Some(32)) + overflow
}

pub fn push_tree_context(
    base_context: &TreeContext,
    total_children: i32,
    index: i32,
) -> Rc<TreeContext> {
    let base_id_with_leading_bit = base_context.id;
    let base_overflow = &base_context.overflow;

    // The leftmost 1 marks the end of the sequence, non-inclusive. It's not part
    // of the id; we use it to account for leading 0s.
    let base_length = get_bit_length(base_id_with_leading_bit) - 1;
    let base_id = base_id_with_leading_bit & !(1 << base_length);

    let slot = index + 1;
    let length = get_bit_length(total_children) + base_length;

    // 30 is the max length we can store without overflowing, taking into
    // consideration the leading 1 we use to mark the end of the sequence.
    if length > 30 {
        // We overflowed the bitwise-safe range. Fall back to slower algorithm.
        // This branch assumes the length of the base id is greater than 5; it won't
        // work for smaller ids, because you need 5 bits per character.
        //
        // We encode the id in multiple steps: first the base id, then the
        // remaining digits.
        //
        // Each 5 bit sequence corresponds to a single base 32 character. So for
        // example, if the current id is 23 bits long, we can convert 20 of those
        // bits into a string of 4 characters, with 3 bits left over.
        //
        // First calculate how many bits in the base id represent a complete
        // sequence of characters.
        let number_of_overflow_bits = base_length - (base_length % 5);

        // Then create a bitmask that selects only those bits.
        let new_overflow_bits = (1 << number_of_overflow_bits) - 1;

        // Select the bits, and convert them to a base 32 string. Every 5 bit
        // group must produce exactly one character, so restore any leading
        // zero characters that toString drops.
        let new_overflow = format!(
            "{:0>width$}",
            js::number::to_string((base_id & new_overflow_bits).into(), Some(32)),
            width = (number_of_overflow_bits / 5) as usize,
        );

        // Now we can remove those bits from the base id.
        let rest_of_base_id = base_id >> number_of_overflow_bits;
        let rest_of_base_length = base_length - number_of_overflow_bits;

        // Finally, encode the rest of the bits using the normal algorithm. Because
        // we made more room, this time it won't overflow.
        let rest_of_length = get_bit_length(total_children) + rest_of_base_length;
        let rest_of_new_bits = slot << rest_of_base_length;
        let id = rest_of_new_bits | rest_of_base_id;
        let overflow = new_overflow + base_overflow;
        Rc::new(TreeContext {
            id: (1 << rest_of_length) | id,
            overflow,
        })
    } else {
        // Normal path
        let new_bits = slot << base_length;
        let id = new_bits | base_id;
        let overflow = base_overflow.clone();
        Rc::new(TreeContext {
            id: (1 << length) | id,
            overflow,
        })
    }
}

fn get_bit_length(number: i32) -> i32 {
    32 - CLZ32.with(|clz32| clz32(number as u32)) as i32
}

fn get_leading_bit(id: i32) -> i32 {
    1 << (get_bit_length(id) - 1)
}

thread_local! {
    // TODO: Math.clz32 is supported in Node 12+. Maybe we can drop the fallback.
    static CLZ32: fn(u32) -> u32 = if js::is_defined(u32::leading_zeros) {
        u32::leading_zeros
    } else {
        clz32_fallback
    };
}

// Count leading zeros.
// Based on:
// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Math/clz32
#[cfg_attr(rust_js, rust_js::name = "log")]
const LOG: fn(f64) -> f64 = f64::ln;
const LN2: f64 = std::f64::consts::LN_2;
fn clz32_fallback(x: u32) -> u32 {
    let as_uint = x;
    if as_uint == 0 {
        return 32;
    }
    31 - (LOG(as_uint as f64) / LN2) as u32
}
