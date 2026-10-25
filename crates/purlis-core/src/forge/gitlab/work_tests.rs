// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::forge::backend::{Asker, Fixed};
use crate::forge::recorded::Recorded;
use crate::forge::{Forge, Kind};

/// The GraphQL documents GitLab is sent, word for word as recorded: a recording names the
/// document's text, never the constant that builds it, so a change to what is sent fails here
/// instead of passing by construction. `SET_ITERATION` is an Enterprise Edition document
/// (`issueSetIteration`, iterations being Premium), which no Community Edition schema can check
/// (ADR 0070, FW-2b amendment); `CHILDREN` and `SET_PARENT` stay constants until #1031 vendors
/// the CE schema they would be checked against.
const RECORDED_CHILDREN: &str = "query($id:WorkItemID!,$after:String){workItem(id:$id){widgets{__typename ... on WorkItemWidgetHierarchy{children(first:100,after:$after){pageInfo{hasNextPage endCursor} nodes{id iid title state webUrl workItemType{name}}}}}}}";
const RECORDED_SET_PARENT: &str = "mutation($id:WorkItemID!,$parent:WorkItemID){workItemUpdate(input:{id:$id,hierarchyWidget:{parentId:$parent}}){errors}}";
/// The epic documents (#1032): an epic is a work item of type Epic, which only GitLab's
/// Enterprise Edition has, so these stay constants too.
const RECORDED_EPICS: &str = "query($path:ID!,$state:IssuableState,$after:String){group(fullPath:$path){workItems(types:[EPIC],state:$state,first:100,after:$after){pageInfo{hasNextPage endCursor} nodes{id iid title description state webUrl widgets{__typename ... on WorkItemWidgetLabels{labels{nodes{title}}}}}}}}";
const RECORDED_EPIC_TYPE: &str =
    "query($path:ID!){namespace(fullPath:$path){workItemTypes(name:EPIC){nodes{id}}}}";
const RECORDED_CREATE_WORK_ITEM: &str = "mutation($path:ID!,$type:WorkItemsTypeID!,$title:String!,$description:String!){workItemCreate(input:{namespacePath:$path,workItemTypeId:$type,title:$title,descriptionWidget:{description:$description}}){errors workItem{id iid title description state webUrl widgets{__typename ... on WorkItemWidgetLabels{labels{nodes{title}}}}}}}";
const RECORDED_SET_ITERATION: &str = "mutation($path:ID!,$iid:String!,$iteration:IterationID){issueSetIteration(input:{projectPath:$path,iid:$iid,iterationId:$iteration}){errors issue{iteration{id}}}}";

/// GitLab over these recorded exchanges, and the recording to check afterwards.
fn over(exchanges: Value) -> (GitLab, Arc<Recorded>) {
    let text = json!({"source": "GitLab 19.4 REST API docs (issues.md, issue_links.md, \
                                 milestones.md, epics.md, iterations.md, boards.md) and its \
                                 GraphQL reference (workItem, workItemUpdate, issueSetIteration)",
                      "exchanges": exchanges});
    let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
    let gitlab = GitLab(Asker {
        forge: Forge::default_of(Kind::GitLab),
        transports: Arc::new(Fixed(recorded.clone())),
    });
    (gitlab, recorded)
}

fn spent(recorded: &Recorded) {
    assert_eq!(recorded.unspent(), Vec::new(), "recorded and never asked");
}

fn rest(method: Option<&str>, path: &str, fields: Value, out: Value) -> Value {
    json!({"call": {"endpoint": {"rest": {"method": method, "path": path}}, "fields": fields},
           "reply": {"code": 0, "out": out.to_string()}})
}

fn gql(query: &str, variables: Value, out: Value) -> Value {
    let mut fields = vec![json!({"text": ["query", query]})];
    fields.extend(variables.as_array().unwrap().iter().cloned());
    json!({"call": {"endpoint": "graphql", "fields": fields},
           "reply": {"code": 0, "out": out.to_string()}})
}

fn issue(iid: u64, title: &str) -> Value {
    json!({"id": 84000 + iid, "iid": iid, "project_id": 3, "title": title,
           "description": null, "state": "opened",
           "web_url": format!("https://gitlab.com/acme/api/-/issues/{iid}"),
           "labels": ["bug"], "milestone": null, "iteration": null, "issue_type": "issue"})
}

fn me() -> Caller {
    Caller::command()
}

