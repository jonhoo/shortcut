#![feature(btree_range, collections_bound)]

use std::collections::HashMap;

mod cmp;
mod idx;

pub struct Store<T: PartialOrd> {
    cols: usize,
    rows: Vec<Vec<T>>,
    indices: HashMap<usize, idx::Index<T>>,
}

struct StoreIterator<'a, T: 'a, I: Iterator<Item = usize>> {
    rows: &'a [Vec<T>],
    row_is: I,
}

impl<'a, T, I: Iterator<Item = usize>> Iterator for StoreIterator<'a, T, I> {
    type Item = &'a [T];
    fn next(&mut self) -> Option<Self::Item> {
        self.row_is.next().and_then(|i| Some(&self.rows[i][..]))
    }
}

impl<T: PartialOrd + Clone> Store<T> {
    pub fn new(cols: usize) -> Store<T> {
        Store {
            cols: cols,
            rows: Vec::new(),
            indices: HashMap::new(),
        }
    }

    pub fn with_capacity(cols: usize, rows: usize) -> Store<T> {
        Store {
            cols: cols,
            rows: Vec::with_capacity(rows),
            indices: HashMap::new(),
        }
    }

    pub fn find<'a>(&'a self,
                    conds: &'a [cmp::Condition<T>])
                    -> Box<Iterator<Item = &'a [T]> + 'a> {

        use idx::EqualityIndex;
        let best_idx = conds.iter()
            .enumerate()
            .filter_map(|(ci, c)| self.indices.get(&c.field).and_then(|idx| Some((ci, idx))))
            .filter(|&(ci, _)| {
                // does this index work for the operation in question?
                match conds[ci].cmp {
                    cmp::Comparison::Equal(cmp::Value::Const(..)) => true,
                    _ => false,
                }
            })
            .min_by_key(|&(_, idx)| idx.estimate());

        let iter = best_idx.and_then(|(ci, idx)| match conds[ci].cmp {
                cmp::Comparison::Equal(cmp::Value::Const(ref v)) => Some(idx.lookup(v)),
                _ => unreachable!(),
            })
            .unwrap_or_else(|| Box::new(0..self.rows.len()));

        Box::new(iter.map(move |rowi| &self.rows[rowi][..])
            .filter(move |row| conds.iter().all(|c| c.matches(row))))
    }

    pub fn insert(&mut self, row: Vec<T>) {
        assert_eq!(row.len(), self.cols);
        let rowi = self.rows.len();
        for (field, idx) in self.indices.iter_mut() {
            use idx::EqualityIndex;
            idx.index(row[*field].clone(), rowi);
        }
        self.rows.push(row);
    }

    pub fn index<I: Into<idx::Index<T>>>(&mut self, field: usize, indexer: I) {
        use idx::EqualityIndex;
        let mut idx = indexer.into();

        // populate the new index
        for (rowi, row) in self.rows.iter().enumerate() {
            idx.index(row[field].clone(), rowi);
        }

        self.indices.insert(field, idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmp;
    use idx;

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
                       field: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        assert_eq!(store.find(&cmp)
                       .count(),
                   2);
    }

    #[test]
    fn it_filters_with_indices() {
        let mut store = Store::new(2);
        store.index(0, idx::HashIndex::new());
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        let cmp = [cmp::Condition {
                       field: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        assert_eq!(store.find(&cmp)
                       .count(),
                   2);
    }

    #[test]
    fn it_filters_with_late_indices() {
        let mut store = Store::new(2);
        store.insert(vec!["a", "x1"]);
        store.insert(vec!["a", "x2"]);
        store.insert(vec!["b", "x3"]);
        store.index(0, idx::HashIndex::new());
        let cmp = [cmp::Condition {
                       field: 0,
                       cmp: cmp::Comparison::Equal(cmp::Value::Const("a")),
                   }];
        assert_eq!(store.find(&cmp)
                       .count(),
                   2);
    }
}
