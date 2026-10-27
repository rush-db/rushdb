//! Neo4j-style index DDL recognised before the query translator (SD-4.6): `CREATE INDEX [name] [IF NOT EXISTS] FOR (n:Label) ON (n.prop)`,
//! `CREATE FULLTEXT INDEX [name] [IF NOT EXISTS] FOR (n:Label) ON EACH
//! [n.prop] [OPTIONS {indexConfig: {consistency: 'immediate'}}]`,
//! `DROP INDEX name [IF EXISTS]`, and `SHOW INDEX[ES]`.

/// Kind of a declared index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdlKind {
    /// Ordered (equality, ranges, prefix).
    Ordered,
    /// Full-text; `immediate` = read-your-writes, else eventual.
    Fulltext {
        /// `consistency: 'immediate'` was given.
        immediate: bool,
    },
}

/// One recognised index statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDdl {
    /// Declare an ordered index.
    Create {
        /// Index name (`<label>_<property>` when omitted).
        name: String,
        /// Record label.
        label: String,
        /// Property name.
        property: String,
        /// Index kind.
        kind: DdlKind,
        /// `IF NOT EXISTS` was given.
        if_not_exists: bool,
    },
    /// Drop an index.
    Drop {
        /// Index name.
        name: String,
        /// `IF EXISTS` was given.
        if_exists: bool,
    },
    /// List indexes.
    Show,
}

fn words(statement: &str) -> Vec<&str> {
    statement
        .trim()
        .trim_end_matches(';')
        .split_whitespace()
        .collect()
}

fn keyword(word: Option<&&str>, expected: &str) -> bool {
    word.is_some_and(|word| word.eq_ignore_ascii_case(expected))
}

/// Parse `statement` as index DDL: `None` when it is not one (the caller
/// runs it as a query), `Some(Err)` when it starts like one but is malformed.
#[must_use]
pub fn parse_index_ddl(statement: &str) -> Option<Result<IndexDdl, String>> {
    let w = words(statement);
    if keyword(w.first(), "SHOW") && (keyword(w.get(1), "INDEXES") || keyword(w.get(1), "INDEX")) {
        return Some(if w.len() == 2 {
            Ok(IndexDdl::Show)
        } else {
            Err("SHOW INDEXES takes no arguments".into())
        });
    }
    if keyword(w.first(), "DROP") && keyword(w.get(1), "INDEX") {
        return Some(match w.as_slice() {
            [_, _, name] => Ok(IndexDdl::Drop {
                name: (*name).to_owned(),
                if_exists: false,
            }),
            [_, _, name, if_, exists]
                if if_.eq_ignore_ascii_case("IF") && exists.eq_ignore_ascii_case("EXISTS") =>
            {
                Ok(IndexDdl::Drop {
                    name: (*name).to_owned(),
                    if_exists: true,
                })
            }
            _ => Err("expected DROP INDEX <name> [IF EXISTS]".into()),
        });
    }
    if keyword(w.first(), "CREATE") && keyword(w.get(1), "INDEX") {
        return Some(parse_create(statement, 2, DdlKind::Ordered));
    }
    if keyword(w.first(), "CREATE") && keyword(w.get(1), "FULLTEXT") && keyword(w.get(2), "INDEX") {
        let upper = statement.to_ascii_uppercase();
        let immediate = upper.contains("CONSISTENCY") && upper.contains("'IMMEDIATE'");
        return Some(parse_create(statement, 3, DdlKind::Fulltext { immediate }));
    }
    None
}

