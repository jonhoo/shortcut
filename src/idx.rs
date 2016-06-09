use std::collections::HashMap;
use std::hash::Hash;

use std::collections::BTreeMap;
use std::collections::Bound;

pub trait EqualityIndex<T> {
    fn lookup<'a>(&'a self, &T) -> Box<Iterator<Item = usize> + 'a>;
    fn index(&mut self, T, usize);
    fn undex(&mut self, T, usize);
}

impl<T: Eq + Hash> EqualityIndex<T> for HashMap<T, Vec<usize>> {
    fn lookup<'a>(&'a self, key: &T) -> Box<Iterator<Item = usize> + 'a> {
        match self.get(key) {
            Some(ref v) => Box::new(v.iter().map(|row| *row)),
            None => Box::new(None.into_iter()),
        }
    }

    fn index(&mut self, key: T, row: usize) {
        self.entry(key).or_insert_with(Vec::new).push(row);
    }

    fn undex(&mut self, key: T, row: usize) {
        use std::collections::hash_map::Entry;
        if let Entry::Occupied(ref mut e) = self.entry(key) {
            e.get_mut().retain(|&i| i != row);
        }
    }
}

pub trait RangeIndex<T>: EqualityIndex<T> {
    fn between<'a>(&'a self, Bound<&T>, Bound<&T>) -> Box<Iterator<Item = usize> + 'a>;
}

impl<T: Ord + Eq + Hash> EqualityIndex<T> for BTreeMap<T, Vec<usize>> {
    fn lookup<'a>(&'a self, key: &T) -> Box<Iterator<Item = usize> + 'a> {
        match self.get(key) {
            Some(ref v) => Box::new(v.iter().map(|row| *row)),
            None => Box::new(None.into_iter()),
        }
    }

    fn index(&mut self, key: T, row: usize) {
        self.entry(key).or_insert_with(Vec::new).push(row);
    }

    fn undex(&mut self, key: T, row: usize) {
        use std::collections::btree_map::Entry;
        if let Entry::Occupied(ref mut e) = self.entry(key) {
            e.get_mut().retain(|&i| i != row);
        }
    }
}
impl<T: Ord + Eq + Hash> RangeIndex<T> for BTreeMap<T, Vec<usize>> {
    fn between<'a>(&'a self, min: Bound<&T>, max: Bound<&T>) -> Box<Iterator<Item = usize> + 'a> {
        Box::new(self.range(min, max).flat_map(|rows| rows.1.iter().map(|row| *row)))
    }
}

pub enum Index<T> {
    Range(RangeIndex<T>),
    Equality(EqualityIndex<T>),
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
    fn undex(&mut self, key: T, row: usize) {
        match *self {
            Index::Range(ref mut ri) => ri.undex(key, row),
            Index::Equality(ref mut ei) => ei.undex(key, row),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::collections::BTreeMap;

    #[test]
    fn hashmap_eq_index() {
        use super::EqualityIndex;
        let mut eqidx = HashMap::new();
        assert_eq!(eqidx.lookup(&"a").count(), 0);
        eqidx.index("a", 0);
        assert_eq!(eqidx.lookup(&"a").count(), 1);
        eqidx.index("a", 1);
        assert_eq!(eqidx.lookup(&"a").count(), 2);
        eqidx.undex("a", 0);
        assert_eq!(eqidx.lookup(&"a").count(), 1);
    }

    #[test]
    fn btree_eq_index() {
        use super::EqualityIndex;
        let mut idx = BTreeMap::new();
        assert_eq!(idx.lookup(&"a").count(), 0);
        idx.index("a", 0);
        assert_eq!(idx.lookup(&"a").count(), 1);
        idx.index("a", 1);
        assert_eq!(idx.lookup(&"a").count(), 2);
        idx.undex("a", 0);
        assert_eq!(idx.lookup(&"a").count(), 1);
    }

    #[test]
    fn btree_range_index() {
        use super::RangeIndex;
        use std::collections::Bound::{Included, Unbounded};

        let mut idx = BTreeMap::new();
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 0);
        idx.index("a", 0);
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 1);
        idx.index("b", 1);
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 2);
        idx.undex("b", 1);
        assert_eq!(idx.between(Included(&"a"), Included(&"b")).count(), 1);
    }
}
