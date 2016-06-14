/// A value represents something to compare against.
#[derive(Clone)]
pub enum Value<T> {
    /// A constant value literal.
    Const(T),

    /// A different column for the same row. Note that comparisons of this kind *cannot use an
    /// index*, at least not in the current implementation.
    Column(usize),
}

impl<T> Value<T> {
    /// Extract the value literal for this `Value` when evaluated for the given row.
    /// For `Const` values, this evaluates to the `Const` value itself. For `Column`, it evaluates
    /// to the value of that column in the given row.
    pub fn value<'a>(&'a self, row: &'a [T]) -> &'a T {
        match *self {
            Value::Column(i) => &row[i],
            Value::Const(ref val) => val,
        }
    }
}

/// A comparison to perform for a literal value against a `Value`.
#[derive(Clone)]
pub enum Comparison<T: PartialOrd> {
    /// Is the value equal to the given `Value`?
    Equal(Value<T>),
}

impl<T: PartialOrd> Comparison<T> {
    /// Returns true if the given value compares successfully against this `Value` when evaluated
    /// against the given row.
    pub fn matches(&self, value: &T, row: &[T]) -> bool {
        match *self {
            Comparison::Equal(ref v) => value == v.value(row),
        }
    }
}

/// A single condition to evaluate for a row in the dataset.
#[derive(Clone)]
pub struct Condition<T: PartialOrd> {
    /// The column of the row to use as the comparison value.
    pub column: usize,

    /// The comparison to perform on the selected value.
    pub cmp: Comparison<T>,
}

impl<T: PartialOrd> Condition<T> {
    /// Returns true if this condition holds true for the given row. To determine if this is the
    /// case, `row[self.column]` is extracted, and is evaluated using the comparison in `self.cmp`.
    pub fn matches(&self, row: &[T]) -> bool {
        self.cmp.matches(&row[self.column], row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value() {
        assert_eq!(Value::Column(0).value(&["a"]), &"a");
        assert_eq!(Value::Const("a").value(&["b"]), &"a");
    }

    #[test]
    fn cmp_eq() {
        assert!(Comparison::Equal(Value::Column(0)).matches(&"a", &["a"]));
        assert!(!Comparison::Equal(Value::Column(0)).matches(&"a", &["b"]));
        assert!(Comparison::Equal(Value::Const("a")).matches(&"a", &["b"]));
        assert!(!Comparison::Equal(Value::Const("b")).matches(&"a", &["a"]));
    }

    #[test]
    fn cond_eq() {
        let cmpf0 = Comparison::Equal(Value::Column(0));
        let cmpca = Comparison::Equal(Value::Const("a"));
        let cmpcb = Comparison::Equal(Value::Const("b"));

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

        assert!(cf10.matches(&["a", "a"]));
        assert!(!cf10.matches(&["a", "b"]));
        assert!(cca.matches(&["a"]));
        assert!(!cca.matches(&["b"]));
        assert!(ccb.matches(&["b"]));
        assert!(!ccb.matches(&["a"]));
    }
}
