// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the operator makes, it then holds as applied server-side: right after it makes an
//! object, the fields it wrote are recorded under an `Apply` entry of its field manager rather
//! than the `Update` entry a create leaves, as Kubernetes' own `csaupgrade` does. A later apply
//! that leaves a field out then takes it out, and one that leaves a field another manager added is
//! followed by a write of the whole spec.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{FieldsV1, ManagedFieldsEntry};
use purlis_cluster_operator::{FIELD_MANAGER, managed_as_applied};
use serde_json::json;

fn entry(manager: &str, operation: &str, subresource: Option<&str>) -> ManagedFieldsEntry {
    ManagedFieldsEntry {
        api_version: Some("v1".into()),
        fields_type: Some("FieldsV1".into()),
        fields_v1: Some(FieldsV1(json!({ "f:spec": { "f:replicas": {} } }))),
        manager: Some(manager.into()),
        operation: Some(operation.into()),
        subresource: subresource.map(Into::into),
        time: None,
    }
}

#[test]
fn the_operators_update_entry_from_its_create_becomes_an_apply_entry_with_the_same_fields() {
    let upstream = entry("agent-sandbox-controller", "Update", None);
    let made = [entry(FIELD_MANAGER, "Update", None), upstream.clone()];
    let held = managed_as_applied(&made).expect("rewritten");
    let mut wanted = made[0].clone();
    wanted.operation = Some("Apply".into());
    assert_eq!(held, vec![wanted, upstream]);
}

#[test]
fn fields_already_held_as_applied_or_held_by_others_are_left_as_they_are() {
    // Already applied: nothing to write.
    assert_eq!(
        managed_as_applied(&[entry(FIELD_MANAGER, "Apply", None)]),
        None
    );
    // Others' entries, and the operator's on a subresource such as status, are not its object's.
    assert_eq!(
        managed_as_applied(&[
            entry("kubectl", "Update", None),
            entry(FIELD_MANAGER, "Update", Some("status")),
        ]),
        None
    );
    assert_eq!(managed_as_applied(&[]), None);
}

/// **What the operator applies says what it wrote, and nothing more**: a server-side apply
/// takes back the fields the operator owns, but leaves a field another manager added. Where the
/// object as applied still does not say what it should, its whole spec is written over, only at
/// the version it was read at, so a change made since is looked at again first.
#[test]
fn a_field_another_manager_added_is_written_over_with_the_whole_spec() {
    use purlis_cluster_operator::{person, spec_held_whole};
    let wanted = person::egress_denied("purlis-runners-0123456789abcdef", vec![]);
    let mut applied = wanted.clone();
    applied.metadata.resource_version = Some("42".into());
    applied
        .spec
        .as_mut()
        .expect("a spec")
        .pod_selector
        .as_mut()
        .expect("a selector")
        .match_labels = Some([("team".to_owned(), "a".to_owned())].into());
    let says = |held: &k8s_openapi::api::networking::v1::NetworkPolicy| held.spec == wanted.spec;
    assert_eq!(
        spec_held_whole(&applied, &wanted, says),
        Some(json!([
            { "op": "test", "path": "/metadata/resourceVersion", "value": "42" },
            {
                "op": "replace",
                "path": "/spec",
                "value": serde_json::to_value(&wanted).expect("JSON")["spec"],
            },
        ]))
    );
}

#[test]
fn what_says_what_it_should_once_applied_is_left_as_it_is() {
    use purlis_cluster_operator::{person, spec_held_whole};
    let wanted = person::egress_denied("purlis-runners-0123456789abcdef", vec![]);
    let mut applied = wanted.clone();
    applied.metadata.resource_version = Some("42".into());
    assert_eq!(
        spec_held_whole(&applied, &wanted, |held| held.spec == wanted.spec),
        None
    );
}
