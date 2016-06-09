#![feature(btree_range, collections_bound)]

use std::collections::HashMap;

mod cmp;
mod idx;

pub struct Store<T: PartialOrd> {
    cols: usize,
    rows: Vec<Vec<T>>,
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

impl<T: PartialOrd> Store<T> {
    pub fn new(cols: usize) -> Store<T> {
        Store {
            cols: cols,
            rows: Vec::new(),
        }
    }

    pub fn with_capacity(cols: usize, rows: usize) -> Store<T> {
        Store {
            cols: cols,
            rows: Vec::with_capacity(rows),
        }
    }

    pub fn find<'a>(&'a self,
                    conds: &'a [&[cmp::Condition<T>]])
                    -> Box<Iterator<Item = &'a [T]> + 'a> {
        Box::new(self.rows
            .iter()
            .filter(move |row| {
                conds.is_empty() || conds.iter().any(|cond| cond.iter().all(|c| c.matches(row)))
            })
            .map(|row| &row[..]))
    }

    pub fn insert(&mut self, row: Vec<T>) {
        assert_eq!(row.len(), self.cols);
        self.rows.push(row);
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
}
