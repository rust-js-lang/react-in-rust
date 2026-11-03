//! Bindings of [neo-async](https://github.com/suguru03/neo-async) 2.6, the
//! `async` library webpack's ecosystem uses: what runs a function of each
//! item of a collection, each given a callback it calls once it's done,
//! then calls a callback of all their results.
//!
//! ```rust,ignore
//! neo_async::map(&paths, |path, done| done(None, Some(path.len())), |err, lengths| { .. });
//! ```

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

/// [`async.map(collection, iterator, callback)`](https://suguru03.github.io/neo-async/doc/async.map.html):
/// `iterator` run of each item at once, each given the callback it calls
/// with what failed or its result; then `callback`, of the first that
/// failed, or of every result, in the items' order. What it gives with an
/// error holds only the results done before.
#[cfg_attr(rust_js, rust_js::link_name = "neo-async#default.map")]
pub fn map<T: 'static, R: 'static, E: 'static>(
    collection: &[T],
    iterator: impl Fn(&'static T, &'static dyn Fn(Option<&'static E>, Option<R>)) + 'static,
    callback: impl FnOnce(Option<&'static E>, Vec<R>) + 'static,
) {
    unreachable!()
}

/// [`async.filter(collection, iterator, callback)`](https://suguru03.github.io/neo-async/doc/async.filter.html):
/// `iterator` run of each item at once, each given the callback it calls
/// with what failed or whether the item stays; then `callback`, of the
/// first that failed, and no items, or of the items that stay, in their
/// order.
#[cfg_attr(rust_js, rust_js::link_name = "neo-async#default.filter")]
pub fn filter<T: 'static, E: 'static>(
    collection: &[T],
    iterator: impl Fn(&'static T, &'static dyn Fn(Option<&'static E>, bool)) + 'static,
    callback: impl FnOnce(Option<&'static E>, Option<Vec<T>>) + 'static,
) {
    unreachable!()
}
