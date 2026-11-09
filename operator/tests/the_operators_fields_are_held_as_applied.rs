// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the operator makes, it then holds as applied server-side: right after it makes an
//! object, the fields it wrote are recorded under an `Apply` entry of its field manager rather
//! than the `Update` entry a create leaves, as Kubernetes' own `csaupgrade` does. A later apply
//! that leaves a field out then takes it out.

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
