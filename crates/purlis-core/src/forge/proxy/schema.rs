// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The GraphQL types a chat may read, **field by field** (ADR 0128 D3e; review item M2).
//!
//! A query is walked from its root with the type each field returns, and a field that is not in
//! its type's table below is refused, whatever it would return. That replaces a list of fields to
//! refuse wherever they appear, which a connection's own `nodes` and `edges` could not live with:
//! `gh pr list` needs `pullRequests { nodes { … } }`, and a field named `nodes` is only a way out
//! of the repo when the type it is on says so.
//!
//! The tables are what `gh`'s and `glab`'s own queries select (`api/query_builder.go` in `gh`'s
//! source, `pr`, `issue`, `repo` and `release` view and list; `glab`'s project, merge request,
//! issue and release queries), plus what a person's own `gh api graphql` would ask of the same
//! objects. They are **reads of what the repo holds**. Three things are left out on purpose and
//! are not on any table: a type that reaches *other* repos' content (`search`, `organization`,
//! `user(login:)`, `topic`, `enterprise`, a fork's `forks`, GitLab's `group` and `namespace`), a
//! type that holds the repo's administration (collaborators, rulesets, webhooks, keys, secrets,
//! variables, protection rules, environments, merge queues), and any field this list was not
//! told of. A field GitHub or GitLab adds tomorrow is refused until someone reads it and adds it.
//!
//! **A repo reached from an object** (a pull request's head repository, a fork's parent, an
//! issue's repository) is [`REPO_REF`]: its name, its owner's login and nothing it holds.
//!
//! Lists are written as `Ty::Conn(node)` (a connection of `node`s), whose `nodes`, `edges`,
//! `pageInfo` and counts are the same on every one.

use super::repo::Kind;

/// What a field returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ty {
    /// A scalar, an enum, a list of them: nothing may be selected below it.
    Leaf,
    /// An object of the named type.
    Obj(&'static str),
    /// A connection of the named type: `nodes`, `edges`, `pageInfo` and the counts.
    Conn(&'static str),
    /// An edge of a connection of the named type: its `node` and cursor.
    Edge(&'static str),
}

use Ty::{Conn, Obj};

/// The type a repo reached from another object has.
pub(crate) const REPO_REF: &str = "RepoRef";

type Table = &'static [(
    &'static str,
    &'static [&'static str],
    &'static [(&'static str, Ty)],
)];

/// What a type's table says of one field: its type, or `None` for a field not on the list.
pub(crate) fn field(kind: Kind, parent: &Ty, name: &str) -> Option<Ty> {
    if name == "__typename" {
        return Some(Ty::Leaf);
    }
    match *parent {
        Ty::Leaf => None,
        Ty::Conn(node) => match name {
            "nodes" => Some(Obj(node)),
            "edges" => Some(Ty::Edge(node)),
            "pageInfo" => Some(Obj("PageInfo")),
            "totalCount" | "count" | "totalSize" | "issueCount" => Some(Ty::Leaf),
            _ => None,
        },
        Ty::Edge(node) => match name {
            "node" => Some(Obj(node)),
            "cursor" | "size" => Some(Ty::Leaf),
            _ => None,
        },
        Ty::Obj(type_name) if type_name.starts_with("__") => introspection(type_name, name),
        Ty::Obj(type_name) => {
            let (_, leaves, objects) = table(kind).iter().find(|(t, _, _)| *t == type_name)?;
            if leaves.contains(&name) {
                Some(Ty::Leaf)
            } else {
                objects.iter().find(|(f, _)| *f == name).map(|(_, ty)| *ty)
            }
        }
    }
}

