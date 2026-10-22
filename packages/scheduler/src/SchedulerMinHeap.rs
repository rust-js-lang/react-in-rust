// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

type Heap<T> = Vec<T>;

/// What the heap orders: anything with a `sortIndex` and an `id`.
pub trait Node {
    #[cfg_attr(rust_js, rust_js::link_name = "get id")]
    fn id(&self) -> f64 {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get sortIndex")]
    fn sort_index(&self) -> f64 {
        unreachable!()
    }
}

pub fn push<T: Node>(heap: &mut Heap<T>, node: T) {
    let index = heap.len();
    heap.push(node);
    sift_up(heap, index);
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn peek<T: Node>(heap: &Heap<T>) -> Option<&T> {
    heap.first()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn pop<T: Node>(heap: &mut Heap<T>) -> Option<T> {
    let last = heap.pop()?;
    if heap.is_empty() {
        return Some(last);
    }
    let first = std::mem::replace(&mut heap[0], last);
    sift_down(heap, 0);
    Some(first)
}

fn sift_up<T: Node>(heap: &mut Heap<T>, i: usize) {
    let mut index = i;
    while index > 0 {
        let parent_index = (index - 1) >> 1;
        if compare(&heap[parent_index], &heap[index]) > 0.0 {
            // The parent is larger. Swap positions.
            heap.swap(parent_index, index);
            index = parent_index;
        } else {
            // The parent is smaller. Exit.
            return;
        }
    }
}

fn sift_down<T: Node>(heap: &mut Heap<T>, i: usize) {
    let mut index = i;
    let length = heap.len();
    let half_length = length >> 1;
    while index < half_length {
        let left_index = (index + 1) * 2 - 1;
        let right_index = left_index + 1;

        // If the left or right node is smaller, swap with the smaller of those.
        if compare(&heap[left_index], &heap[index]) < 0.0 {
            if right_index < length && compare(&heap[right_index], &heap[left_index]) < 0.0 {
                heap.swap(index, right_index);
                index = right_index;
            } else {
                heap.swap(index, left_index);
                index = left_index;
            }
        } else if right_index < length && compare(&heap[right_index], &heap[index]) < 0.0 {
            heap.swap(index, right_index);
            index = right_index;
        } else {
            // Neither child is smaller. Exit.
            return;
        }
    }
}

fn compare<T: Node>(a: &T, b: &T) -> f64 {
    // Compare sort index first, then task id.
    let diff = a.sort_index() - b.sort_index();
    if diff != 0.0 { diff } else { a.id() - b.id() }
}
