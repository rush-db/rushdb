//! Gremlin traversal bytecode (`TinkerPop` `GraphSON` 3) → Gremlin text.
//!
//! Language variants (gremlin-python, gremlin-javascript, Gremlin.Net, the
//! Java driver) send a traversal as bytecode: an ordered list of steps,
//! each a name and typed arguments. This translates it deterministically
//! into the text traversal the engine parses, so a driver's traversal runs
//! without the client writing Gremlin text.
//!
//! Accepted: `g:Bytecode` (or its untyped `{"step": [...], "source":
//! [...]}` form) with JSON scalars, `g:Int32`/`g:Int64`/`g:Float`/
//! `g:Double`, `g:List`/`g:Set`, `g:Map`, `g:P` (including `and`/`or`/
//! `not`), `g:TextP`, the `g:T`/`g:Order`/`g:Scope`/`g:Column`/
//! `g:Direction`/`g:Pop`/`g:Cardinality` enums and nested `g:Bytecode`
//! (anonymous traversals, written as bare chained steps, `where(out('x'))`:
//! valid TinkerPop grammar and what the engine's parser reads).
//! Refused: lambdas, traversal strategies and other source steps, and any
//! type not listed.

use serde_json::Value as Json;

use crate::LanguageError;

/// Nesting allowed for anonymous traversals and argument values.
const MAX_DEPTH: usize = 64;

fn unsupported(message: impl Into<String>) -> LanguageError {
    LanguageError::UnsupportedMutation(format!("gremlin bytecode: {}", message.into()))
}

/// The Gremlin text of a top-level traversal `bytecode`.
///
/// # Errors
/// Returns [`LanguageError::UnsupportedMutation`] for malformed bytecode or
/// a construct this translation does not accept.
pub fn bytecode_to_gremlin(bytecode: &Json) -> Result<String, LanguageError> {
    let (source, steps) = unwrap_bytecode(bytecode)?;
    if !source.is_empty() {
        return Err(unsupported(
            "source instructions (strategies, side effects) are not accepted",
        ));
    }
    if steps.is_empty() {
        return Err(unsupported("a traversal needs at least one step"));
    }
    let mut out = String::from("g");
    for step in steps {
        out.push('.');
        out.push_str(&instruction(step, 0)?);
    }
    Ok(out)
}

/// `(source, step)` instruction lists of a bytecode value.
fn unwrap_bytecode(value: &Json) -> Result<(&[Json], &[Json]), LanguageError> {
    let body = match typed(value) {
        Some(("g:Bytecode", inner)) => inner,
        Some((other, _)) => return Err(unsupported(format!("expected g:Bytecode, got {other}"))),
        None => value,
    };
    let list = |key: &str| -> Result<&[Json], LanguageError> {
        match body.get(key) {
            None => Ok(&[]),
            Some(Json::Array(items)) => Ok(items),
            Some(_) => Err(unsupported(format!(
                "`{key}` must be a list of instructions"
            ))),
        }
    };
    Ok((list("source")?, list("step")?))
}

/// `(@type, @value)` of a `GraphSON` typed value.
fn typed(value: &Json) -> Option<(&str, &Json)> {
    let object = value.as_object()?;
    Some((object.get("@type")?.as_str()?, object.get("@value")?))
}

