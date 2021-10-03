use percent_encoding::percent_decode;
use std::fmt::Debug;

#[derive(Debug)]
/// Parsed query.
pub struct Query <'a, 'b> {
    pub parts: Vec<QueryNameValue<'a, 'b>>,
}

/// Query part as "b=2" in request like "GET /?a=1&b=2&c=3 HTTP/1.1\r\n\r\n".
pub struct QueryNameValue <'a, 'b> {
    /// Name. Can't be empty.
    pub name: &'a [u8],
    /// Value. Can be empty.
    pub value: &'b [u8],
}

impl Query<'_, '_> {
    /// Return first value by name.
    pub fn value(&self, name: &str) -> Option<String> {
        for query_part in self.iter() {
            if query_part.name == name.as_bytes() {
                if let Ok(decoded_value) = percent_decode(query_part.value).decode_utf8() {
                    return Some(decoded_value.to_string());
                }
            }
        }

        None
    }

    /// Return first value by index.
    pub fn value_at(&self, index: usize) -> Option<String> {
        if let Some(query_part) = self.parts.get(index) {
            if let Ok(decoded_value) = percent_decode(query_part.value).decode_utf8() {
                return Some(decoded_value.replace('+', " "));
            }
        }

        None
    }
}

impl<'a, 'b> std::ops::Deref for Query<'a, 'b> {
    type Target = Vec<QueryNameValue<'a, 'b>>;

    fn deref(&self) -> &Self::Target {
        &self.parts
    }
}

impl std::ops::DerefMut for Query<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parts
    }
}

/// Parse raw query. Splits to names and values array.
pub fn parse_query(query: &[u8]) -> Query {
    let mut result = Query { parts: Vec::new() };
    let mut token_index = 0;

    let query_len = query.len();

    for (i, ch) in query.iter().enumerate() {
        if *ch == b'&' || *ch == b';' {
            let name = &query[token_index..i];
            if !name.is_empty() {
                result.push(QueryNameValue { name, value: &[] });
            }
            token_index = i + 1;
        } else if i == query_len - 1 {
            let name = &query[token_index..=i];
            if !name.is_empty() {
                result.push(QueryNameValue { name, value: &[] });
            }
            token_index = i;
        }
    }

    for query_part in result.iter_mut() {
        for (i, ch) in query_part.name.iter().enumerate() {
            if *ch == b'=' && i > 0 {
                let name = &query_part.name[0..i];
                let value = &query_part.name[i + 1..];
                query_part.name = name;
                query_part.value = value;
                break;
            }
        }
    }

    result
}

impl Debug for QueryNameValue<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("QueryNameValue");
        let f = if let Ok(decoded_name) = percent_decode(&self.name).decode_utf8() {
            f.field("name", &decoded_name)
        } else {
            f.field("name", &self.name)
        };

        let f = if let Ok(decoded_name) = percent_decode(&self.value).decode_utf8() {
            f.field("value", &decoded_name)
        } else {
            f.field("value", &self.value)
        };

        f.finish()
    }
}
