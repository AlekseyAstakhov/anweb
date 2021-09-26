use crate::query::{parse_query, QueryNameValue};

impl<'a> PartialEq for QueryNameValue<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value
    }
}

#[test]
fn parse() {
    assert!(parse_query(b"").parts.is_empty());
    assert!(parse_query(b"&").parts.is_empty());
    assert!(parse_query(b"&&").parts.is_empty());
    assert!(!parse_query(b"x").parts.is_empty());
    assert_eq!(parse_query(b"x=").parts, vec![QueryNameValue { name: b"x", value: b"" }]);
    assert_eq!(parse_query(b"x=1").parts, vec![QueryNameValue { name: b"x", value: b"1" }]);
    assert_eq!(parse_query(b"x&").parts, vec![QueryNameValue { name: b"x", value: b"" }]);
    assert_eq!(parse_query(b"x&y").parts, vec![QueryNameValue { name: b"x", value: b"" }, QueryNameValue { name: b"y", value: b"" }]);
    assert_eq!(parse_query(b"x=1&y=").parts, vec![QueryNameValue { name: b"x", value: b"1" }, QueryNameValue { name: b"y", value: b"" }]);
    assert_eq!(parse_query(b"x=1&y=1").parts, vec![QueryNameValue { name: b"x", value: b"1" }, QueryNameValue { name: b"y", value: b"1" }]);
    assert_eq!(parse_query(b"x=1&y=1").parts, vec![QueryNameValue { name: b"x", value: b"1" }, QueryNameValue { name: b"y", value: b"1" }]);
    assert_eq!(parse_query(b"x&y;z").parts, vec![QueryNameValue { name: b"x", value: b"" }, QueryNameValue { name: b"y", value: b"" }, QueryNameValue { name: b"z", value: b"" }]);
    assert_eq!(
        parse_query(b"abc=xyz&test=check&xyz=abc").parts,
        vec![QueryNameValue { name: b"abc", value: b"xyz" }, QueryNameValue { name: b"test", value: b"check" }, QueryNameValue { name: b"xyz", value: b"abc" }]
    );
}
