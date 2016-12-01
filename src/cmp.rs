use Row;
use std::fmt;
use std::borrow::Cow;
use std::borrow::Borrow;

/// A value represents something to compare against.
#[derive(Clone, Debug)]
pub enum Value<'a, T: Clone + 'a> {
    /// A constant value literal.
    Const(Cow<'a, T>),

    /// A different column for the same row. Note that comparisons of this kind *cannot use an
    /// index*, at least not in the current implementation.
    Column(usize),
}

impl<'a, T: Clone + 'a> Value<'a, T> {
    /// Extract the value literal for this `Value` when evaluated for the given row.
    /// For `Const` values, this evaluates to the `Const` value itself. For `Column`, it evaluates
    /// to the value of that column in the given row.
    pub fn value<'b: 'a, R: Row<T> + ?Sized>(&'b self, row: &'b R) -> &'b T {
        match *self {
            Value::Column(i) => &row.index(i),
            Value::Const(ref val) => val,
        }
    }

    /// Construct a new `Value` by moving an existing value.
    pub fn new<I: Into<T>>(t: I) -> Self {
        Value::Const(Cow::Owned(t.into()))
    }

    /// Construct a new `Value` by using a reference to an existing value.
    pub fn using<I: Borrow<T>>(t: &'a I) -> Self {
        Value::Const(Cow::Borrowed(t.borrow()))
    }

    /// Construct a new `Value` that refers to the value in a particular column of a row.
    pub fn column(c: usize) -> Self {
        Value::Column(c)
    }
}

/// A comparison to perform for a literal value against a `Value`.
#[derive(Clone, Debug)]
pub enum Comparison<'a, T: Clone + 'a> {
    /// Is the value equal to the given `Value`?
    Equal(Value<'a, T>),
}

impl<'a, T: Ord + Clone + 'a> Comparison<'a, T> {
    /// Returns true if the given value compares successfully against this `Value` when evaluated
    /// against the given row.
    pub fn matches<R: Row<T> + ?Sized>(&self, value: &T, row: &R) -> bool {
        match *self {
            Comparison::Equal(ref v) => value == v.value(row),
        }
    }
}

/// A single condition to evaluate for a row in the dataset.
#[derive(Clone, Debug)]
pub struct Condition<'a, T: Clone + 'a> {
    /// The column of the row to use as the comparison value.
    pub column: usize,

    /// The comparison to perform on the selected value.
    pub cmp: Comparison<'a, T>,
}

impl<'a, T: Ord + Clone + 'a> Condition<'a, T> {
    /// Returns true if this condition holds true for the given row. To determine if this is the
    /// case, `row[self.column]` is extracted, and is evaluated using the comparison in `self.cmp`.
    pub fn matches<R: Row<T> + ?Sized>(&self, row: &R) -> bool {
        self.cmp.matches(&row.index(self.column), row)
    }
}

impl<'a, T: fmt::Display + Clone + 'a> fmt::Display for Value<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Value::Column(i) => write!(f, "[{}]", i),
            Value::Const(ref val) => write!(f, "{}", val),
        }
    }
}

impl<'a, T: fmt::Display + Clone + 'a> fmt::Display for Comparison<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Comparison::Equal(ref v) => write!(f, "= {}", v),
        }
    }
}

impl<'a, T: fmt::Display + Clone + 'a> fmt::Display for Condition<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}] {}", self.column, self.cmp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value() {
        let a = &["a"];
        let b = &["b"];
        assert_eq!(Value::column(0).value(&a[..]), &"a");
        assert_eq!(Value::new("a").value(&b[..]), &"a");
    }

    #[test]
    fn cmp_eq() {
        let a = &["a"];
        let b = &["b"];
        assert!(Comparison::Equal(Value::column(0)).matches(&"a", &a[..]));
        assert!(!Comparison::Equal(Value::column(0)).matches(&"a", &b[..]));
        assert!(Comparison::Equal(Value::new("a")).matches(&"a", &b[..]));
        assert!(!Comparison::Equal(Value::new("b")).matches(&"a", &a[..]));
    }

    #[test]
    fn borrowed_values() {
        let a = vec!["a".to_string()];
        let b = vec!["b".to_string()];
        assert!(Comparison::Equal(Value::column(0)).matches(&a[0], &a));
        assert!(!Comparison::Equal(Value::column(0)).matches(&a[0], &b));
        assert!(Comparison::Equal(Value::using(&a[0])).matches(&a[0], &b));
        assert!(!Comparison::Equal(Value::using(&b[0])).matches(&a[0], &a));
    }

    #[test]
    fn through_deref() {
        let a = vec!["a".to_string()];
        let b = vec!["b".to_string()];
        assert!(Comparison::Equal(Value::column(0)).matches(&a[0], &a));
        assert!(!Comparison::Equal(Value::column(0)).matches(&a[0], &b));
        assert!(Comparison::Equal(Value::new("a")).matches(&a[0], &b));
        assert!(!Comparison::Equal(Value::new("b")).matches(&a[0], &a));
    }

    #[test]
    fn cond_eq() {
        let cmpf0 = Comparison::Equal(Value::column(0));
        let cmpca = Comparison::Equal(Value::new("a"));
        let cmpcb = Comparison::Equal(Value::new("b"));

        let cf10 = Condition {
            column: 1,
            cmp: cmpf0,
        };
        let cca = Condition {
            column: 0,
            cmp: cmpca,
        };
        let ccb = Condition {
            column: 0,
            cmp: cmpcb,
        };

        let a = &["a"];
        let b = &["b"];
        let aa = &["a", "a"];
        let ab = &["a", "b"];
        assert!(cf10.matches(&aa[..]));
        assert!(!cf10.matches(&ab[..]));
        assert!(cca.matches(&a[..]));
        assert!(!cca.matches(&b[..]));
        assert!(ccb.matches(&b[..]));
        assert!(!ccb.matches(&a[..]));
    }

    #[test]
    fn display() {
        let cf01: Condition<String> = Condition {
            column: 0,
            cmp: Comparison::Equal(Value::Column(1)),
        };

        let cca = Condition {
            column: 0,
            cmp: Comparison::Equal::<&str>(Value::new("a")),
        };

        assert_eq!(format!("{}", cf01), "[0] = [1]");
        assert_eq!(format!("{}", cca), "[0] = a")
    }
}
