//! The `serde` feature: what a failure and a secret look like once serialised.
//!
//! decision-3 keeps output application-owned, so the library writes no JSON of its own.
//! What it owns is the shape an application serialises — a failure is a kind and a
//! message, and a secret is the redaction marker — and these tests read that shape back
//! through `serde_json`, which is what an application would build a JSON body with.
#![cfg(feature = "serde")]

use clifacet::secret::{REDACTED, Secret};
use clifacet::{Failure, FailureKind};
use rstest::rstest;
use serde_json::json;

#[rstest]
#[case::validation(Failure::validation("--repeat must be between 1 and 8"), "validation")]
#[case::execution(Failure::execution("the greeting could not be written"), "execution")]
#[case::cancelled(Failure::cancelled("cancelled before anything ran"), "cancelled")]
fn a_failure_serialises_as_its_kind_and_its_message(#[case] failure: Failure, #[case] kind: &str) {
    let body = serde_json::to_value(&failure).expect("a failure always serialises");

    assert_eq!(body, json!({ "kind": kind, "message": failure.message() }));
}

#[rstest]
#[case::validation(FailureKind::Validation, "validation")]
#[case::execution(FailureKind::Execution, "execution")]
#[case::cancelled(FailureKind::Cancelled, "cancelled")]
fn a_failure_kind_serialises_as_its_lowercase_name(#[case] kind: FailureKind, #[case] name: &str) {
    assert_eq!(
        serde_json::to_value(kind).expect("a kind serialises"),
        json!(name)
    );
}

#[test]
fn the_body_lists_the_kind_before_the_message() {
    let body = serde_json::to_string(&Failure::validation("blank")).expect("serialises");

    assert_eq!(body, r#"{"kind":"validation","message":"blank"}"#);
}

#[rstest]
#[case::a_token("sk-live-1234")]
#[case::empty("")]
#[case::quotes_and_escapes("\"},\"kind\":\"leak\\")]
#[case::non_ascii("pässwörd-🔑")]
#[case::the_marker_itself(REDACTED)]
fn a_secret_serialises_as_the_marker_whatever_it_holds(#[case] value: &str) {
    let serialised = serde_json::to_string(&Secret::new(value.to_owned())).expect("serialises");

    assert_eq!(serialised, format!("\"{REDACTED}\""));
}

#[test]
fn a_secret_inside_a_serialised_value_does_not_reach_the_output() {
    let token = "sk-live-1234";
    let body = serde_json::to_string(&json!({
        "user": "ada",
        "token": serde_json::to_value(Secret::new(token.to_owned())).expect("serialises"),
    }))
    .expect("serialises");

    assert!(!body.contains(token), "the credential leaked: {body}");
    assert!(body.contains(REDACTED), "{body}");
}
