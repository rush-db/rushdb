//! Numbers parsed from client JSON keep every digit: a float written by a
//! client is stored and returned as the same `f64` (found in the LMPG
//! catalog audit: MIMIC-IV `inputevents.amount` came back one ulp off).

#[test]
fn parsed_floats_round_trip_exactly() {
    for text in [
        "18.916268423199654",
        "2.0612498074769974",
        "0.30000000000000004",
        "123456789.12345678",
        "5e-324",
        "1.7976931348623157e308",
    ] {
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let expected: f64 = text.parse().unwrap();
        assert_eq!(
            parsed.as_f64().unwrap().to_bits(),
            expected.to_bits(),
            "{text}"
        );
    }
}