fn instruction(step: &Json, depth: usize) -> Result<String, LanguageError> {
    let Some((name, args)) = step.as_array().and_then(|parts| parts.split_first()) else {
        return Err(unsupported("an instruction is a non-empty list"));
    };
    let Some(name) = name.as_str() else {
        return Err(unsupported("an instruction name is a string"));
    };
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(unsupported(format!("invalid step name `{name}`")));
    }
    let args = args
        .iter()
        .map(|arg| argument(arg, depth + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{name}({})", args.join(", ")))
}

fn argument(value: &Json, depth: usize) -> Result<String, LanguageError> {
    if depth > MAX_DEPTH {
        return Err(unsupported("nesting too deep"));
    }
    match value {
        Json::Null => Ok("null".into()),
        Json::Bool(b) => Ok(b.to_string()),
        Json::Number(n) => Ok(n.to_string()),
        Json::String(s) => Ok(quoted(s)),
        Json::Array(items) => list(items, depth),
        Json::Object(_) => match typed(value) {
            Some((kind, inner)) => typed_argument(kind, inner, depth),
            None => Err(unsupported("an untyped object argument")),
        },
    }
}

fn list(items: &[Json], depth: usize) -> Result<String, LanguageError> {
    let items = items
        .iter()
        .map(|item| argument(item, depth + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("[{}]", items.join(", ")))
}

fn typed_argument(kind: &str, inner: &Json, depth: usize) -> Result<String, LanguageError> {
    match kind {
        "g:Int32" | "g:Int64" | "g:Float" | "g:Double" => match inner {
            Json::Number(n) => Ok(n.to_string()),
            _ => Err(unsupported(format!("{kind} needs a number"))),
        },
        "g:List" | "g:Set" => match inner {
            Json::Array(items) => list(items, depth),
            _ => Err(unsupported(format!("{kind} needs a list"))),
        },
        "g:Map" => {
            let Json::Array(flat) = inner else {
                return Err(unsupported("g:Map needs a flat key/value list"));
            };
            if flat.len() % 2 != 0 {
                return Err(unsupported("g:Map needs key/value pairs"));
            }
            let entries = flat
                .chunks(2)
                .map(|pair| {
                    Ok(format!(
                        "{}: {}",
                        argument(&pair[0], depth + 1)?,
                        argument(&pair[1], depth + 1)?
                    ))
                })
                .collect::<Result<Vec<_>, LanguageError>>()?;
            Ok(format!("[{}]", entries.join(", ")))
        }
        "g:P" | "g:TextP" => predicate(inner, depth),
        "g:T" => enum_value("T", inner),
        "g:Order" | "g:Cardinality" => enum_value("", inner),
        "g:Scope" => enum_value("Scope", inner),
        "g:Column" => enum_value("Column", inner),
        "g:Direction" => enum_value("Direction", inner),
        "g:Pop" => enum_value("Pop", inner),
        "g:Bytecode" => {
            let (source, steps) = unwrap_bytecode(&serde_json::json!({
                "@type": "g:Bytecode", "@value": inner
            }))
            .map(|(source, steps)| (source.to_vec(), steps.to_vec()))?;
            if !source.is_empty() {
                return Err(unsupported("an anonymous traversal has no source"));
            }
            if steps.is_empty() {
                return Err(unsupported("an anonymous traversal needs a step"));
            }
            let out = steps
                .iter()
                .map(|step| instruction(step, depth + 1))
                .collect::<Result<Vec<_>, _>>()?
                .join(".");
            Ok(out)
        }
        "g:Lambda" => Err(unsupported("lambdas are not accepted")),
        other => Err(unsupported(format!("argument type {other}"))),
    }
}

/// `P.gt(5)`, `P.within(1, 2)`, `P.and(P.gt(1), P.lt(5))`, `P.not(...)`.
fn predicate(inner: &Json, depth: usize) -> Result<String, LanguageError> {
    let name = inner
        .get("predicate")
        .and_then(Json::as_str)
        .ok_or_else(|| unsupported("a predicate needs a name"))?;
    if !name.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(unsupported(format!("invalid predicate `{name}`")));
    }
    let value = inner
        .get("value")
        .ok_or_else(|| unsupported("a predicate needs a value"))?;
    let args = match (name, value) {
        // Composite predicates carry their operands as a list of predicates.
        ("and" | "or", Json::Array(operands)) => operands
            .iter()
            .map(|operand| argument(operand, depth + 1))
            .collect::<Result<Vec<_>, _>>()?,
        ("within" | "without" | "between" | "inside" | "outside", _) => {
            let items = match typed(value) {
                Some(("g:List" | "g:Set", Json::Array(items))) => items.clone(),
                _ => match value {
                    Json::Array(items) => items.clone(),
                    other => vec![other.clone()],
                },
            };
            items
                .iter()
                .map(|item| argument(item, depth + 1))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => vec![argument(value, depth + 1)?],
    };
    Ok(format!("P.{name}({})", args.join(", ")))
}

fn enum_value(prefix: &str, inner: &Json) -> Result<String, LanguageError> {
    let value = inner
        .as_str()
        .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_alphabetic()))
        .ok_or_else(|| unsupported("an enum value is a name"))?;
    Ok(if prefix.is_empty() {
        value.to_owned()
    } else {
        format!("{prefix}.{value}")
    })
}

fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for c in text.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn translates_driver_bytecode_to_text() {
        // gremlin-python: g.V().hasLabel('person').has('age', P.gt(30))
        //   .out('knows').order().by('name', Order.desc).limit(2).values('name')
        let bytecode = json!({"@type": "g:Bytecode", "@value": {"step": [
            ["V"],
            ["hasLabel", "person"],
            ["has", "age", {"@type": "g:P", "@value": {"predicate": "gt", "value": {"@type": "g:Int32", "@value": 30}}}],
            ["out", "knows"],
            ["order"],
            ["by", "name", {"@type": "g:Order", "@value": "desc"}],
            ["limit", {"@type": "g:Int64", "@value": 2}],
            ["values", "name"]
        ]}});
        assert_eq!(
            bytecode_to_gremlin(&bytecode).unwrap(),
            "g.V().hasLabel('person').has('age', P.gt(30)).out('knows').order().by('name', desc).limit(2).values('name')"
        );
    }

    #[test]
    fn translates_anonymous_traversals_predicates_and_enums() {
        let bytecode = json!({"step": [
            ["V"],
            ["where", {"@type": "g:Bytecode", "@value": {"step": [["out", "created"]]}}],
            ["has", "name", {"@type": "g:P", "@value": {"predicate": "within", "value": {"@type": "g:List", "@value": ["a", "b'c"]}}}],
            ["has", "age", {"@type": "g:P", "@value": {"predicate": "and", "value": [
                {"@type": "g:P", "@value": {"predicate": "gte", "value": 1}},
                {"@type": "g:P", "@value": {"predicate": "lt", "value": 9}}
            ]}}],
            ["values", {"@type": "g:T", "@value": "label"}]
        ]});
        assert_eq!(
            bytecode_to_gremlin(&bytecode).unwrap(),
            "g.V().where(out('created')).has('name', P.within('a', 'b\\'c')).has('age', P.and(P.gte(1), P.lt(9))).values(T.label)"
        );
    }

    #[test]
    fn refuses_lambdas_strategies_and_unknown_types() {
        let lambda = json!({"step": [["V"], ["map", {"@type": "g:Lambda", "@value": {"script": "it.get()"}}]]});
        assert!(bytecode_to_gremlin(&lambda).is_err());
        let strategy = json!({"source": [["withStrategies", "x"]], "step": [["V"]]});
        assert!(bytecode_to_gremlin(&strategy).is_err());
        let unknown = json!({"step": [["V", {"@type": "g:Vertex", "@value": {}}]]});
        assert!(bytecode_to_gremlin(&unknown).is_err());
        let injected = json!({"step": [["V"], ["out').drop("]]});
        assert!(
            bytecode_to_gremlin(&injected).is_err(),
            "step names are identifiers"
        );
    }
}