fn parse_create(statement: &str, keywords: usize, kind: DdlKind) -> Result<IndexDdl, String> {
    let malformed = || {
        "expected CREATE [FULLTEXT] INDEX [name] [IF NOT EXISTS] FOR (n:Label) ON (n.prop) | ON EACH [n.prop]"
            .to_string()
    };
    let text = statement.trim().trim_end_matches(';');
    // OPTIONS {...} only configures consistency (read by the caller).
    let text = match text.to_ascii_uppercase().find(" OPTIONS ") {
        Some(at) => &text[..at],
        None => text,
    };
    let upper = text.to_ascii_uppercase();
    let for_at = upper.find(" FOR ").ok_or_else(malformed)?;
    let head: Vec<&str> = text[..for_at].split_whitespace().skip(keywords).collect();
    let (name, if_not_exists) = match head.as_slice() {
        [] => (None, false),
        [name] => (Some((*name).to_owned()), false),
        [if_, not, exists]
            if if_.eq_ignore_ascii_case("IF")
                && not.eq_ignore_ascii_case("NOT")
                && exists.eq_ignore_ascii_case("EXISTS") =>
        {
            (None, true)
        }
        [name, if_, not, exists]
            if if_.eq_ignore_ascii_case("IF")
                && not.eq_ignore_ascii_case("NOT")
                && exists.eq_ignore_ascii_case("EXISTS") =>
        {
            (Some((*name).to_owned()), true)
        }
        _ => return Err(malformed()),
    };
    let rest = &text[for_at + " FOR ".len()..];
    let on_at = rest
        .to_ascii_uppercase()
        .find(" ON ")
        .ok_or_else(malformed)?;
    // `(n:Label)`
    let pattern = rest[..on_at].trim();
    let inner = pattern
        .strip_prefix('(')
        .and_then(|p| p.strip_suffix(')'))
        .ok_or_else(malformed)?;
    let (variable, label) = inner.split_once(':').ok_or_else(malformed)?;
    let (variable, label) = (variable.trim(), label.trim().trim_matches('`'));
    // `(n.prop)`, or `EACH [n.prop]` for a full-text index
    let target = rest[on_at + " ON ".len()..].trim();
    let inner = match kind {
        DdlKind::Ordered => target.strip_prefix('(').and_then(|p| p.strip_suffix(')')),
        DdlKind::Fulltext { .. } => target
            .get(4..)
            .filter(|_| {
                target
                    .get(..4)
                    .is_some_and(|each| each.eq_ignore_ascii_case("EACH"))
            })
            .map(str::trim)
            .and_then(|list| list.strip_prefix('['))
            .and_then(|list| list.strip_suffix(']')),
    }
    .ok_or_else(malformed)?;
    let (owner, property) = inner.split_once('.').ok_or_else(malformed)?;
    if owner.trim() != variable || label.is_empty() {
        return Err(malformed());
    }
    let property = property.trim().trim_matches('`').to_owned();
    if property.is_empty() || property.contains(',') {
        return Err("a declared index covers exactly one property".into());
    }
    Ok(IndexDdl::Create {
        name: name.unwrap_or_else(|| format!("{label}_{property}")),
        label: label.to_owned(),
        property,
        kind,
        if_not_exists,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neo4j_index_ddl_forms_parse() {
        assert_eq!(
            parse_index_ddl("CREATE INDEX person_age FOR (n:Person) ON (n.age);"),
            Some(Ok(IndexDdl::Create {
                name: "person_age".into(),
                label: "Person".into(),
                property: "age".into(),
                kind: DdlKind::Ordered,
                if_not_exists: false,
            }))
        );
        assert_eq!(
            parse_index_ddl("create index if not exists for (p:Item) on (p.price)"),
            Some(Ok(IndexDdl::Create {
                name: "Item_price".into(),
                label: "Item".into(),
                property: "price".into(),
                kind: DdlKind::Ordered,
                if_not_exists: true,
            }))
        );
        assert_eq!(
            parse_index_ddl(
                "CREATE FULLTEXT INDEX docs FOR (d:Doc) ON EACH [d.body] OPTIONS {indexConfig: {consistency: 'immediate'}}"
            ),
            Some(Ok(IndexDdl::Create {
                name: "docs".into(),
                label: "Doc".into(),
                property: "body".into(),
                kind: DdlKind::Fulltext { immediate: true },
                if_not_exists: false,
            }))
        );
        assert!(matches!(
            parse_index_ddl("CREATE FULLTEXT INDEX docs FOR (d:Doc) ON EACH [d.title]"),
            Some(Ok(IndexDdl::Create {
                kind: DdlKind::Fulltext { immediate: false },
                ..
            }))
        ));
        assert_eq!(
            parse_index_ddl("DROP INDEX person_age IF EXISTS"),
            Some(Ok(IndexDdl::Drop {
                name: "person_age".into(),
                if_exists: true
            }))
        );
        assert_eq!(parse_index_ddl("SHOW INDEXES"), Some(Ok(IndexDdl::Show)));
        assert_eq!(parse_index_ddl("MATCH (n) RETURN n"), None);
        assert!(matches!(
            parse_index_ddl("CREATE INDEX x FOR (n:A) ON (m.p)"),
            Some(Err(_))
        ));
        assert!(matches!(
            parse_index_ddl("CREATE INDEX x FOR (n:A) ON (n.p, n.q)"),
            Some(Err(_))
        ));
    }
}
