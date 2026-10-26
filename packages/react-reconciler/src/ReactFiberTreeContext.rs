// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

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

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use shared::environment::__DEV__;
use webapi::console;

use crate::clz32::CLZ32;
use crate::react_fiber_flags::{FORKED, NO_FLAGS};
use crate::react_fiber_hydration_context::get_is_hydrating;
use crate::react_internal_types::Fiber;

pub struct TreeContext {
    pub id: i32,
    pub overflow: String,
}

// A pushed id: its tree context's id, overflow and provider.
type IdStackEntry = (i32, String, Option<Rc<Fiber>>);

thread_local! {
    // TODO: Use the unified fiber stack module instead of this local one?
    // Intentionally not using it yet to derisk the initial implementation, because
    // the way we push/pop these values is a bit unusual. If there's a mistake, I'd
    // rather the ids be wrong than crash the whole reconciler.
    static FORK_STACK: RefCell<Vec<(i32, Option<Rc<Fiber>>)>> = const { RefCell::new(Vec::new()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static TREE_FORK_PROVIDER: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    static TREE_FORK_COUNT: Cell<i32> = const { Cell::new(0) };

    static ID_STACK: RefCell<Vec<IdStackEntry>> = const { RefCell::new(Vec::new()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static TREE_CONTEXT_PROVIDER: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    static TREE_CONTEXT_ID: Cell<i32> = const { Cell::new(1) };
    static TREE_CONTEXT_OVERFLOW: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn is_forked_child(work_in_progress: &Fiber) -> bool {
    warn_if_not_hydrating();
    (work_in_progress.flags.get() & FORKED) != NO_FLAGS
}

pub fn get_forks_at_level(_work_in_progress: &Fiber) -> i32 {
    warn_if_not_hydrating();
    TREE_FORK_COUNT.get()
}

pub fn get_tree_id() -> String {
    let overflow = TREE_CONTEXT_OVERFLOW.with_borrow(|overflow| overflow.clone());
    let id_with_leading_bit = TREE_CONTEXT_ID.get();
    let id = id_with_leading_bit & !get_leading_bit(id_with_leading_bit);
    js::number::to_string(id.into(), Some(32)) + &overflow
}

pub fn push_tree_fork(work_in_progress: &Rc<Fiber>, total_children: i32) {
    // This is called right after we reconcile an array (or iterator) of child
    // fibers, because that's the only place where we know how many children in
    // the whole set without doing extra work later, or storing addtional
    // information on the fiber.
    //
    // That's why this function is separate from pushTreeId — it's called during
    // the render phase of the fork parent, not the child, which is where we push
    // the other context values.
    //
    // In the Fizz implementation this is much simpler because the child is
    // rendered in the same callstack as the parent.
    //
    // It might be better to just add a `forks` field to the Fiber type. It would
    // make this module simpler.

    warn_if_not_hydrating();

    FORK_STACK.with_borrow_mut(|fork_stack| {
        fork_stack.push((
            TREE_FORK_COUNT.get(),
            TREE_FORK_PROVIDER.with_borrow(|provider| provider.clone()),
        ));
    });

    TREE_FORK_PROVIDER.set(Some(Rc::clone(work_in_progress)));
    TREE_FORK_COUNT.set(total_children);
}

pub fn push_tree_id(work_in_progress: &Rc<Fiber>, total_children: i32, index: i32) {
    warn_if_not_hydrating();

    ID_STACK.with_borrow_mut(|id_stack| {
        id_stack.push((
            TREE_CONTEXT_ID.get(),
            TREE_CONTEXT_OVERFLOW.with_borrow(|overflow| overflow.clone()),
            TREE_CONTEXT_PROVIDER.with_borrow(|provider| provider.clone()),
        ));
    });

    TREE_CONTEXT_PROVIDER.set(Some(Rc::clone(work_in_progress)));

    let base_id_with_leading_bit = TREE_CONTEXT_ID.get();
    let base_overflow = TREE_CONTEXT_OVERFLOW.with_borrow(|overflow| overflow.clone());

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
        let overflow = new_overflow + &base_overflow;

        TREE_CONTEXT_ID.set((1 << rest_of_length) | id);
        TREE_CONTEXT_OVERFLOW.set(overflow);
    } else {
        // Normal path
        let new_bits = slot << base_length;
        let id = new_bits | base_id;
        let overflow = base_overflow;

        TREE_CONTEXT_ID.set((1 << length) | id);
        TREE_CONTEXT_OVERFLOW.set(overflow);
    }
}

pub fn push_materialized_tree_id(work_in_progress: &Rc<Fiber>) {
    warn_if_not_hydrating();

    // This component materialized an id. This will affect any ids that appear
    // in its children.
    let return_fiber = work_in_progress.r#return.borrow().clone();
    if return_fiber.is_some() {
        let number_of_forks = 1;
        let slot_index = 0;
        push_tree_fork(work_in_progress, number_of_forks);
        push_tree_id(work_in_progress, number_of_forks, slot_index);
    }
}

fn get_bit_length(number: i32) -> i32 {
    32 - CLZ32.with(|clz32| clz32(number as u32)) as i32
}

fn get_leading_bit(id: i32) -> i32 {
    1 << (get_bit_length(id) - 1)
}

pub fn pop_tree_context(work_in_progress: &Fiber) {
    // Restore the previous values.

    // This is a bit more complicated than other context-like modules in Fiber
    // because the same Fiber may appear on the stack multiple times and for
    // different reasons. We have to keep popping until the work-in-progress is
    // no longer at the top of the stack.

    while TREE_FORK_PROVIDER.with_borrow(|provider| Some(work_in_progress) == provider.as_deref()) {
        let (fork_count, fork_provider) = FORK_STACK
            .with_borrow_mut(|fork_stack| fork_stack.pop())
            .expect("a fork pushed by the work-in-progress");
        TREE_FORK_PROVIDER.set(fork_provider);
        TREE_FORK_COUNT.set(fork_count);
    }

    while TREE_CONTEXT_PROVIDER
        .with_borrow(|provider| Some(work_in_progress) == provider.as_deref())
    {
        let (id, overflow, provider) = ID_STACK
            .with_borrow_mut(|id_stack| id_stack.pop())
            .expect("an id pushed by the work-in-progress");
        TREE_CONTEXT_PROVIDER.set(provider);
        TREE_CONTEXT_OVERFLOW.set(overflow);
        TREE_CONTEXT_ID.set(id);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_suspended_tree_context() -> Option<TreeContext> {
    warn_if_not_hydrating();
    if TREE_CONTEXT_PROVIDER.with_borrow(|provider| provider.is_some()) {
        Some(TreeContext {
            id: TREE_CONTEXT_ID.get(),
            overflow: TREE_CONTEXT_OVERFLOW.with_borrow(|overflow| overflow.clone()),
        })
    } else {
        None
    }
}

pub fn restore_suspended_tree_context(
    work_in_progress: &Rc<Fiber>,
    suspended_context: &TreeContext,
) {
    warn_if_not_hydrating();

    ID_STACK.with_borrow_mut(|id_stack| {
        id_stack.push((
            TREE_CONTEXT_ID.get(),
            TREE_CONTEXT_OVERFLOW.with_borrow(|overflow| overflow.clone()),
            TREE_CONTEXT_PROVIDER.with_borrow(|provider| provider.clone()),
        ));
    });

    TREE_CONTEXT_ID.set(suspended_context.id);
    TREE_CONTEXT_OVERFLOW.set(suspended_context.overflow.clone());
    TREE_CONTEXT_PROVIDER.set(Some(Rc::clone(work_in_progress)));
}

fn warn_if_not_hydrating() {
    if __DEV__ && !get_is_hydrating() {
        console::error((concat!(
            "Expected to be hydrating. This is a bug in React. Please file ",
            "an issue."
        ),));
    }
}
