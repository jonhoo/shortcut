//! This create provides an indexed, queryable column-based storage system.
//!
//! The storage system is, fundamentally, row-based storage, where all rows have the same number of
//! columns. All columns are the same "type", but given that they can be enum types, you can
//! effectively use differently typed values. Data is stored in a `BTreeMap<usize, Vec<T>>`,
//! where the outermost `BTreeMap` is dynamically sized (and may be re-allocated as more rows come
//! in), whereas the innermost `Vec` is expected to never change. The map index is an
//! autoincremented row identifier similar to the one used by SQLite:
//! https://www.sqlite.org/lang_createtable.html#rowid.
//!
//! What makes this crate interesting is that it also allows you to place indices on columns for
//! fast lookups. These indices are automatically updates whenever the dataset changes, so that
//! queries continue to return correct results. Indices should conform to either the
//! `EqualityIndex` trait or the `RangeIndex` trait. As you would expect, the former allows
//! speeding up exact lookups, whereas the latter can also perform efficient range queries.
//!
//! Queries are performed over the dataset by calling `find` with a set of `Condition`s that will
//! be `AND`ed together. `OR` is currently not supported --- issue multiple quieries instead. Each
//! `Condition` represents a value comparison against the value in a single column. The system
//! automatically picks what index to use to satisfy the query, using a heuristic based on the
//! expected number of rows returned for that column for each index.
//!
//! # Known limitations
//!
//!  - The set of match operations is currently fairly limited.
//!  - The system currently provides an add/remove-only abstraction (i.e., no edit).

#![deny(missing_docs)]
#![feature(btree_range, collections_bound)]

use std::collections::HashMap;
use std::collections::BTreeMap;

/// The `cmp` module holds the mechanisms needed to compare values and express conditionals.
pub mod cmp;
pub use cmp::Comparison;
pub use cmp::Condition;
pub use cmp::Value;

/// The `idx` module described the traits indexers must adhere to, and implements sensible default
/// indexers.
pub mod idx;
pub use idx::EqualityIndex;
pub use idx::RangeIndex;
pub use idx::Index;

/// A `Store` is the main storage unit in shortcut. It keeps track of all the rows of data, as well
/// as what indices are available. You will generally be accessing the `Store` either through the
/// `find` method (which lets you find rows that match a certain condition), or through the
/// `insert` method, which lets you add another row.
///
/// Note that the type used for the rows needs to be `Clone`. This is because the value is also
/// given to the index, which (currently) take a full value, not just a borrow. This *might* change
/// down the line, but it's tricky to get the lifetimes to work out, because the indices would then
/// be scoped by the lifetime of the `Store`.
pub struct Store<T: PartialOrd + Clone> {
    cols: usize,
    rowid: usize,
    rows: BTreeMap<usize, Vec<T>>,
    indices: HashMap<usize, Index<T>>,
}

impl<T: PartialOrd + Clone> Store<T> {
    /// Allocate a new `Store` with the given number of columns. The column count is checked in
    /// `insert` at runtime (bleh).
    pub fn new(cols: usize) -> Store<T> {
        Store {
            cols: cols,
            rowid: 0,
            rows: BTreeMap::new(),
            indices: HashMap::new(),
        }
    }

    /// Decide what index to use in order to match the given conditions most efficiently. Note that
    /// the iterator returned by this method will return a superset of the rows that match the
    /// given conditions. Users will need to match each individual row against `conds` again.
    fn using_index<'a>(&'a self,
                       conds: &'a [cmp::Condition<T>])
                       -> Box<Iterator<Item = usize> + 'a> {

        use EqualityIndex;
        let best_idx = conds.iter()
            .enumerate()
            .filter_map(|(ci, c)| self.indices.get(&c.column).and_then(|idx| Some((ci, idx))))
            .filter(|&(ci, _)| {
                // does this index work for the operation in question?
                match conds[ci].cmp {
                    cmp::Comparison::Equal(cmp::Value::Const(..)) => true,
                    _ => false,
                }
            })
            .min_by_key(|&(_, idx)| idx.estimate());

