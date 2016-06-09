pub enum Value<T> {
    Field(usize),
    Const(T),
}

impl<T> Value<T> {
    pub fn value<'a>(&'a self, row: &'a [T]) -> &'a T {
        match *self {
            Value::Field(i) => &row[i],
            Value::Const(ref val) => val,
        }
    }
}

pub enum Comparison<T: PartialOrd> {
    Equal(Value<T>),
}

impl<T: PartialOrd> Comparison<T> {
    pub fn matches(&self, value: &T, row: &[T]) -> bool {
        match *self {
            Comparison::Equal(ref v) => value == v.value(row),
        }
    }
}

pub struct Condition<T: PartialOrd> {
    pub field: usize,
    pub cmp: Comparison<T>,
}

impl<T: PartialOrd> Condition<T> {
    pub fn matches(&self, row: &[T]) -> bool {
        self.cmp.matches(&row[self.field], row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value() {
        assert_eq!(Value::Field(0).value(&["a"]), &"a");
        assert_eq!(Value::Const("a").value(&["b"]), &"a");
    }

    #[test]
    fn cmp_eq() {
        assert!(Comparison::Equal(Value::Field(0)).matches(&"a", &["a"]));
        assert!(!Comparison::Equal(Value::Field(0)).matches(&"a", &["b"]));
        assert!(Comparison::Equal(Value::Const("a")).matches(&"a", &["b"]));
        assert!(!Comparison::Equal(Value::Const("b")).matches(&"a", &["a"]));
    }

    #[test]
    fn cond_eq() {
        let cmpf0 = Comparison::Equal(Value::Field(0));
        let cmpca = Comparison::Equal(Value::Const("a"));
        let cmpcb = Comparison::Equal(Value::Const("b"));

        let cf10 = Condition {
            field: 1,
            cmp: cmpf0,
        };
        let cca = Condition {
            field: 0,
            cmp: cmpca,
        };
        let ccb = Condition {
            field: 0,
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