#[test]
fn issues_are_listed_page_by_page_and_written_with_their_fields_as_literals() {
    let page: Vec<Value> = (1..=100).map(|n| issue(n, "one")).collect();
    let (gitlab, recorded) = over(json!([
        rest(
            None,
            "projects/acme%2Fapi/issues?state=opened&per_page=100&page=1",
            json!([]),
            json!(page)
        ),
        rest(
            None,
            "projects/acme%2Fapi/issues?state=opened&per_page=100&page=2",
            json!([]),
            json!([issue(101, "last")])
        ),
        rest(
            None,
            "projects/acme%2Fapi/issues/7",
            json!([]),
            issue(7, "seven")
        ),
        rest(
            Some("POST"),
            "projects/acme%2Fapi/issues",
            json!([{"text": ["title", "@not-a-file"]}, {"text": ["description", "why"]},
                   {"text": ["labels", "bug,charter::ws::alpha"]},
                   {"typed": ["milestone_id", "5"]}, {"text": ["issue_type", "incident"]}]),
            issue(8, "@not-a-file")
        ),
        rest(
            Some("PUT"),
            "projects/acme%2Fapi/issues/8",
            json!([{"text": ["state_event", "close"]}, {"text": ["labels", ""]}]),
            issue(8, "@not-a-file")
        ),
    ]));
    let open = gitlab.issues(&me(), "acme/api", "opened").unwrap();
    assert_eq!(open.len(), 101);
    assert_eq!(open[100].title, "last");
    let seven = gitlab.issue(&me(), "acme/api", 7).unwrap();
    assert_eq!(
        (seven.iid, seven.id, seven.labels.as_slice()),
        (7, 84007, &["bug".to_string()][..])
    );
    assert_eq!(seven.forge_ref(), ForgeRef("84007".into()));
    assert_eq!(seven.work_item_id(), "gid://gitlab/WorkItem/84007");
    let made = gitlab
        .create_issue(
            &me(),
            "acme/api",
            &NewIssue {
                title: "@not-a-file".into(),
                description: Some("why".into()),
                labels: vec!["bug".into(), "charter::ws::alpha".into()],
                milestone_id: Some(5),
                issue_type: Some("incident".into()),
            },
        )
        .unwrap();
    assert_eq!(made.iid, 8);
    gitlab
        .update_issue(
            &me(),
            "acme/api",
            8,
            &IssueEdit {
                state_event: Some("close".into()),
                labels: Some(Vec::new()),
                ..IssueEdit::default()
            },
        )
        .unwrap();
    spent(&recorded);
}

#[test]
fn a_label_with_a_comma_is_refused_before_anything_is_sent() {
    let (gitlab, recorded) = over(json!([]));
    let refused = gitlab.create_issue(
        &me(),
        "acme/api",
        &NewIssue {
            title: "t".into(),
            labels: vec!["a,b".into()],
            ..NewIssue::default()
        },
    );
    assert!(refused.unwrap_err().to_string().contains("a,b"));
    spent(&recorded);
}

#[test]
fn blocking_links_are_read_and_written_by_iid_and_removed_by_link_id() {
    let mut blocker = issue(4, "blocker");
    blocker["issue_link_id"] = json!(91);
    blocker["link_type"] = json!("is_blocked_by");
    let mut related = issue(5, "related");
    related["issue_link_id"] = json!(92);
    related["link_type"] = json!("relates_to");
    let (gitlab, recorded) = over(json!([
        rest(
            None,
            "projects/acme%2Fapi/issues/1/links",
            json!([]),
            json!([blocker, related])
        ),
        rest(
            Some("POST"),
            "projects/acme%2Fapi/issues/1/links",
            json!([{"text": ["target_project_id", "acme/web"]},
                   {"typed": ["target_issue_iid", "4"]},
                   {"text": ["link_type", "is_blocked_by"]}]),
            json!({"source_issue": issue(1, "parent"), "target_issue": issue(4, "blocker"),
                   "link_type": "is_blocked_by"})
        ),
        rest(
            Some("DELETE"),
            "projects/acme%2Fapi/issues/1/links/91",
            json!([]),
            json!({"source_issue": issue(1, "parent"), "target_issue": issue(4, "blocker"),
                   "link_type": "is_blocked_by"})
        ),
    ]));
    let blockers = gitlab.blocked_by(&me(), "acme/api", 1).unwrap();
    assert_eq!(
        blockers.len(),
        1,
        "only the blocking link, not the related one"
    );
    assert_eq!((blockers[0].issue.iid, blockers[0].link_id), (4, 91));
    gitlab
        .add_blocked_by(&me(), "acme/api", 1, "acme/web", 4)
        .unwrap();
    gitlab.remove_link(&me(), "acme/api", 1, 91).unwrap();
    spent(&recorded);
}

