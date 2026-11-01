//! SQL/PGQ statement preparation before Grafeo parses it.
//!
//! Grafeo's SQL/PGQ translator names every anonymous node pattern `_anon`,
//! so two anonymous nodes in one `MATCH` are joined as the same node: in
//! `(a)-[:O]->(:X)-[:I]->(p)<-[:I]-(:X)<-[:O]-(b)` both `:X` must be one
//! node, so `b` can only be `a`. Its Cypher and GQL translators number them
//! instead.
//! `RushDB` does not patch Grafeo, so each anonymous node pattern gets its own
//! generated variable here.

use std::borrow::Cow;

/// Prefix of the variables given to anonymous node patterns. Not a name a
/// statement is expected to use.
const ANONYMOUS_PREFIX: &str = "_rushdb_anon_";

/// `statement` with a distinct variable in every anonymous node pattern
/// (`(:Label)`, `({...})`, `()`) of its `MATCH` clauses. Node patterns are
/// the parentheses at the top level of a `MATCH`, up to its `WHERE` or
/// `COLUMNS`. String literals, quoted identifiers and comments are skipped
/// with the rules of Grafeo's SQL/PGQ lexer.
pub(crate) fn name_anonymous_nodes(statement: &str) -> Cow<'_, str> {
    let bytes = statement.as_bytes();
    let mut insertions = Vec::new();
    let mut depth = 0usize;
    // The bracket depth of the `MATCH` being read, if inside one.
    let mut pattern_depth: Option<usize> = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b'\'' | b'"' => index = skip_quoted(bytes, index),
            b'-' if is_line_comment(bytes, index) => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index < bytes.len()
                    && !(bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/'))
                {
                    index += 1;
                }
                index += 2;
            }
            b'(' | b'[' | b'{' => {
                if byte == b'(' && pattern_depth == Some(depth) && is_anonymous(bytes, index + 1) {
                    insertions.push(index + 1);
                }
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                // The parenthesis that closes the `GRAPH_TABLE` ends its pattern.
                if pattern_depth.is_some_and(|pattern| depth < pattern) {
                    pattern_depth = None;
                }
                index += 1;
            }
            _ if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                let word = &statement[start..index];
                if word.eq_ignore_ascii_case("MATCH") {
                    pattern_depth = Some(depth);
                } else if pattern_depth == Some(depth)
                    && (word.eq_ignore_ascii_case("WHERE") || word.eq_ignore_ascii_case("COLUMNS"))
                {
                    pattern_depth = None;
                }
            }
            _ => index += 1,
        }
    }
    if insertions.is_empty() {
        return Cow::Borrowed(statement);
    }
    let mut named = String::with_capacity(statement.len() + insertions.len() * 16);
    let mut copied = 0;
    for (ordinal, at) in insertions.into_iter().enumerate() {
        named.push_str(&statement[copied..at]);
        named.push_str(ANONYMOUS_PREFIX);
        named.push_str(&ordinal.to_string());
        copied = at;
    }
    named.push_str(&statement[copied..]);
    Cow::Owned(named)
}

/// Whether the node pattern whose body starts at `index` has no variable:
/// it opens with a label, a property map, or closes at once.
fn is_anonymous(bytes: &[u8], mut index: usize) -> bool {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    matches!(bytes.get(index), Some(b':' | b'{' | b')'))
}

/// `--` followed by whitespace or the end starts a line comment; another
/// `--` is an edge.
fn is_line_comment(bytes: &[u8], index: usize) -> bool {
    bytes.get(index + 1) == Some(&b'-')
        && bytes
            .get(index + 2)
            .is_none_or(|next| matches!(next, b' ' | b'\t' | b'\n' | b'\r'))
}

/// The index after the string literal or quoted identifier opening at
/// `start`. Single-quoted strings take backslash escapes.
fn skip_quoted(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if quote == b'\'' => index += 2,
            byte if byte == quote => return index + 1,
            _ => index += 1,
        }
    }
    index
}

#[cfg(test)]
mod tests {
    use super::name_anonymous_nodes;

    #[test]
    fn anonymous_node_patterns_get_distinct_variables() {
        assert_eq!(
            name_anonymous_nodes(
                "SELECT * FROM GRAPH_TABLE (MATCH (a:C)-[:O]->(:X)<-[:O]-( :X {k: 'a(:b)'}), () WHERE a.n = '(:Y)' COLUMNS (count(a) AS c)) AS g"
            ),
            "SELECT * FROM GRAPH_TABLE (MATCH (a:C)-[:O]->(_rushdb_anon_0:X)<-[:O]-(_rushdb_anon_1 :X {k: 'a(:b)'}), (_rushdb_anon_2) WHERE a.n = '(:Y)' COLUMNS (count(a) AS c)) AS g"
        );
    }

    #[test]
    fn named_patterns_strings_and_comments_are_left_alone() {
        for statement in [
            "SELECT * FROM GRAPH_TABLE (MATCH (a:C)-[e:O]->(b) COLUMNS (a.n AS n)) AS g",
            "SELECT * FROM GRAPH_TABLE (MATCH (a:C) WHERE a.n IN ('(:X)') COLUMNS (now() AS t)) AS g",
            "SELECT 'MATCH (:X)' AS s -- MATCH (:X)\n/* MATCH (:X) */",
        ] {
            assert_eq!(name_anonymous_nodes(statement), statement);
        }
    }
}
