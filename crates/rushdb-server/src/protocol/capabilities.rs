//! Language and feature capability advertisement.

use rushdb_protocol::{LanguageSet, QueryLanguageCapability};

#[expect(
    clippy::too_many_lines,
    reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
)]
pub(crate) fn query_language_capabilities(enabled: LanguageSet) -> Vec<QueryLanguageCapability> {
    use rushdb_protocol::{
        LanguageStatus, ParameterStyle, QueryLanguageCapability, StatementAccess,
        StatementLanguage, StatementTransport,
    };
    use ParameterStyle::{Named, Positional};
    use StatementAccess::{ReadOnly, ReadWrite};
    use StatementLanguage::{Cypher, Gql, Gremlin, Mongo, Sql, SqlPgq};
    use StatementTransport::{Pgwire, ProtocolJson};

    let all = vec![
        QueryLanguageCapability {
            language: Sql,
            status: LanguageStatus::Stable,
            profile: "PostgreSQL-dialect relational SQL (DataFusion 54.1)".into(),
            implementation: "Apache DataFusion".into(),
            access: ReadWrite,
            parameter_styles: vec![Positional, Named],
            transports: vec![ProtocolJson, Pgwire],
            mutation_transports: vec![ProtocolJson, Pgwire],
            features: language_features(&[
                "select",
                "filter",
                "projection",
                "join",
                "cte",
                "window",
                "aggregation",
                "order",
                "pagination",
                "preparedStatements",
                "relationshipTables",
                "insert",
                "update",
                "delete",
                "atomicStatementCommit",
            ]),
            limitations: language_features(&[
                "recordTablesOnlyForMutation",
                "existingCatalogColumnsOnly",
                "snapshotRowLimit:100000",
            ]),
        },
        QueryLanguageCapability {
            language: Gql,
            status: LanguageStatus::ReleaseCandidate,
            profile: "RushDB GQL property-graph profile v1 (ISO/IEC 39075 syntax subset)".into(),
            implementation: "Grafeo 0.5.42".into(),
            access: ReadWrite,
            parameter_styles: vec![Named],
            transports: vec![ProtocolJson],
            mutation_transports: vec![ProtocolJson],
            features: graph_language_features(),
            limitations: gql_language_limitations(),
        },
        QueryLanguageCapability {
            language: Cypher,
            status: LanguageStatus::ReleaseCandidate,
            profile: "openCypher property-graph profile".into(),
            implementation: "Grafeo 0.5.42".into(),
            access: ReadWrite,
            parameter_styles: vec![Named],
            transports: vec![ProtocolJson],
            mutation_transports: vec![ProtocolJson],
            features: graph_language_features(),
            limitations: cypher_language_limitations(),
        },
        QueryLanguageCapability {
            language: Gremlin,
            status: LanguageStatus::Stable,
            profile: "Apache TinkerPop Gremlin traversal profile (text and GraphSON 3 bytecode)"
                .into(),
            implementation: "Grafeo 0.5.42 / gremlin-rs grammar".into(),
            access: ReadWrite,
            parameter_styles: vec![Named],
            transports: vec![ProtocolJson],
            mutation_transports: vec![ProtocolJson],
            features: {
                let mut features = graph_language_features();
                features.push("graphSon3Bytecode".into());
                features
            },
            limitations: language_features(&[
                "singlePrimaryLabelPerRecord",
                "statementLabelMutationUnavailable",
                "graphBinaryNotExposed",
            ]),
        },
        QueryLanguageCapability {
            language: SqlPgq,
            status: LanguageStatus::Stable,
            profile: "SQL:2023 GRAPH_TABLE read profile".into(),
            implementation: "Grafeo 0.5.42".into(),
            access: ReadOnly,
            parameter_styles: vec![Named],
            transports: vec![ProtocolJson],
            mutation_transports: vec![],
            features: language_features(&["graphTable", "match", "filter", "projection", "order"]),
            limitations: language_features(&["readOnly"]),
        },
        QueryLanguageCapability {
            language: Mongo,
            status: LanguageStatus::Stable,
            profile: "MongoDB query language (find/aggregate + CRUD + index)".into(),
            implementation: "rushdb-mongodb".into(),
            access: ReadWrite,
            parameter_styles: vec![Named],
            transports: vec![ProtocolJson],
            mutation_transports: vec![ProtocolJson],
            features: language_features(&[
                "find",
                "filter",
                "projection",
                "sort",
                "pagination",
                "aggregate",
                "insert",
                "update",
                "delete",
                "upsert",
                "indexCreate",
                "indexDrop",
                "indexRevalidate",
            ]),
            limitations: language_features(&["noLookup", "noGeoNear", "noUnwind", "noProject"]),
        },
    ];
    all.into_iter()
        .filter(|capability| enabled.enabled(capability.language))
        .collect()
}

