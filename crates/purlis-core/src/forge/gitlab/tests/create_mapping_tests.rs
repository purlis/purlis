// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use serde_json::json;

use crate::forge::Forge;
use crate::forge::Kind;
use crate::forge::backend::{Caller, ForgeRef, NewWorkItem};
use crate::forge::recorded::Recorded;
use crate::work::Milestone;

fn created(answer: serde_json::Value) -> crate::work::WorkItem {
    let call = json!({"endpoint": {"rest": {"method": "POST", "path": "projects/acme%2Fapi/issues"}},
                          "fields": [{"text": ["title", "Port the picker"]},
                                     {"text": ["description", "Why."]}]});
    let text = json!({"source": "GitLab REST API v4, issues: New issue",
                          "exchanges": [{"call": call, "reply": {"code": 0, "out": answer.to_string()}}]});
    let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
    let backend = Forge::default_of(Kind::GitLab).backend_over(recorded);
    let new = NewWorkItem {
        title: "Port the picker".into(),
        body: "Why.".into(),
        workspace_label: None,
    };
    backend
        .create(&Caller::command(), "acme/api", &new)
        .unwrap()
}

fn issue() -> serde_json::Value {
    json!({"id": 84012, "iid": 12, "project_id": 3, "title": "Port the picker",
               "state": "opened", "labels": [], "web_url": "https://gitlab.com/acme/api/-/issues/12",
               "references": {"full": "acme/api#12"}})
}

#[test]
fn a_milestone_in_gitlabs_answer_is_mapped_with_its_id_and_due_day() {
    let mut answer = issue();
    answer["milestone"] = json!({"id": 501, "iid": 1, "title": "v1", "state": "active",
                                     "due_date": "2026-10-31"});
    assert_eq!(
        created(answer).milestone,
        Some(Milestone {
            forge_ref: Some(ForgeRef("501".into())),
            title: "v1".into(),
            due: chrono::NaiveDate::from_ymd_opt(2026, 10, 31),
        })
    );
}

#[test]
fn no_milestone_or_no_due_date_in_gitlabs_answer_maps_to_none() {
    assert_eq!(created(issue()).milestone, None);
    let mut answer = issue();
    answer["milestone"] = json!({"id": 501, "iid": 1, "title": "v1", "state": "active",
                                     "due_date": null});
    assert_eq!(created(answer).milestone.unwrap().due, None);
}