/// The schema's own description of itself (`__type`, `__schema`): read-only metadata of the API,
/// the same for every repo, which `gh` reads to learn what the forge supports. Fields of the
/// introspection types are the specification's.
fn introspection(type_name: &str, name: &str) -> Option<Ty> {
    let obj = |t: &'static str| Some(Obj(t));
    match (type_name, name) {
        ("__Schema", "description") => Some(Ty::Leaf),
        ("__Schema", "types") => Some(Conn("__Type").introspected()),
        ("__Schema", "queryType" | "mutationType" | "subscriptionType") => obj("__Type"),
        ("__Schema", "directives") => obj("__Directive"),
        ("__Type", "kind" | "name" | "description" | "specifiedByURL" | "isOneOf") => {
            Some(Ty::Leaf)
        }
        ("__Type", "fields" | "inputFields" | "interfaces" | "possibleTypes" | "enumValues") => {
            match name {
                "fields" => obj("__Field"),
                "inputFields" => obj("__InputValue"),
                "enumValues" => obj("__EnumValue"),
                _ => obj("__Type"),
            }
        }
        ("__Type", "ofType") => obj("__Type"),
        ("__Field", "name" | "description" | "isDeprecated" | "deprecationReason") => {
            Some(Ty::Leaf)
        }
        ("__Field", "args") => obj("__InputValue"),
        ("__Field", "type") => obj("__Type"),
        (
            "__InputValue",
            "name" | "description" | "defaultValue" | "isDeprecated" | "deprecationReason",
        ) => Some(Ty::Leaf),
        ("__InputValue", "type") => obj("__Type"),
        ("__EnumValue", "name" | "description" | "isDeprecated" | "deprecationReason") => {
            Some(Ty::Leaf)
        }
        ("__Directive", "name" | "description" | "locations" | "isRepeatable") => Some(Ty::Leaf),
        ("__Directive", "args") => obj("__InputValue"),
        _ => None,
    }
}

impl Ty {
    /// `__Schema.types` is a plain list: its type, not a connection.
    fn introspected(self) -> Ty {
        match self {
            Conn(node) => Obj(node),
            other => other,
        }
    }
}

/// The type of a name met as a type condition (`... on PullRequest`), if the tables have one,
/// and the document's own spelling is exact.
pub(crate) fn type_named(kind: Kind, name: &str) -> Option<&'static str> {
    table(kind).iter().map(|(t, _, _)| *t).find(|t| *t == name)
}

/// The type a selection set takes on under `parent` when a fragment says `on cond`: `cond` if
/// `parent` is that type or is one it may be (a union, an interface), else nothing.
pub(crate) fn narrow(kind: Kind, parent: &Ty, cond: &str) -> Option<Ty> {
    let Ty::Obj(parent) = parent else {
        return None;
    };
    let cond = type_named(kind, cond)?;
    let same = *parent == cond;
    let possible = POSSIBLE
        .iter()
        .any(|(abstract_type, concrete)| abstract_type == parent && concrete.contains(&cond));
    (same || possible).then_some(Obj(cond))
}

/// The types an abstract one may be.
const POSSIBLE: &[(&str, &[&str])] = &[
    (
        "Actor",
        &["User", "Bot", "Organization", "Mannequin", "Actor"],
    ),
    (
        "RepositoryOwner",
        &["User", "Organization", "Actor", "RepositoryOwner"],
    ),
    ("RequestedReviewer", &["User", "Team", "Mannequin", "Bot"]),
    ("SearchResultItem", &["Issue", "PullRequest"]),
    ("GitObject", &["Commit", "Blob"]),
    ("IssueOrPullRequest", &["Issue", "PullRequest"]),
    ("StatusCheckRollupContext", &["CheckRun", "StatusContext"]),
    (
        "ProjectV2ItemFieldValue",
        &["ProjectV2ItemFieldSingleSelectValue"],
    ),
    (
        "Node",
        &[
            "Issue",
            "PullRequest",
            "Repository",
            "IssueComment",
            "PullRequestReview",
            "PullRequestReviewComment",
            "PullRequestReviewThread",
            "Release",
            "Label",
            "Milestone",
            "Commit",
            "Ref",
        ],
    ),
];

fn table(kind: Kind) -> Table {
    match kind {
        Kind::GitHub => GITHUB,
        Kind::GitLab => GITLAB,
    }
}

/// What `viewer` and `currentUser`-like actors show: who someone is, and nothing they own.
const ACTOR: &[&str] = &[
    "login",
    "name",
    "id",
    "databaseId",
    "url",
    "avatarUrl",
    "resourcePath",
    "slug",
    "__typename",
];

const REACTION_GROUP: (&str, &[&str], &[(&str, Ty)]) = (
    "ReactionGroup",
    &["content", "viewerHasReacted", "createdAt"],
    &[("users", Conn("Actor"))],
);