/// Feature-capability flags the engine advertises to clients (Core's Rust-mode
/// negotiation). Add a flag when a protocol feature is available; a client must
/// treat an absent flag as unsupported rather than guessing from version.
pub(crate) fn engine_capabilities(enabled: LanguageSet) -> Vec<String> {
    let mut flags = vec![
        "linkRecordsById".into(),
        "unlinkRecordsById".into(),
        "linkRecordsByKey".into(),
        "unlinkRecordsByKey".into(),
        "retypeRelations".into(),
        "retypeRecord".into(),
        "ingestDocuments".into(),
        "ingestCsv".into(),
        "exportCsv".into(),
        "purgeTenant".into(),
        "pointInTimeSnapshot".into(),
        "graphLanguageMutations".into(),
        // Catalog reads (CAT-2/3/4, REL-1): value search inside
        // `propertyValues` (`search`, `similar`, `domainOnMiss`),
        // `query.searchValues`, `describe.names`, `describe.schemaSummary`
        // and `describe.valueOverlap`.
        "valueSearch".into(),
        "searchValues".into(),
        "describeNames".into(),
        "schemaSummary".into(),
        "valueOverlap".into(),
    ];
    // Language enablement is discoverable: only enabled languages are
    // advertised. `searchquery` is always enabled and implied.
    flags.push("language:searchquery".into());
    let advertised = enabled.advertised();
    if !advertised.is_empty() {
        flags.push("statementLanguages".into());
    }
    for language in advertised {
        flags.push(format!("language:{}", language.as_str()));
    }
    if enabled.sql {
        flags.push("pgwire".into());
    }
    // Transport availability is discoverable: gRPC is transport-only (serves
    // every enabled language), Bolt serves Cypher.
    flags.push("grpc".into());
    if enabled.cypher {
        flags.push("bolt".into());
    }
    flags
}

fn language_features(features: &[&str]) -> Vec<String> {
    features
        .iter()
        .map(|feature| (*feature).to_owned())
        .collect()
}

fn graph_language_features() -> Vec<String> {
    language_features(&[
        "match",
        "filter",
        "projection",
        "parameters",
        "optionalMatch",
        "aggregation",
        "unwind",
        "order",
        "pagination",
        "createNode",
        "updateProperties",
        "deleteNode",
        "createRelationship",
        "deleteRelationship",
        "atomicStatementCommit",
    ])
}

fn graph_language_limitations() -> Vec<String> {
    language_features(&[
        "singlePrimaryLabelPerRecord",
        "statementLabelMutationUnavailable",
    ])
}

fn gql_language_limitations() -> Vec<String> {
    let mut limitations = graph_language_limitations();
    // The constructs gql-profile-v1 excludes (its LDBC corpus records why).
    limitations.extend(language_features(&[
        "noFullIsoGqlConformance",
        "noMatchAfterWithPipeline",
        "noNextComposition",
        "noOrderByInWith",
        "noShortestPath",
        "noPatternPredicates",
        "noListOrPatternComprehensions",
        "noLabelAlternation",
    ]));
    limitations
}

fn cypher_language_limitations() -> Vec<String> {
    let mut limitations = graph_language_limitations();
    // The exclusions of the openCypher TCK profile
    // (`conformance/opencypher-tck-v2024.2-profile.json`) and its known
    // Grafeo defects.
    limitations.extend(language_features(&[
        "noFullOpenCypherTckConformance",
        "noProcedures",
        "limitedTemporalTypes",
    ]));
    limitations
}
