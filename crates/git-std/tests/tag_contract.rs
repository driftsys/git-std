mod tag_support;
use tag_support::*;

#[test]
fn tag_json_uses_the_public_status_and_schema() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let value = ok(tag(r.path(), &["HEAD", "--dry-run", "--format", "json"]));
    assert_eq!(value["status"], "success");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/v1/cli/tag.schema.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&value));
    let out = tag(r.path(), &["invalid-commit", "--format", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let refusal: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(validator.is_valid(&refusal));
}
