//! The public GQL profile is machine-readable. Keep its advertised feature
//! identifiers aligned with the server capability and keep its evidence and
//! external-corpus boundary explicit.

use serde_json::Value;

const PROFILE: &str = include_str!("../conformance/gql-profile-v1.json");

#[test]
fn gql_profile_manifest_is_complete_and_traceable() {
    let profile: Value = serde_json::from_str(PROFILE).expect("valid GQL profile JSON");
    assert_eq!(profile["profileId"], "rushdb-gql-property-graph-v1");
    assert_eq!(profile["language"], "ISO/IEC 39075 GQL");
    assert_eq!(profile["status"], "release_candidate");
    assert_eq!(profile["externalCorpusGate"], "passed");
    assert_eq!(profile["externalCorpus"]["unclassified"], 0);

    let accepted = profile["acceptedFeatures"]
        .as_array()
        .expect("accepted feature list");
    for required in [
        "match",
        "parameters",
        "optionalMatch",
        "aggregation",
        "unwind",
        "createNode",
        "createRelationship",
        "atomicStatementCommit",
    ] {
        assert!(
            accepted.iter().any(|value| value == required),
            "profile omits {required}"
        );
    }
    let excluded = profile["excludedFeatures"]
        .as_array()
        .expect("excluded feature list");
    assert!(excluded
        .iter()
        .any(|value| value == "fullIsoGqlConformance"));

    let sources = profile["sources"].as_array().expect("source list");
    assert!(sources.iter().any(|source| source["kind"] == "standard"));
    let workload = sources
        .iter()
        .find(|source| source["kind"] == "public-workload-input")
        .expect("pinned public workload source");
    assert!(workload["commit"]
        .as_str()
        .is_some_and(|commit| commit.len() == 40));
    assert!(profile["evidence"]
        .as_array()
        .is_some_and(|evidence| !evidence.is_empty()));
}