        best_idx.and_then(|(ci, idx)| match conds[ci].cmp {
                cmp::Comparison::Equal(cmp::Value::Const(ref v)) => Some(idx.lookup(v)),
                _ => unreachable!(),
            })
            .unwrap_or_else(|| Box::new(self.rows.keys().map(|k| *k)))
    }

    /// Returns an iterator that yields all rows matching all the given `Condition`s.
    ///
    /// This method will automatically determine what index to use to satisfy this query. It
    /// currently uses a fairly simple heuristic: it picks the index that: a) is over one of
    /// columns being filtered on; b) supports the operation for that filter; and c) has the lowest
    /// expected number of rows for a single value. This latter metric is generally the total
    /// number of rows divided by the number of entries in the index. See `EqualityIndex::estimate`
    /// for details.
    pub fn find<'a>(&'a self,
                    conds: &'a [cmp::Condition<T>])
                    -> Box<Iterator<Item = &'a [T]> + 'a> {
        Box::new(self.using_index(conds)
            .map(move |rowi| &self.rows[&rowi][..])
            .filter(move |row| conds.iter().all(|c| c.matches(row))))
    }

    /// Delete all rows that match the given conditions.
    pub fn delete(&mut self, conds: &[cmp::Condition<T>]) {
        self.delete_filter(conds, |_| true);
    }

    /// Delete all rows that match the given conditions *and* where the given filter function
    /// returns true.
    pub fn delete_filter<F: FnMut(&[T]) -> bool>(&mut self,
                                                 conds: &[cmp::Condition<T>],
                                                 mut f: F) {
        // find the rows we should delete
        let rowids = self.using_index(conds)
            .map(|rowi| (rowi, &self.rows[&rowi][..]))
            .filter(move |&(_, row)| conds.iter().all(|c| c.matches(row)))
            .filter(|&(_, row)| f(row))
            .map(|(rowid, _)| rowid)
            .collect::<Vec<_>>();

        let deleted = rowids.into_iter()
            .map(|rowid| (rowid, self.rows.remove(&rowid).unwrap()))
            .collect::<Vec<_>>();

        // we want to avoid having to clone out of row to pass a T to undex(), which we'd have to
        // do if it were a Vec (or we'd have to do some trickery to not mess up the indices after
        // calling .remove()). Instead, we allocate a single HashMap from column index -> T, which
        // we populate for each row.
        let mut rowcols = HashMap::with_capacity(self.cols);
        for (rowid, row) in deleted.into_iter() {
            rowcols.extend(row.into_iter().enumerate());
            for (col, idx) in self.indices.iter_mut() {
                idx.undex(rowcols.remove(col).unwrap(), rowid);
            }
        }
    }

    /// Insert a new data row into the `Store`. The row **must** have the same number of columns as
    /// specified when the `Store` was created. If it does not, the code will panic with an
    /// assertion failure.
    ///
    /// Inserting a row has similar complexity to `BTreeMap::insert`, and *may* need to re-allocate
    /// the backing memory for the `Store`. The insertion also updates all maintained indices,
    /// which may also re-allocate.
    pub fn insert(&mut self, row: Vec<T>) {
        assert_eq!(row.len(), self.cols);
        let rowid = self.rowid;
        for (column, idx) in self.indices.iter_mut() {
            use EqualityIndex;
            idx.index(row[*column].clone(), rowid);
        }
        self.rows.insert(self.rowid, row);
        self.rowid += 1;
    }

    /// Add an index on the given colum using the given indexer. The indexer *must*, at the very
    /// least, implement `EqualityIndex`. It *may* also implement other, more sophisticated,
    /// indexing strategies outlined in `Index`.
    ///
    /// When an index is added, it is immediately fed all rows in the current dataset. Thus, adding
    /// an index to a `Store` with many rows can be fairly costly. Keep this in mind!
    pub fn index<I: Into<Index<T>>>(&mut self, column: usize, indexer: I) {
        use EqualityIndex;
        let mut idx = indexer.into();

        // populate the new index
        for (rowid, row) in self.rows.iter() {
            idx.index(row[column].clone(), *rowid);
        }

        self.indices.insert(column, idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut store = Store::new(2);
        store.insert(vec!["a1", "a2"]);
        store.insert(vec!["b1", "b2"]);
        store.insert(vec!["c1", "c2"]);
        assert_eq!(store.find(&[]).count(), 3);
    }

    #[test]
    fn it_works_with_indices() {
        let mut store = Store::new(2);
        store.index(0, idx::HashIndex::new());
        store.insert(vec!["a1", "a2"]);
        store.insert(vec!["b1", "b2"]);
        store.insert(vec!["c1", "c2"]);
        assert_eq!(store.find(&[]).count(), 3);
    }

    #[test]
    fn it_filters() {
        let mut store = Store::new(2);
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        let cmp = [cmp::Condition {
                       column: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        assert_eq!(store.find(&cmp)
                       .count(),
                   2);
        assert!(store.find(&cmp).all(|r| r[0] == "a"));
    }

    #[test]
    fn it_filters_with_indices() {
        let mut store = Store::new(2);
        store.index(0, idx::HashIndex::new());
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        let cmp = [cmp::Condition {
                       column: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        assert_eq!(store.find(&cmp)
                       .count(),
                   2);
        assert!(store.find(&cmp).all(|r| r[0] == "a"));
    }

    #[test]
    fn it_filters_with_partial_indices() {
        let mut store = Store::new(2);
        store.index(0, idx::HashIndex::new());
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        let cmp = [cmp::Condition {
                       column: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   },
                   cmp::Condition {
                       column: 1,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("x2")),
                   }];
        assert_eq!(store.find(&cmp).count(), 1);
        assert!(store.find(&cmp).all(|r| r[0] == "a" && r[1] == "x2"));
    }

    #[test]
    fn it_filters_with_late_indices() {
        let mut store = Store::new(2);
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        store.index(0, idx::HashIndex::new());
        let cmp = [cmp::Condition {
                       column: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        assert_eq!(store.find(&cmp)
                       .count(),
                   2);
        assert!(store.find(&cmp).all(|r| r[0] == "a"));
    }

    #[test]
    fn is_send_sync() {
        use std::sync;
        use std::thread;
        let store = sync::Arc::new(Store::<()>::new(0));
        thread::spawn(move || {
                drop(store);
            })
            .join()
            .unwrap();
    }

    #[test]
    fn it_deletes() {
        let mut store = Store::new(2);
        store.insert(vec!["a1", "a2"]);
        store.insert(vec!["b1", "b2"]);
        store.insert(vec!["c1", "c2"]);
        store.delete(&[]);
        assert_eq!(store.find(&[]).count(), 0);
    }

    #[test]
    fn filtered_delete() {
        let mut store = Store::new(2);
        store.insert(vec!["a1", "a2"]);
        store.insert(vec!["b1", "b2"]);
        store.insert(vec!["c1", "c2"]);
        store.delete_filter(&[], |row| row[0] != "b1");
        assert_eq!(store.find(&[]).count(), 1);
        assert!(store.find(&[]).all(|r| r[0] == "b1"));
    }

    #[test]
    fn it_deletes_with_filters() {
        let mut store = Store::new(2);
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        let cmp = [cmp::Condition {
                       column: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        store.delete(&cmp);
        assert_eq!(store.find(&cmp).count(), 0);
        assert_eq!(store.find(&[]).count(), 1);
        assert!(store.find(&[]).all(|r| r[0] == "b"));
    }

    #[test]
    fn it_deletes_with_indices() {
        let mut store = Store::new(2);
        store.index(0, idx::HashIndex::new());
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        let cmp = [cmp::Condition {
                       column: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        store.delete(&cmp);
        assert_eq!(store.find(&cmp).count(), 0);
        assert_eq!(store.find(&[]).count(), 1);
        assert!(store.find(&[]).all(|r| r[0] == "b"));
    }

    #[test]
    fn it_deletes_with_partial_indices() {
        let mut store = Store::new(2);
        store.index(0, idx::HashIndex::new());
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        let cmp = [cmp::Condition {
                       column: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   },
                   cmp::Condition {
                       column: 1,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("x2")),
                   }];
        store.delete(&cmp);
        assert_eq!(store.find(&cmp).count(), 0);
        assert_eq!(store.find(&[]).count(), 2);
        assert!(store.find(&[]).any(|r| r[0] == "a" && r[1] == "x1"));
        assert!(store.find(&[]).any(|r| r[0] == "b" && r[1] == "x3"));
    }
}