#[test]
fn child_items_are_read_and_a_parent_is_set_and_cleared_through_work_items() {
    let (gitlab, recorded) = over(json!([
        gql(
            RECORDED_CHILDREN,
            json!([{"text": ["id", "gid://gitlab/WorkItem/84001"]}]),
            json!({"data": {"workItem": {"widgets": [
                {"__typename": "WorkItemWidgetDescription"},
                {"__typename": "WorkItemWidgetHierarchy", "children": {
                    "pageInfo": {"hasNextPage": true, "endCursor": "c1"},
                    "nodes": [{"id": "gid://gitlab/WorkItem/84002", "iid": "2",
                               "title": "child", "state": "OPEN",
                               "webUrl": "https://gitlab.com/acme/api/-/work_items/2",
                               "workItemType": {"name": "Task"}}]}}]}}})
        ),
        gql(
            RECORDED_CHILDREN,
            json!([{"text": ["id", "gid://gitlab/WorkItem/84001"]},
                   {"text": ["after", "c1"]}]),
            json!({"data": {"workItem": {"widgets": [
                {"__typename": "WorkItemWidgetHierarchy", "children": {
                    "pageInfo": {"hasNextPage": false, "endCursor": null},
                    "nodes": [{"id": "gid://gitlab/WorkItem/84003", "iid": "3",
                               "title": "second", "state": "CLOSED",
                               "webUrl": "https://gitlab.com/acme/api/-/work_items/3",
                               "workItemType": {"name": "Issue"}}]}}]}}})
        ),
        gql(
            RECORDED_SET_PARENT,
            json!([{"text": ["id", "gid://gitlab/WorkItem/84002"]},
                   {"text": ["parent", "gid://gitlab/WorkItem/84001"]}]),
            json!({"data": {"workItemUpdate": {"errors": []}}})
        ),
        gql(
            RECORDED_SET_PARENT,
            json!([{"text": ["id", "gid://gitlab/WorkItem/84002"]},
                   {"typed": ["parent", "null"]}]),
            json!({"data": {"workItemUpdate": {"errors": []}}})
        ),
    ]));
    let children = gitlab
        .children(&me(), "gid://gitlab/WorkItem/84001")
        .unwrap();
    assert_eq!(
        children
            .iter()
            .map(|c| (c.iid.as_str(), c.kind.as_str()))
            .collect::<Vec<_>>(),
        [("2", "Task"), ("3", "Issue")]
    );
    gitlab
        .set_parent(
            &me(),
            "gid://gitlab/WorkItem/84002",
            Some("gid://gitlab/WorkItem/84001"),
        )
        .unwrap();
    gitlab
        .set_parent(&me(), "gid://gitlab/WorkItem/84002", None)
        .unwrap();
    spent(&recorded);
}

#[test]
fn a_mutation_gitlab_refuses_in_its_payload_is_an_error_in_its_words() {
    let (gitlab, recorded) = over(json!([gql(
        RECORDED_SET_PARENT,
        json!([{"text": ["id", "gid://gitlab/WorkItem/84002"]},
               {"text": ["parent", "gid://gitlab/WorkItem/84001"]}]),
        json!({"data": {"workItemUpdate": {"errors": ["No matching work item found"]}}})
    )]));
    let refused = gitlab.set_parent(
        &me(),
        "gid://gitlab/WorkItem/84002",
        Some("gid://gitlab/WorkItem/84001"),
    );
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains("No matching work item found")
    );
    spent(&recorded);
}

#[test]
fn a_payload_refusal_reaches_the_window_as_one_line_and_capped() {
    let long = "x".repeat(5_000);
    let (gitlab, recorded) = over(json!([gql(
        RECORDED_SET_PARENT,
        json!([{"text": ["id", "gid://gitlab/WorkItem/84002"]},
               {"text": ["parent", "gid://gitlab/WorkItem/84001"]}]),
        json!({"data": {"workItemUpdate": {"errors": ["first\nline\u{202e}reversed", long]}}})
    )]));
    let said = gitlab
        .set_parent(
            &me(),
            "gid://gitlab/WorkItem/84002",
            Some("gid://gitlab/WorkItem/84001"),
        )
        .unwrap_err()
        .to_string();
    assert!(said.contains("first⟦U+000A⟧line⟦U+202E⟧reversed"), "{said}");
    assert!(!said.contains('\n') && !said.contains('\u{202e}'), "{said}");
    assert!(
        said.chars().count() < 1_000,
        "{} characters",
        said.chars().count()
    );
    spent(&recorded);
}

