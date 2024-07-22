use std::fmt::{Debug, Display};

#[derive(Debug)]
pub struct Filter(String);

pub enum FilterOp {
    Equals,
    NotEqual,
    GreaterThan,
    GreatherThanEquals,
    LessThan,
    LessThanEquals,
}

pub struct Guid(String);

impl Guid {
    #[inline]
    fn serialize(&self) -> String {
        format!("guid'{}'", self.0)
    }

    #[inline]
    pub fn new<S: AsRef<str>>(guid: S) -> Self {
        Self(guid.as_ref().to_string())
    }
}

impl ToString for Guid {
    fn to_string(&self) -> String {
        self.serialize()
    }
}

pub trait FilterValue: ToString {
    fn serialize(&self) -> String {
        format!("'{}'", self.to_string())
    }
}

impl FilterValue for String {}
impl FilterValue for Guid {
    fn serialize(&self) -> String {
        Guid::serialize(&self)
    }
}
impl<'a> FilterValue for &'a str {}
impl<'a, T: FilterValue> FilterValue for &'a T where &'a T: ToString {}

pub struct Bool(bool);

impl Display for Bool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = if self.0 {
            "true".to_string()
        } else {
            "false".to_string()
        };
        write!(f, "{}", str)
    }
}

impl FilterOp {
    #[inline]
    pub(crate) fn serialize(&self) -> &'static str {
        match self {
            Self::Equals => "eq",
            Self::NotEqual => "ne",
            Self::GreaterThan => "gt",
            Self::GreatherThanEquals => "ge",
            Self::LessThan => "lt",
            Self::LessThanEquals => "le",
        }
    }
}

pub enum FilterFunction {
    EndsWith(String),
    StartsWith(String),
    SubstringOf(String),
}

impl FilterFunction {
    fn apply<T: ToString + Debug>(self, to: T) -> String {
        match self {
            Self::StartsWith(v) => format!("startswith({}, {v})", to.to_string()),
            Self::EndsWith(v) => format!("endswith({}, {v})", to.to_string()),
            Self::SubstringOf(v) => format!("substringof({}, {v})", to.to_string()),
        }
    }
}

impl Filter {
    pub fn function<V: FilterValue, T: ToString + Debug>(
        mut self,
        key: T,
        f: FilterFunction,
        op: FilterOp,
        value: V,
    ) -> Self {
        self.0.push_str(&format!(
            "+{}",
            Self::format_filter_string(f.apply(key), value, op,)
        ));

        self
    }

    #[inline]
    pub fn new<T: ToString + Debug>(key: T, value: impl FilterValue, op: FilterOp) -> Self {
        Self(Self::format_filter_string(key, value, op))
    }

    #[inline]
    pub fn and<T: ToString + Debug>(
        mut self,
        key: T,
        value: impl FilterValue,
        op: FilterOp,
    ) -> Self {
        self.push_operation("and", key, value, op);
        self
    }

    #[inline]
    pub fn or<T: ToString + Debug>(
        mut self,
        key: T,
        value: impl FilterValue,
        op: FilterOp,
    ) -> Self {
        self.push_operation("or", key, value, op);
        self
    }

    #[inline]
    fn push_operation<T: ToString + Debug>(
        &mut self,
        name: &str,
        key: T,
        value: impl FilterValue,
        op: FilterOp,
    ) {
        self.0.push_str(&format!(
            "+{name}+{}",
            Self::format_filter_string(key, value, op)
        ));
    }

    #[inline]
    pub fn join_and(mut self, other: &Self) -> Self {
        self.join(other, "and");
        self
    }

    #[inline]
    pub fn join_or(mut self, other: &Self) -> Self {
        self.join(other, "or");
        self
    }

    #[inline]
    fn join(&mut self, other: &Self, op: &str) {
        self.0 = format!("({}+{op}+{})", self.0, other.0);
    }

    #[inline]
    fn format_filter_string<T: ToString + Debug>(
        key: T,
        value: impl FilterValue,
        op: FilterOp,
    ) -> String {
        format!(
            "{}+{}+{}",
            key.to_string(),
            op.serialize(),
            value.serialize()
        )
    }

    #[inline]
    pub fn finalize(mut self) -> String {
        if self.0.starts_with("(") && self.0.ends_with(")") {
            self.0.remove(0);
            self.0.remove(self.0.len() - 1);
        }

        self.0
    }
}

#[cfg(test)]
mod test {
    use super::{Filter, FilterOp};
    use crate::Guid;
    use strum_macros::Display;

    #[derive(Display, Debug)]
    pub enum TestKeys {
        Foo,
        Bar,
    }

    #[test]
    fn guid() {
        let s = Filter::new(TestKeys::Foo, Guid::new("Foo"), FilterOp::Equals).finalize();
        assert_eq!(s, "Foo+eq+guid'Foo'");
    }

    #[test]
    fn eq() {
        let s = Filter::new(TestKeys::Bar, "bar", FilterOp::Equals).finalize();
        assert_eq!(s, "Bar+eq+'bar'");
    }

    #[test]
    fn ne() {
        let s = Filter::new(TestKeys::Bar, "bar", FilterOp::NotEqual).finalize();
        assert_eq!(s, "Bar+ne+'bar'");
    }

    #[test]
    fn and() {
        let s = Filter::new(TestKeys::Bar, "bar", FilterOp::NotEqual)
            .and(TestKeys::Foo, "foo", FilterOp::Equals)
            .finalize();
        assert_eq!(s, "Bar+ne+'bar'+and+Foo+eq+'foo'");
    }

    #[test]
    fn or() {
        let s = Filter::new(TestKeys::Bar, "bar", FilterOp::NotEqual)
            .or(TestKeys::Foo, "foo", FilterOp::Equals)
            .finalize();
        assert_eq!(s, "Bar+ne+'bar'+or+Foo+eq+'foo'");
    }

    #[test]
    fn join() {
        let s = Filter::new(TestKeys::Bar, "bar", FilterOp::Equals)
            .join_and(&Filter::new(TestKeys::Foo, "foo", FilterOp::NotEqual))
            .join_or(&Filter::new(TestKeys::Bar, "baz", FilterOp::Equals))
            .finalize();
        assert_eq!(s, "(Bar+eq+'bar'+and+Foo+ne+'foo')+or+Bar+eq+'baz'");
    }
}