#[rustfmt::skip]
const GITHUB: Table = &[
    ("Query", &[], &[]),
    ("Actor", ACTOR, &[]),
    ("RepositoryOwner", ACTOR, &[]),
    ("User", ACTOR, &[]),
    ("Bot", ACTOR, &[]),
    ("Organization", ACTOR, &[]),
    ("Mannequin", ACTOR, &[]),
    ("Team", &["name", "slug", "id", "url", "combinedSlug"], &[("organization", Obj("Organization"))]),
    ("RequestedReviewer", &[], &[]),
    ("Node", &["id"], &[]),
    ("RateLimit", &["limit", "cost", "remaining", "resetAt", "used", "nodeCount"], &[]),
    ("PageInfo", &["hasNextPage", "hasPreviousPage", "startCursor", "endCursor"], &[]),
    ("Payload", &["clientMutationId"], &[
        ("pullRequest", Obj("PullRequest")),
        ("issue", Obj("Issue")),
        ("issueComment", Obj("IssueComment")),
        ("commentEdge", Ty::Edge("IssueComment")),
        ("issueEdge", Ty::Edge("Issue")),
        ("pullRequestEdge", Ty::Edge("PullRequest")),
        ("thread", Obj("PullRequestReviewThread")),
        ("comment", Obj("PullRequestReviewComment")),
        ("labelable", Obj("IssueOrPullRequest")),
        ("repository", Obj("RepoRef")),
    ]),
    (REPO_REF, &["id", "name", "nameWithOwner", "url", "isPrivate", "viewerPermission", "databaseId"], &[
        ("owner", Obj("Actor")),
        ("defaultBranchRef", Obj("RefName")),
    ]),
    // A repository named by a reference in text: what names it, not what the viewer may do to it
    // or whether it is private (that would say whether a repo a chat typed exists).
    ("RepoName", &["id", "name", "nameWithOwner", "url", "databaseId"], &[("owner", Obj("Actor"))]),
    ("RefName", &["id", "name", "prefix"], &[]),
    ("Repository", &[
        "id", "databaseId", "name", "nameWithOwner", "url", "resourcePath", "description",
        "descriptionHTML", "homepageUrl", "isPrivate", "isFork", "isArchived", "isEmpty",
        "isTemplate", "isMirror", "isInOrganization", "isDisabled", "isLocked", "visibility",
        "createdAt", "updatedAt", "pushedAt", "hasIssuesEnabled", "hasWikiEnabled",
        "hasProjectsEnabled", "hasDiscussionsEnabled", "hasSponsorshipsEnabled",
        "mergeCommitAllowed", "squashMergeAllowed", "rebaseMergeAllowed", "autoMergeAllowed",
        "deleteBranchOnMerge", "viewerPermission", "viewerCanAdminister", "viewerHasStarred",
        "viewerSubscription", "viewerDefaultMergeMethod", "viewerDefaultCommitEmail",
        "viewerPossibleCommitEmails", "stargazerCount", "forkCount", "diskUsage", "sshUrl",
        "mirrorUrl", "openGraphImageUrl", "usesCustomOpenGraphImage", "isSecurityPolicyEnabled",
        "securityPolicyUrl", "isBlankIssuesEnabled", "isUserConfigurationRepository",
        "__typename",
    ], &[
        ("owner", Obj("RepositoryOwner")),
        ("defaultBranchRef", Obj("Ref")),
        ("licenseInfo", Obj("License")),
        ("primaryLanguage", Obj("Language")),
        ("languages", Conn("Language")),
        ("repositoryTopics", Conn("RepositoryTopic")),
        ("issues", Conn("Issue")),
        ("issue", Obj("Issue")),
        ("pullRequests", Conn("PullRequest")),
        ("pullRequest", Obj("PullRequest")),
        ("issueOrPullRequest", Obj("IssueOrPullRequest")),
        ("releases", Conn("Release")),
        ("latestRelease", Obj("Release")),
        ("release", Obj("Release")),
        ("labels", Conn("Label")),
        ("label", Obj("Label")),
        ("milestones", Conn("Milestone")),
        ("milestone", Obj("Milestone")),
        ("ref", Obj("Ref")),
        ("refs", Conn("Ref")),
        ("object", Obj("GitObject")),
        ("parent", Obj("RepoRef")),
        ("templateRepository", Obj("RepoRef")),
        ("assignableUsers", Conn("Actor")),
        ("mentionableUsers", Conn("Actor")),
        ("pullRequestTemplates", Obj("PullRequestTemplate")),
        ("issueTemplates", Obj("IssueTemplate")),
        ("contactLinks", Obj("ContactLink")),
        ("fundingLinks", Obj("FundingLink")),
        ("codeOfConduct", Obj("CodeOfConduct")),
    ]),
    ("IssueOrPullRequest", &["id", "number", "title", "url", "state", "body"], &[]),
    ("SearchResultItem", &[], &[]),
    ("PullRequestTemplate", &["body", "filename", "isDefault", "repository"], &[]),
    ("IssueTemplate", &["name", "title", "body", "about", "filename"], &[]),
    ("ContactLink", &["about", "name", "url"], &[]),
    ("FundingLink", &["platform", "url"], &[]),
    ("CodeOfConduct", &["name", "key", "url", "body", "resourcePath"], &[]),
    ("Ref", &["id", "name", "prefix"], &[
        ("target", Obj("GitObject")),
        ("associatedPullRequests", Conn("PullRequestRef")),
    ]),
    ("GitObject", &["id", "oid", "abbreviatedOid"], &[]),
    ("Commit", &[
        "id", "oid", "abbreviatedOid", "message", "messageHeadline", "messageBody",
        "committedDate", "authoredDate", "url", "additions", "deletions", "changedFilesIfAvailable",
    ], &[
        ("author", Obj("GitActor")),
        ("committer", Obj("GitActor")),
        ("authors", Conn("GitActor")),
        ("statusCheckRollup", Obj("StatusCheckRollup")),
        ("associatedPullRequests", Conn("PullRequestRef")),
    ]),
    ("Blob", &["id", "oid", "abbreviatedOid", "text", "byteSize", "isBinary", "isTruncated"], &[]),
    ("GitActor", &["name", "email", "date", "id", "login"], &[("user", Obj("User"))]),
    ("StatusCheckRollup", &["id", "state"], &[
        ("contexts", Conn("StatusCheckRollupContext")),
        ("commit", Obj("Commit")),
    ]),
    ("StatusCheckRollupContext", &[], &[]),
    ("CheckRun", &[
        "id", "databaseId", "name", "status", "conclusion", "startedAt", "completedAt",
        "detailsUrl", "url", "permalink", "title", "summary", "text", "isRequired",
    ], &[("checkSuite", Obj("CheckSuite"))]),
    ("CheckSuite", &["id", "status", "conclusion", "createdAt", "updatedAt", "url"], &[
        ("workflowRun", Obj("WorkflowRun")),
    ]),
    ("WorkflowRun", &["id", "event", "runNumber", "url", "createdAt", "updatedAt"], &[
        ("workflow", Obj("Workflow")),
    ]),
    ("Workflow", &["id", "name"], &[]),
    ("StatusContext", &["id", "context", "state", "targetUrl", "createdAt", "description", "isRequired"], &[]),
    ("PullRequest", &[
        "id", "databaseId", "number", "title", "titleHTML", "body", "bodyText", "bodyHTML",
        "state", "url", "resourcePath", "permalink", "closed", "closedAt", "createdAt", "updatedAt",
        "mergedAt", "merged", "isDraft", "mergeable", "mergeStateStatus", "reviewDecision",
        "baseRefName", "headRefName", "baseRefOid", "headRefOid", "additions", "deletions",
        "changedFiles", "locked", "activeLockReason", "maintainerCanModify", "isCrossRepository",
        "authorAssociation", "isReadByViewer", "viewerCanUpdate", "viewerCanReact",
        "viewerDidAuthor", "viewerSubscription", "viewerCanApplySuggestion",
        "viewerCanDeleteHeadRef", "includesCreatedEdit", "lastEditedAt", "publishedAt",
    ], &[
        ("author", Obj("Actor")),
        ("editor", Obj("Actor")),
        ("mergedBy", Obj("Actor")),
        ("assignees", Conn("Actor")),
        ("labels", Conn("Label")),
        ("milestone", Obj("Milestone")),
        ("comments", Conn("IssueComment")),
        ("reviews", Conn("PullRequestReview")),
        ("latestReviews", Conn("PullRequestReview")),
        ("latestOpinionatedReviews", Conn("PullRequestReview")),
        ("reviewRequests", Conn("ReviewRequest")),
        ("reviewThreads", Conn("PullRequestReviewThread")),
        ("commits", Conn("PullRequestCommit")),
        ("files", Conn("PullRequestChangedFile")),
        ("closingIssuesReferences", Conn("IssueRef")),
        ("headRepository", Obj("RepoRef")),
        ("headRepositoryOwner", Obj("Actor")),
        ("baseRepository", Obj("RepoRef")),
        ("repository", Obj("RepoRef")),
        ("mergeCommit", Obj("Commit")),
        ("potentialMergeCommit", Obj("Commit")),
        ("headRef", Obj("Ref")),
        ("baseRef", Obj("Ref")),
        ("autoMergeRequest", Obj("AutoMergeRequest")),
        ("projectCards", Conn("ProjectCard")),
        ("projectItems", Conn("ProjectV2Item")),
        ("reactionGroups", Obj("ReactionGroup")),
    ]),
    ("AutoMergeRequest", &["authorEmail", "commitBody", "commitHeadline", "mergeMethod", "enabledAt"], &[
        ("enabledBy", Obj("Actor")),
    ]),
    ("PullRequestCommit", &["id", "url", "resourcePath"], &[("commit", Obj("Commit"))]),
    ("PullRequestChangedFile", &["path", "additions", "deletions", "changeType", "viewerViewedState"], &[]),
    ("PullRequestReview", &[
        "id", "body", "bodyText", "state", "submittedAt", "createdAt", "url", "authorAssociation",
        "includesCreatedEdit", "viewerDidAuthor",
    ], &[
        ("author", Obj("Actor")),
        ("commit", Obj("Commit")),
        ("comments", Conn("PullRequestReviewComment")),
        ("reactionGroups", Obj("ReactionGroup")),
    ]),
    ("PullRequestReviewComment", &[
        "id", "body", "bodyText", "path", "line", "originalLine", "position", "originalPosition",
        "diffHunk", "createdAt", "url", "outdated", "isMinimized", "minimizedReason",
        "authorAssociation",
    ], &[
        ("author", Obj("Actor")),
        ("commit", Obj("Commit")),
        ("reactionGroups", Obj("ReactionGroup")),
    ]),
    ("PullRequestReviewThread", &[
        "id", "isResolved", "isOutdated", "isCollapsed", "path", "line", "startLine", "diffSide",
    ], &[
        ("comments", Conn("PullRequestReviewComment")),
        ("resolvedBy", Obj("Actor")),
    ]),
    ("ReviewRequest", &["id", "asCodeOwner"], &[("requestedReviewer", Obj("RequestedReviewer"))]),
    ("IssueComment", &[
        "id", "body", "bodyText", "createdAt", "updatedAt", "url", "authorAssociation",
        "includesCreatedEdit", "isMinimized", "minimizedReason", "viewerDidAuthor",
        "viewerCanUpdate",
    ], &[
        ("author", Obj("Actor")),
        ("reactionGroups", Obj("ReactionGroup")),
    ]),
    REACTION_GROUP,
    ("Issue", &[
        "id", "databaseId", "number", "title", "titleHTML", "body", "bodyText", "bodyHTML",
        "state", "stateReason", "url", "resourcePath", "closed", "closedAt", "createdAt",
        "updatedAt", "locked", "activeLockReason", "isPinned", "authorAssociation",
        "viewerDidAuthor", "viewerCanUpdate", "viewerSubscription", "includesCreatedEdit",
        "viewerCanReact", "viewerCanSubscribe", "isReadByViewer",
        "lastEditedAt", "publishedAt",
    ], &[
        ("author", Obj("Actor")),
        ("assignees", Conn("Actor")),
        ("labels", Conn("Label")),
        ("milestone", Obj("Milestone")),
        ("comments", Conn("IssueComment")),
        ("repository", Obj("RepoRef")),
        ("closedByPullRequestsReferences", Conn("PullRequestRef")),
        ("issueType", Obj("IssueType")),
        ("parent", Obj("RelatedIssue")),
        ("subIssues", Conn("RelatedIssue")),
        ("blockedBy", Conn("RelatedIssue")),
        ("blocking", Conn("RelatedIssue")),
        ("subIssuesSummary", Obj("SubIssuesSummary")),
        ("projectCards", Conn("ProjectCard")),
        ("projectItems", Conn("ProjectV2Item")),
        ("reactionGroups", Obj("ReactionGroup")),
    ]),
    // An issue named by a reference in text ("Fixes org/private#N" in a pull request the chat
    // writes): only what names it, never what it holds. Not its title or state either: a
    // chat that writes the reference could read them for a repo it has no right to, and so
    // learn that the issue exists.
    ("IssueRef", &["id", "number", "url"], &[("repository", Obj("RepoName"))]),
    // An issue the forge itself has related to another (its parent, a sub-issue, one it is
    // blocked by). Only a relationship write makes one, and the proxy allows a chat none, so
    // a chat cannot point it at a repo it chooses; `gh issue view` selects title and state on
    // all four, and nothing it holds beyond them.
    ("RelatedIssue", &["id", "number", "title", "url", "state"], &[("repository", Obj("RepoName"))]),
    // The same for a pull request reached from an issue, a ref or a commit: a reference such as
    // "Fixes org/private#N" can name another repo's, so only what names it is readable.
    ("PullRequestRef", &["id", "number", "url"], &[("repository", Obj("RepoName"))]),
    ("IssueType", &["id", "name", "description", "color"], &[]),
    ("SubIssuesSummary", &["total", "completed", "percentCompleted"], &[]),
    ("Label", &["id", "name", "color", "description", "url", "isDefault", "createdAt", "updatedAt"], &[]),
    ("Milestone", &[
        "id", "number", "title", "description", "dueOn", "state", "url", "closed", "closedAt",
        "createdAt", "updatedAt", "progressPercentage",
    ], &[]),
    ("ProjectCard", &["id", "state", "url"], &[
        ("project", Obj("Project")),
        ("column", Obj("ProjectColumn")),
    ]),
    ("Project", &["id", "name", "number", "url", "closed"], &[]),
    ("ProjectColumn", &["id", "name"], &[]),
    ("ProjectV2Item", &["id", "type"], &[
        ("project", Obj("ProjectV2")),
        ("fieldValueByName", Obj("ProjectV2ItemFieldValue")),
    ]),
    ("ProjectV2", &["id", "title", "number", "url"], &[]),
    ("ProjectV2ItemFieldValue", &[], &[]),
    ("ProjectV2ItemFieldSingleSelectValue", &["optionId", "name"], &[]),
    ("Release", &[
        "id", "name", "tagName", "description", "descriptionHTML", "url", "resourcePath",
        "isDraft", "isPrerelease", "isLatest", "createdAt", "publishedAt", "updatedAt",
    ], &[
        ("author", Obj("Actor")),
        ("releaseAssets", Conn("ReleaseAsset")),
        ("tag", Obj("Ref")),
    ]),
    ("ReleaseAsset", &[
        "id", "name", "contentType", "size", "downloadUrl", "downloadCount", "createdAt",
        "updatedAt", "url", "label",
    ], &[("uploadedBy", Obj("Actor"))]),
    ("License", &["id", "name", "nameWithOwner", "nickname", "key", "spdxId", "url", "pseudoLicense", "description"], &[]),
    ("Language", &["id", "name", "color"], &[]),
    ("RepositoryTopic", &["id", "url"], &[("topic", Obj("Topic"))]),
    ("Topic", &["id", "name"], &[]),
];

