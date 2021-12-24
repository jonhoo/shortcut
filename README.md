# shortcut

[![Crates.io](https://img.shields.io/crates/v/shortcut.svg)](https://crates.io/crates/shortcut)
[![Documentation](https://docs.rs/shortcut/badge.svg)](https://docs.rs/shortcut/)
[![Build Status](https://travis-ci.org/jonhoo/shortcut.svg?branch=master)](https://travis-ci.org/jonhoo/shortcut)

This crate provides an indexed, queryable column-based storage system.

The storage system is, fundamentally, row-based storage, where all rows have the same number of
columns. All columns are the same "type", but given that they can be enum types, you can
effectively use differently typed values. Data is stored in a `BTreeMap<usize, Vec<T>>`,
where the outermost `BTreeMap` is dynamically sized (and may be re-allocated as more rows come
in), whereas the innermost `Vec` is expected to never change. The map index is an
autoincremented row identifier similar to the one used by SQLite:
https://www.sqlite.org/lang_createtable.html#rowid.

What makes this crate interesting is that it also allows you to place indices on columns for
fast lookups. These indices are automatically updated whenever the dataset changes, so that
queries continue to return correct results. Indices should conform to either the
`EqualityIndex` trait or the `RangeIndex` trait. As you would expect, the former allows
speeding up exact lookups, whereas the latter can also perform efficient range queries.

Queries are performed over the dataset by calling `find` with a set of `Condition`s that will
be `AND`ed together. `OR` is currently not supported --- issue multiple quieries instead. Each
`Condition` represents a value comparison against the value in a single column. The system
automatically picks what index to use to satisfy the query, using a heuristic based on the
expected number of rows returned for that column for each index.

## Known limitations

 - The set of match operations is currently fairly limited.
 - The system currently provides an add/remove-only abstraction (i.e., no edit).