#[test]
fn milestones_are_read_whatever_their_state_and_one_is_made() {
    let (gitlab, recorded) = over(json!([
        rest(
            None,
            "projects/acme%2Fapi/milestones?per_page=100&page=1",
            json!([]),
            json!([{"id": 50, "iid": 1, "title": "M1", "state": "closed",
                    "due_date": "2026-11-01", "start_date": null}])
        ),
        rest(
            Some("POST"),
            "projects/acme%2Fapi/milestones",
            json!([{"text": ["title", "M2"]}, {"text": ["due_date", "2026-12-01"]}]),
            json!({"id": 51, "iid": 2, "title": "M2", "state": "active"})
        ),
    ]));
    let all = gitlab.milestones(&me(), "acme/api").unwrap();
    assert_eq!(
        (all[0].title.as_str(), all[0].state.as_str()),
        ("M1", "closed")
    );
    let made = gitlab
        .create_milestone(
            &me(),
            "acme/api",
            &NewMilestone {
                title: "M2".into(),
                due_date: Some("2026-12-01".into()),
                ..NewMilestone::default()
            },
        )
        .unwrap();
    assert_eq!((made.id, made.iid), (51, 2));
    spent(&recorded);
}

#[test]
fn a_groups_epics_are_read_made_and_given_an_issue_as_work_items() {
    let epic = |iid: u64, title: &str, labels: Value| {
        json!({"id": format!("gid://gitlab/WorkItem/{}", 900 + iid), "iid": iid.to_string(),
               "title": title, "description": null, "state": "OPEN",
               "webUrl": format!("https://gitlab.com/groups/acme/platform/-/work_items/{iid}"),
               "widgets": [{"__typename": "WorkItemWidgetDescription"},
                           {"__typename": "WorkItemWidgetLabels",
                            "labels": {"nodes": labels}}]})
    };
    let page = |nodes: Value, next: Option<&str>| {
        json!({"data": {"group": {"workItems": {
            "pageInfo": {"hasNextPage": next.is_some(), "endCursor": next},
            "nodes": nodes}}}})
    };
    let (gitlab, recorded) = over(json!([
        gql(
            RECORDED_EPICS,
            json!([{"text": ["path", "acme/platform"]}, {"text": ["state", "opened"]}]),
            page(
                json!([epic(1, "Q4", json!([{"title": "roadmap"}]))]),
                Some("c1")
            )
        ),
        gql(
            RECORDED_EPICS,
            json!([{"text": ["path", "acme/platform"]}, {"text": ["state", "opened"]},
                   {"text": ["after", "c1"]}]),
            page(json!([epic(3, "Q1", json!([]))]), None)
        ),
        gql(
            RECORDED_EPIC_TYPE,
            json!([{"text": ["path", "acme/platform"]}]),
            json!({"data": {"namespace": {"workItemTypes": {"nodes": [
                {"id": "gid://gitlab/WorkItems::Type/8"}]}}}})
        ),
        gql(
            RECORDED_CREATE_WORK_ITEM,
            json!([{"text": ["path", "acme/platform"]},
                   {"text": ["type", "gid://gitlab/WorkItems::Type/8"]},
                   {"text": ["title", "Port"]}, {"text": ["description", "why"]}]),
            json!({"data": {"workItemCreate": {"errors": [],
                   "workItem": epic(2, "Port", json!([]))}}})
        ),
        gql(
            RECORDED_SET_PARENT,
            json!([{"text": ["id", "gid://gitlab/WorkItem/84007"]},
                   {"text": ["parent", "gid://gitlab/WorkItem/902"]}]),
            json!({"data": {"workItemUpdate": {"errors": []}}})
        ),
    ]));
    let epics = gitlab.epics(&me(), "acme/platform", "opened").unwrap();
    let read: Vec<(u64, &str, &[String])> = epics
        .iter()
        .map(|e| (e.iid, e.title.as_str(), e.labels.as_slice()))
        .collect();
    assert_eq!(
        read,
        [(1, "Q4", &["roadmap".to_string()][..]), (3, "Q1", &[][..])],
        "every page, each epic with its labels"
    );
    assert_eq!(epics[0].id, "gid://gitlab/WorkItem/901");
    let made = gitlab
        .create_epic(
            &me(),
            "acme/platform",
            &NewEpic {
                title: "Port".into(),
                description: Some("why".into()),
            },
        )
        .unwrap();
    assert_eq!(
        (made.iid, made.id.as_str()),
        (2, "gid://gitlab/WorkItem/902")
    );
    let seven: Issue = serde_json::from_value(issue(7, "seven")).unwrap();
    gitlab.add_to_epic(&me(), &made, &seven).unwrap();
    spent(&recorded);
}

