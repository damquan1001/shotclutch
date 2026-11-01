use serde_json::json;
use shotclutch_format::{FormatError, decode, encode};
use shotclutch_project::{Project, ProjectExtension};

#[test]
fn unknown_optional_extensions_survive_edit_save_reopen() {
    let mut p = Project::new("Extension fixture");
    let payload = json!({"revision": "9007199254740993", "nested": [null, {"unrecognized": true}], "reference": "clip_future"});
    p.extensions.insert("example.future-map".into(), ProjectExtension { version: 7, required: false, data: payload.clone() });
    let mut loaded = decode(&encode(&p, false)).unwrap().project;
    loaded.notes = "ordinary edit".into();
    let reopened = decode(&encode(&loaded, true)).unwrap().project;
    assert_eq!(reopened.extensions, p.extensions);
    assert_eq!(reopened.extensions["example.future-map"].data, payload);
    assert_eq!(reopened.notes, "ordinary edit");
}

#[test]
fn required_unknown_extensions_fail_before_editing() {
    let mut p = Project::new("Required extension");
    p.extensions.insert("example.required".into(), ProjectExtension { version: 2, required: true, data: json!({}) });
    let bytes = encode(&p, false);
    assert!(matches!(decode(&bytes), Err(FormatError::UnsupportedExtension { name, version: 2 }) if name == "example.required"));
    // Loading never rewrites or migrates the supplied bytes in place.
    assert_eq!(bytes, encode(&p, false));
}

#[test]
fn v15_upgrades_without_discarding_optional_extension_data() {
    let p = Project::new("v15");
    let mut doc: serde_json::Value = serde_json::from_slice(&encode(&p, false)).unwrap();
    doc["schema_version"] = json!(15);
    doc["project"]["extensions"] = json!({"example.future": {"version": 1, "data": {"keep": [1, 2, 3]}}});
    let loaded = decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert!(loaded.migrated());
    assert_eq!(loaded.schema_version, 15);
    assert_eq!(loaded.project.extensions["example.future"].data, json!({"keep": [1, 2, 3]}));
}

#[test]
fn zero_extension_version_is_rejected() {
    let mut p = Project::new("Invalid extension");
    p.extensions.insert("example.invalid".into(), ProjectExtension { version: 0, required: false, data: json!(null) });
    assert!(matches!(decode(&encode(&p, false)), Err(FormatError::Corrupt(_))));
}
