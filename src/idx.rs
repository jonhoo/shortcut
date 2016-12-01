use std::collections::HashMap;
use std::hash::Hash;

use std::collections::BTreeMap;
use std::collections::Bound;

/// An `EqualityIndex` is an index that can perform *efficient* equality lookups.
pub trait EqualityIndex<T> {
    /// Return an iterator that yields the indices of all rows that match the given value.
    fn lookup<'a>(&'a self, &T) -> Box<Iterator<Item = usize> + 'a>;

    /// Add the given row index to the index under the given value.
    fn index(&mut self, T, usize);

    /// Remove the given row index under the given value from the index.
    fn undex(&mut self, &T, usize);

    /// Give the expected number of rows returned for a key.
    /// This method may be called often, and in rapid succession, and so should return quickly.
    fn estimate(&self) -> usize;
}

/// An implementation of `EqualityIndex` that uses a `HashMap`.
#[derive(Clone)]
pub struct HashIndex<K: Eq + Hash> {
    num: usize,
    map: HashMap<K, Vec<usize>>,
}

impl<K: Eq + Hash> HashIndex<K> {
    /// Allocate a new `HashIndex`.
    pub fn new() -> HashIndex<K> {
        HashIndex {
            map: HashMap::new(),
            num: 0,
        }
    }
}

impl<T: Eq + Hash> EqualityIndex<T> for HashIndex<T> {
    fn lookup<'a>(&'a self, key: &T) -> Box<Iterator<Item = usize> + 'a> {
        match self.map.get(key) {
            Some(ref v) => Box::new(v.iter().map(|row| *row)),
            None => Box::new(None.into_iter()),
        }
    }

    fn index(&mut self, key: T, row: usize) {
        self.map.entry(key).or_insert_with(Vec::new).push(row);
        self.num += 1;
    }

    fn undex(&mut self, key: &T, row: usize) {
        let mut empty = false;
        if let Some(mut l) = self.map.get_mut(key) {
            empty = {
                match l.iter().position(|&r| r == row) {
                    Some(i) => {
                        l.swap_remove(i);
                    }
                    None => unreachable!(),
                }
                l.is_empty()
            };
        }
        if empty {
            self.map.remove(key);
        }
    }

    fn estimate(&self) -> usize {
        let len = self.map.len();
        if len > 0 {
            self.num / self.map.len()
        } else {
            0
        }
    }
}

/// A `RangeIndex` is an index that, in addition to performing efficient equality lookups, can
/// *also* perform efficient range queries.
pub trait RangeIndex<T>: EqualityIndex<T> {
    /// Return an iterator that yields the indices of all rows whose value (in the column this
    /// index is assigned to) lies within the given `Bound`s.
    fn between<'a>(&'a self, Bound<&T>, Bound<&T>) -> Box<Iterator<Item = usize> + 'a>;
}

/// An implementation of `RangeIndex` using a `BTreeMap`.
#[derive(Clone)]
pub struct BTreeIndex<K: Ord + Eq> {
    num: usize,
    map: BTreeMap<K, Vec<usize>>,
}

impl<K: Ord + Eq> BTreeIndex<K> {
    /// Allocate a new `BTreeIndex`.
    pub fn new() -> BTreeIndex<K> {
        BTreeIndex {
            map: BTreeMap::new(),
            num: 0,
        }
    }
}

impl<T: Ord + Eq> EqualityIndex<T> for BTreeIndex<T> {
    fn lookup<'a>(&'a self, key: &T) -> Box<Iterator<Item = usize> + 'a> {
        match self.map.get(key) {
            Some(ref v) => Box::new(v.iter().map(|row| *row)),
            None => Box::new(None.into_iter()),
        }
    }

    fn index(&mut self, key: T, row: usize) {
        self.map.entry(key).or_insert_with(Vec::new).push(row);
        self.num += 1;
    }

    fn undex(&mut self, key: &T, row: usize) {
        if let Some(ref mut l) = self.map.get_mut(key) {
            self.num -= l.len();
            l.retain(|&i| i != row);
            self.num += l.len();
        }
    }

    fn estimate(&self) -> usize {
        self.num / self.map.len()
    }
}
impl<T: Ord + Eq> RangeIndex<T> for BTreeIndex<T> {
    fn between<'a>(&'a self, min: Bound<&T>, max: Bound<&T>) -> Box<Iterator<Item = usize> + 'a> {
        Box::new(self.map.range(min, max).flat_map(|rows| rows.1.iter().map(|row| *row)))
    }
}

/// A sum type expressing all different types of indices so they can easily be stored. Since all
/// indices must at least implement `EqualityIndex`, this enum also forwards all calls of
/// that trait to the underlying index for convenience.
pub enum Index<T> {
    /// A `RangeIndex` trait object.
    Range(Box<RangeIndex<T> + Send + Sync>),
    /// An `EqualityIndex` trait object.
    Equality(Box<EqualityIndex<T> + Send + Sync>),
}

impl<T> EqualityIndex<T> for Index<T> {
    fn lookup<'a>(&'a self, key: &T) -> Box<Iterator<Item = usize> + 'a> {
        match *self {
            Index::Range(ref ri) => ri.lookup(key),
            Index::Equality(ref ei) => ei.lookup(key),
        }
    }
    fn index(&mut self, key: T, row: usize) {
        match *self {
            Index::Range(ref mut ri) => ri.index(key, row),
            Index::Equality(ref mut ei) => ei.index(key, row),
        }
    }
    fn undex(&mut self, key: &T, row: usize) {
        match *self {
            Index::Range(ref mut ri) => ri.undex(key, row),
            Index::Equality(ref mut ei) => ei.undex(key, row),
        }
    }
    fn estimate(&self) -> usize {
        match *self {
            Index::Range(ref ri) => ri.estimate(),
            Index::Equality(ref ei) => ei.estimate(),
        }
    }
}

impl<T: Eq + Hash + 'static + Send + Sync> From<HashIndex<T>> for Index<T> {
    fn from(x: HashIndex<T>) -> Index<T> {
        Index::Equality(Box::new(x))
    }
}

impl<T: Ord + Eq + 'static + Send + Sync> From<BTreeIndex<T>> for Index<T> {
    fn from(x: BTreeIndex<T>) -> Index<T> {
        Index::Range(Box::new(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashmap_eq_index() {
        use super::EqualityIndex;
        let mut eqidx = HashIndex::new();
        assert_eq!(eqidx.lookup(&"a").count(), 0);
        eqidx.index("a", 0);
        assert_eq!(eqidx.lookup(&"a").count(), 1);
        eqidx.index("a", 1);
        assert_eq!(eqidx.lookup(&"a").count(), 2);
        eqidx.undex(&"a", 0);
        assert_eq!(eqidx.lookup(&"a").count(), 1);
    }

    #[test]
    fn btree_eq_index() {
        use super::EqualityIndex;
        let mut idx = BTreeIndex::new();
        assert_eq!(idx.lookup(&"a").count(), 0);
        idx.index("a", 0);
        assert_eq!(idx.lookup(&"a").count(), 1);
        idx.index("a", 1);
        assert_eq!(idx.lookup(&"a").count(), 2);
        idx.undex(&"a", 0);
        assert_eq!(idx.lookup(&"a").count(), 1);
    }

    #[test]
    fn btree_range_index() {
        use super::RangeIndex;
        use std::collections::Bound::Included;

        let mut idx = BTreeIndex::new();
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 0);
        idx.index("a", 0);
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 1);
        idx.index("b", 1);
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 2);
        idx.undex(&"b", 1);
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 1);
    }
}