#[rustfmt::skip]
const GITLAB: Table = &[
    ("Query", &[], &[]),
    ("UserCore", &["id", "username", "name", "webUrl", "webPath", "avatarUrl", "state", "__typename"], &[]),
    ("Metadata", &["version", "revision", "enterprise", "kas"], &[]),
    ("PageInfo", &["hasNextPage", "hasPreviousPage", "startCursor", "endCursor"], &[]),
    ("Payload", &["clientMutationId", "errors"], &[
        ("mergeRequest", Obj("MergeRequest")),
        ("issue", Obj("Issue")),
        ("note", Obj("Note")),
        ("discussion", Obj("Discussion")),
    ]),
    ("Project", &[
        "id", "name", "fullPath", "path", "description", "webUrl", "visibility", "archived",
        "starCount", "forksCount", "createdAt", "lastActivityAt", "sshUrlToRepo", "httpUrlToRepo",
        "nameWithNamespace", "openIssuesCount", "issuesEnabled", "mergeRequestsEnabled",
    ], &[
        ("repository", Obj("GlRepository")),
        ("mergeRequests", Conn("MergeRequest")),
        ("mergeRequest", Obj("MergeRequest")),
        ("issues", Conn("Issue")),
        ("issue", Obj("Issue")),
        ("labels", Conn("Label")),
        ("label", Obj("Label")),
        ("releases", Conn("Release")),
        ("release", Obj("Release")),
        ("pipelines", Conn("Pipeline")),
        ("pipeline", Obj("Pipeline")),
        ("milestones", Conn("Milestone")),
    ]),
    ("GlRepository", &["rootRef", "exists", "empty"], &[]),
    ("MergeRequest", &[
        "id", "iid", "title", "description", "state", "draft",
        "webUrl", "createdAt", "updatedAt", "mergedAt", "closedAt", "sourceBranch", "targetBranch",
        "commitCount", "userNotesCount", "upvotes", "downvotes", "mergeStatus",
        "detailedMergeStatus", "conflicts", "shouldBeRebased", "approved", "approvalsRequired",
        "approvalsLeft", "diffHeadSha", "subscribed", "discussionLocked", "sourceProjectId",
        "targetProjectId",
    ], &[
        ("author", Obj("UserCore")),
        ("assignees", Conn("UserCore")),
        ("reviewers", Conn("UserCore")),
        ("labels", Conn("Label")),
        ("milestone", Obj("Milestone")),
        ("notes", Conn("Note")),
        ("discussions", Conn("Discussion")),
        ("headPipeline", Obj("Pipeline")),
        ("diffStatsSummary", Obj("DiffStatsSummary")),
    ]),
    ("DiffStatsSummary", &["additions", "deletions", "changes", "fileCount"], &[]),
    ("Issue", &[
        "id", "iid", "title", "description", "state", "webUrl",
        "createdAt", "updatedAt", "closedAt", "confidential", "userNotesCount", "upvotes",
        "downvotes", "dueDate", "type",
    ], &[
        ("author", Obj("UserCore")),
        ("assignees", Conn("UserCore")),
        ("labels", Conn("Label")),
        ("milestone", Obj("Milestone")),
        ("notes", Conn("Note")),
        ("discussions", Conn("Discussion")),
    ]),
    ("Note", &[
        "id", "body", "createdAt", "updatedAt", "system", "url", "resolvable",
        "resolved", "internal",
    ], &[("author", Obj("UserCore")), ("resolvedBy", Obj("UserCore"))]),
    ("Discussion", &["id", "createdAt", "resolvable", "resolved", "replyId"], &[
        ("notes", Conn("Note")),
        ("resolvedBy", Obj("UserCore")),
    ]),
    ("Label", &["id", "title", "color", "textColor", "description", "createdAt", "updatedAt"], &[]),
    ("Milestone", &[
        "id", "iid", "title", "description", "dueDate", "startDate", "state", "webPath", "webUrl",
        "createdAt", "updatedAt",
    ], &[]),
    ("Release", &[
        "name", "tagName", "tagPath", "description", "createdAt", "releasedAt",
        "upcomingRelease",
    ], &[
        ("author", Obj("UserCore")),
        ("assets", Obj("ReleaseAssets")),
        ("links", Obj("ReleaseLinks")),
        ("commit", Obj("Commit")),
    ]),
    ("ReleaseAssets", &["count"], &[
        ("links", Conn("ReleaseAssetLink")),
        ("sources", Conn("ReleaseSource")),
    ]),
    ("ReleaseAssetLink", &["id", "name", "url", "directAssetUrl", "linkType", "external"], &[]),
    ("ReleaseSource", &["format", "url"], &[]),
    ("ReleaseLinks", &[
        "closedIssuesUrl", "closedMergeRequestsUrl", "editUrl", "mergedMergeRequestsUrl",
        "openedIssuesUrl", "openedMergeRequestsUrl", "selfUrl",
    ], &[]),
    ("Commit", &[
        "sha", "shortId", "title", "description", "message", "webUrl", "authoredDate",
        "committedDate", "authorName", "authorEmail",
    ], &[]),
    ("Pipeline", &[
        "id", "iid", "status", "sha", "ref", "createdAt", "startedAt", "finishedAt", "duration",
        "path",
    ], &[
        ("detailedStatus", Obj("DetailedStatus")),
        ("jobs", Conn("CiJob")),
        ("stages", Conn("CiStage")),
    ]),
    ("DetailedStatus", &["text", "label", "group", "icon", "tooltip", "detailsPath", "hasDetails"], &[]),
    ("CiJob", &["id", "name", "status", "duration", "startedAt", "finishedAt", "webPath", "allowFailure"], &[]),
    ("CiStage", &["id", "name", "status"], &[]),
];