#[test]
fn a_group_whose_gitlab_has_no_epic_type_makes_no_epic() {
    let (gitlab, recorded) = over(json!([gql(
        RECORDED_EPIC_TYPE,
        json!([{"text": ["path", "acme"]}]),
        json!({"data": {"namespace": {"workItemTypes": {"nodes": []}}}})
    )]));
    let refused = gitlab
        .create_epic(
            &me(),
            "acme",
            &NewEpic {
                title: "Port".into(),
                description: None,
            },
        )
        .unwrap_err();
    assert!(refused.said().contains("has no epics"), "{refused}");
    spent(&recorded);
}

#[test]
fn no_epic_is_asked_of_v4s_epics_endpoints() {
    let source = include_str!("work.rs");
    assert!(
        !source.contains("/epics"),
        "GitLab deprecated v4's epics API in 17.0; epics are work items (#1032)"
    );
}

#[test]
fn a_groups_iterations_are_read_and_an_issue_is_set_to_one() {
    let (gitlab, recorded) = over(json!([
        rest(
            None,
            "groups/acme/iterations?state=opened&include_ancestors=true&per_page=100&page=1",
            json!([]),
            json!([{"id": 53, "iid": 13, "sequence": 1, "group_id": 6, "title": null,
                    "state": 2, "start_date": "2026-10-01", "due_date": "2026-10-14",
                    "web_url": "https://gitlab.com/groups/acme/-/iterations/53"}])
        ),
        gql(
            RECORDED_SET_ITERATION,
            json!([{"text": ["path", "acme/api"]}, {"text": ["iid", "7"]},
                   {"text": ["iteration", "gid://gitlab/Iteration/53"]}]),
            json!({"data": {"issueSetIteration": {"errors": [],
                   "issue": {"iteration": {"id": "gid://gitlab/Iteration/53"}}}}})
        ),
    ]));
    let iterations = gitlab.iterations(&me(), "acme", "opened").unwrap();
    assert_eq!(
        (iterations[0].id, iterations[0].start_date.as_str()),
        (53, "2026-10-01")
    );
    assert_eq!(iterations[0].global_id(), "gid://gitlab/Iteration/53");
    gitlab
        .set_iteration(&me(), "acme/api", 7, Some("gid://gitlab/Iteration/53"))
        .unwrap();
    spent(&recorded);
}

#[test]
fn a_boards_lists_are_read_and_an_issue_is_moved_onto_one_by_its_label() {
    let (gitlab, recorded) = over(json!([
        rest(
            None,
            "projects/acme%2Fapi/boards?per_page=100&page=1",
            json!([]),
            json!([{"id": 2, "name": "Development", "lists": [
                {"id": 21, "label": {"id": 7, "name": "Doing"}, "position": 0},
                {"id": 22, "label": null, "position": 1}]}])
        ),
        rest(
            Some("PUT"),
            "projects/acme%2Fapi/issues/7",
            json!([{"text": ["add_labels", "Doing"]}]),
            issue(7, "seven")
        ),
    ]));
    let boards = gitlab.boards(&me(), "acme/api").unwrap();
    assert_eq!(boards[0].name, "Development");
    let doing = &boards[0].lists[0];
    assert_eq!(doing.label.as_deref(), Some("Doing"));
    assert_eq!(boards[0].lists[1].label, None, "a list with no label");
    gitlab.put_on_list(&me(), "acme/api", 7, doing).unwrap();
    let refused = gitlab.put_on_list(&me(), "acme/api", 7, &boards[0].lists[1]);
    assert!(
        refused.is_err(),
        "a list with no label holds nothing purlis can move onto it"
    );
    spent(&recorded);
}

#[test]
fn a_repos_pipelines_are_read_at_a_ref() {
    let (gitlab, recorded) = over(json!([rest(
        None,
        "projects/acme%2Fapi/pipelines?ref=feature%2Fx&per_page=100&page=1",
        json!([]),
        json!([{"id": 77, "iid": 12, "sha": "6dcb09b5", "ref": "feature/x",
                "status": "success", "web_url": "https://gitlab.com/acme/api/-/pipelines/77"}])
    )]));
    let pipelines = gitlab.pipelines(&me(), "acme/api", "feature/x").unwrap();
    assert_eq!(
        (pipelines[0].id, pipelines[0].status.as_str()),
        (77, "success")
    );
    spent(&recorded);
}
