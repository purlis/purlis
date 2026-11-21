// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The held acts' forge adapters** (one login F2; ADR 0128 D8): [`HeldForge`] for GitHub and for
//! GitLab, over REST, **always as the person**: the transport they are built on carries the
//! person's sign-in ([`CredentialTokens`]), never a chat's.
//!
//! | Act | GitHub | GitLab |
//! |---|---|---|
//! | tag | `POST git/refs` (`refs/tags/<name>` at the commit): a tag that exists is a 422, never moved | `POST repository/tags` with `ref` the commit |
//! | release | `POST releases` with `tag_name` and **no `target_commitish`** | `POST releases` with `tag_name` and no `ref` |
//! | CI-file push | blobs, a tree on the base's tree, **one commit whose parent is the base**, then `PATCH git/refs/heads/<branch>` with `force: false` (the lease: a branch that moved is no fast-forward and is refused) or `POST git/refs` for a branch that is not there | `POST repository/commits` with `actions`; the branch's tip is read right before and again after, and a branch that moved is said |
//! | fork | `POST forks` with no organization | `POST fork` with no namespace |
//!
//! **A push through the API is one commit** whose tree is the tip's and whose parent is the base:
//! the commits between are not sent, and its id is not the tip's. The ask showed the tip's changes
//! against the base, and this makes exactly those. ([`TipFiles`] reads them from the clone.)
//!
//! **GitHub's push uses the second credential** ([`WorkflowTransports`]) for every call it makes,
//! because a tree that changes a workflow needs the `workflow` scope. **Seam "G"**: slice G's
//! `accounts::workflow` becomes a [`WorkflowTransports`] when it lands; until then the app builds
//! none and a held CI-file push on GitHub is refused with [`NoWorkflowCredential`]'s sentence.
//!
//! **Tier** (ADR 0069): nothing is stored here.
//!
//! [`NoWorkflowCredential`]: crate::heldact::NoWorkflowCredential

use std::sync::Arc;

use base64::Engine as _;
use secrecy::SecretString;
use serde_json::Value;

use super::credential::CredentialSource;
use super::http::TokenSource;
use super::transport::{Call, Field, Method, Reply, Transport};
use super::{Forge, ForgeError, Kind, LIST_TIMEOUT, quote};
use crate::heldact::{Forked, HeldForge, TagName, TipChange, TipFiles, valid_rev};
use crate::mergehold::{Base, HeadSha, Repo};
use crate::secrets::keyring::Secret;

/// **The person's sign-in as a [`TokenSource`]**: read from the credential source for each
/// request, so a renewal is picked up and nothing is held.
pub struct CredentialTokens {
    host: String,
    source: Arc<dyn CredentialSource>,
}

impl CredentialTokens {
    pub fn new(host: &str, source: Arc<dyn CredentialSource>) -> CredentialTokens {
        CredentialTokens {
            host: host.to_owned(),
            source,
        }
    }
}

impl TokenSource for CredentialTokens {
    fn token(&self) -> Result<SecretString, ForgeError> {
        self.source
            .credential_for(&self.host)
            .map(|credential| credential.token().clone())
            .map_err(|why| ForgeError::of(super::Failure::Auth, why.to_string()))
    }
}

/// **A transport that carries the second credential** for one push. A seam: slice G's workflow
/// account makes the real one, through the resolver like every native transport
/// (`only_the_resolver_builds_the_native_transport`).
pub trait WorkflowTransports: Send + Sync {
    fn with(&self, forge: &Forge, secret: &Secret) -> Result<Arc<dyn Transport>, String>;
}

/// **The held-act forge for `forge`**, as the person: `person` is the native transport the resolver
/// built with the person's sign-in ([`CredentialTokens`] is the token source it gives it). `files`
/// reads a held push's changes from the quarantine.
pub fn held_forge(
    forge: Forge,
    person: Arc<dyn Transport>,
    files: Arc<dyn TipFiles>,
    workflow: Arc<dyn WorkflowTransports>,
) -> Box<dyn HeldForge> {
    let rest = Rest {
        forge: forge.clone(),
        person,
        files,
        workflow,
    };
    match forge.kind {
        Kind::GitHub => Box::new(GitHubHeld(rest)),
        Kind::GitLab => Box::new(GitLabHeld(rest)),
    }
}

struct Rest {
    forge: Forge,
    person: Arc<dyn Transport>,
    files: Arc<dyn TipFiles>,
    workflow: Arc<dyn WorkflowTransports>,
}

/// An answer: its JSON, or `None` for a 404.
type Answer = Result<Option<Value>, String>;

impl Rest {
    fn kind(&self) -> Kind {
        self.forge.kind
    }

    fn send(&self, over: &dyn Transport, call: &Call) -> Answer {
        let reply = over
            .send(&self.forge, call)
            .map_err(|why| why.said().to_owned())?;
        if reply.ok() {
            return Ok(Some(json_of(&reply)));
        }
        if not_there(&reply) {
            return Ok(None);
        }
        Err(reply.said(self.kind()))
    }

    /// A read that must answer.
    fn get(&self, over: &dyn Transport, path: &str, doing: &str) -> Result<Value, String> {
        self.send(over, &Call::get(path, LIST_TIMEOUT))?
            .ok_or_else(|| format!("{doing}: the forge has no such thing"))
    }

    /// A read where there being nothing is an answer.
    fn find(&self, over: &dyn Transport, path: &str) -> Answer {
        self.send(over, &Call::get(path, LIST_TIMEOUT))
    }

    fn write(
        &self,
        over: &dyn Transport,
        method: Method,
        path: &str,
        fields: Vec<Field>,
        doing: &str,
    ) -> Result<Value, String> {
        self.send(over, &Call::write(method, path, fields))?
            .ok_or_else(|| format!("{doing}: the forge has no such thing"))
    }

    fn person(&self) -> &dyn Transport {
        self.person.as_ref()
    }
}

fn json_of(reply: &Reply) -> Value {
    serde_json::from_str(&reply.out).unwrap_or(Value::Null)
}

fn not_there(reply: &Reply) -> bool {
    reply.status == Some(404) || reply.both().contains("HTTP 404")
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

fn sha_in(value: &Value, what: &str) -> Result<HeadSha, String> {
    let said = value.as_str().unwrap_or_default();
    HeadSha::try_from(said).map_err(|_| format!("the forge gave no commit for {what}"))
}

/// **A ref name that is safe in a path**, by `git check-ref-format`'s rules: a branch is a chat's
/// words, `quoted_path` keeps its slashes, and a `..` segment would walk the request out of the
/// repository's `git/refs` to another route.
fn ref_name(name: &str) -> Result<(), String> {
    if valid_rev(name) {
        Ok(())
    } else {
        Err("the branch's name is not a name git allows, so nothing was sent".to_owned())
    }
}

/// A path's segments, each quoted: a branch's slashes stay slashes. Only for a name [`ref_name`]
/// has passed.
fn quoted_path(name: &str) -> String {
    name.split('/').map(quote).collect::<Vec<_>>().join("/")
}

// ---------------------------------------------------------------------------------------
// GitHub                                                                                  #
// ---------------------------------------------------------------------------------------

struct GitHubHeld(Rest);

impl GitHubHeld {
    fn repo_path(repo: &Repo) -> String {
        format!("repos/{}", quoted_path(repo.as_str()))
    }

    fn repo_record(&self, repo: &Repo) -> Result<Value, String> {
        self.0.get(
            self.0.person(),
            &Self::repo_path(repo),
            &format!("reading {}", repo.as_str()),
        )
    }

    /// The commit a ref of `kind` (`heads` or `tags`) names, or `None` where there is no such ref.
    /// An annotated tag is followed to the commit it tags.
    fn ref_commit(
        &self,
        over: &dyn Transport,
        repo: &Repo,
        kind: &str,
        name: &str,
    ) -> Result<Option<HeadSha>, String> {
        ref_name(name)?;
        let path = format!(
            "{}/git/ref/{kind}/{}",
            Self::repo_path(repo),
            quoted_path(name)
        );
        let Some(record) = self.0.find(over, &path)? else {
            return Ok(None);
        };
        let object = &record["object"];
        let sha = sha_in(&object["sha"], name)?;
        if object["type"].as_str() == Some("tag") {
            let tag = self.0.get(
                over,
                &format!("{}/git/tags/{}", Self::repo_path(repo), sha.as_str()),
                "reading the tag",
            )?;
            return sha_in(&tag["object"]["sha"], name).map(Some);
        }
        Ok(Some(sha))
    }

    /// The push, as the transport `over` carries it.
    fn push(
        &self,
        over: &dyn Transport,
        repo: &Repo,
        branch: &Base,
        tip: &HeadSha,
        base: &HeadSha,
        new_branch: bool,
    ) -> Result<(), String> {
        ref_name(branch.as_str())?;
        let path = Self::repo_path(repo);
        // The lease, read before anything is sent: the branch must be where the ask showed it (or
        // not be there at all). A forge's non-forced update accepts any fast-forward of the
        // branch's *current* tip, so a branch rewound to an ancestor of the base would take the
        // commit, and this refuses that.
        let now = self.ref_commit(over, repo, "heads", branch.as_str())?;
        match (new_branch, &now) {
            (false, Some(at)) if at == base => {}
            (false, _) => {
                return Err(format!(
                    "the branch is no longer at {}, so nothing was pushed",
                    &base.as_str()[..12]
                ));
            }
            (true, Some(_)) => {
                return Err("the branch exists now, so nothing was pushed".to_owned());
            }
            (true, None) => {}
        }
        let parent = base.clone();
        let changes = self.0.files.changes(&parent, tip)?;
        if changes.is_empty() {
            return Err("the tip changes nothing against the base".to_owned());
        }
        let message = self.0.files.message(tip)?;
        let base_commit = self.0.get(
            over,
            &format!("{path}/git/commits/{}", parent.as_str()),
            "reading the base commit",
        )?;
        let base_tree = base_commit["tree"]["sha"]
            .as_str()
            .ok_or_else(|| "the forge gave no tree for the base".to_owned())?
            .to_owned();
        let mut tree_fields = vec![Field::text("base_tree", &base_tree)];
        for change in &changes {
            tree_fields.extend(self.tree_entry(over, &path, change)?);
        }
        let tree = self.0.write(
            over,
            Method::Post,
            &format!("{path}/git/trees"),
            tree_fields,
            "making the tree",
        )?;
        let tree = tree["sha"]
            .as_str()
            .ok_or_else(|| "the forge gave no tree".to_owned())?;
        let commit = self.0.write(
            over,
            Method::Post,
            &format!("{path}/git/commits"),
            vec![
                Field::text("message", &message),
                Field::text("tree", tree),
                Field::text("parents[]", parent.as_str()),
            ],
            "making the commit",
        )?;
        let commit = commit["sha"]
            .as_str()
            .ok_or_else(|| "the forge gave no commit".to_owned())?;
        match new_branch {
            // The lease: not forced, so a branch that is no longer at the base is no fast-forward.
            false => self.0.write(
                over,
                Method::Patch,
                &format!("{path}/git/refs/heads/{}", quoted_path(branch.as_str())),
                vec![Field::text("sha", commit), Field::typed("force", "false")],
                "moving the branch",
            ),
            // A branch that was not there: creating it is refused if it is now.
            true => self.0.write(
                over,
                Method::Post,
                &format!("{path}/git/refs"),
                vec![
                    Field::text("ref", &format!("refs/heads/{}", branch.as_str())),
                    Field::text("sha", commit),
                ],
                "making the branch",
            ),
        }
        .map(|_| ())
    }

    fn tree_entry(
        &self,
        over: &dyn Transport,
        path: &str,
        change: &TipChange,
    ) -> Result<Vec<Field>, String> {
        let mut entry = vec![
            Field::text("tree[][path]", &change.path),
            Field::text(
                "tree[][mode]",
                if change.executable {
                    "100755"
                } else {
                    "100644"
                },
            ),
            Field::text("tree[][type]", "blob"),
        ];
        match &change.content {
            None => entry.push(Field::typed("tree[][sha]", "null")),
            Some(content) => {
                let blob = self.0.write(
                    over,
                    Method::Post,
                    &format!("{path}/git/blobs"),
                    vec![
                        Field::text(
                            "content",
                            &base64::engine::general_purpose::STANDARD.encode(content),
                        ),
                        Field::text("encoding", "base64"),
                    ],
                    "making a file",
                )?;
                let sha = blob["sha"]
                    .as_str()
                    .ok_or_else(|| "the forge gave no blob".to_owned())?;
                entry.push(Field::text("tree[][sha]", sha));
            }
        }
        Ok(entry)
    }
}

impl HeldForge for GitHubHeld {
    fn kind(&self) -> Kind {
        Kind::GitHub
    }

    fn host(&self) -> String {
        self.0.forge.host.clone()
    }

    fn rev_commit(&self, repo: &Repo, rev: &str) -> Result<HeadSha, String> {
        if !valid_rev(rev) {
            return Err("that is not a commit id or a ref name".to_owned());
        }
        // The whole rev is one path segment: a slash in it never becomes another route.
        let record = self.0.get(
            self.0.person(),
            &format!("{}/commits/{}", Self::repo_path(repo), quote(rev)),
            &format!("reading {rev}"),
        )?;
        sha_in(&record["sha"], rev)
    }

    fn default_branch(&self, repo: &Repo) -> Result<(Base, HeadSha), String> {
        let record = self.repo_record(repo)?;
        let name = text(&record, "default_branch")
            .ok_or_else(|| "the forge named no default branch".to_owned())?;
        let base =
            Base::try_from(name).map_err(|_| "the default branch's name is odd".to_owned())?;
        let tip = self
            .ref_commit(self.0.person(), repo, "heads", name)?
            .ok_or_else(|| "the default branch has no tip".to_owned())?;
        Ok((base, tip))
    }

    fn branch_tip(&self, repo: &Repo, branch: &Base) -> Result<Option<HeadSha>, String> {
        self.ref_commit(self.0.person(), repo, "heads", branch.as_str())
    }

    fn on_branch(&self, repo: &Repo, branch: &Base, commit: &HeadSha) -> Result<bool, String> {
        // Compared as the commit then the branch: the branch is ahead of, or at, the commit
        // exactly where the commit is reachable from it.
        ref_name(branch.as_str())?;
        let path = format!(
            "{}/compare/{}...{}?per_page=1",
            Self::repo_path(repo),
            commit.as_str(),
            quoted_path(branch.as_str())
        );
        let record = self.0.get(self.0.person(), &path, "comparing")?;
        Ok(matches!(
            record["status"].as_str(),
            Some("ahead" | "identical")
        ))
    }

    fn tag_at(&self, repo: &Repo, name: &TagName) -> Result<Option<HeadSha>, String> {
        self.ref_commit(self.0.person(), repo, "tags", name.as_str())
    }

    fn ci_config_path(&self, _: &Repo) -> Result<Option<String>, String> {
        // GitHub's CI files are by convention only: `.github/workflows` and its kin.
        Ok(None)
    }

    fn source_id(&self, repo: &Repo) -> Result<String, String> {
        let record = self.repo_record(repo)?;
        record["id"]
            .as_u64()
            .map(|id| id.to_string())
            .ok_or_else(|| "the forge gave the repo no id".to_owned())
    }

    fn account(&self) -> Result<String, String> {
        let record = self
            .0
            .get(self.0.person(), "user", "reading who is signed in")?;
        text(&record, "login")
            .map(str::to_owned)
            .ok_or_else(|| "the forge named no account".to_owned())
    }

    fn may_fork(&self, repo: &Repo) -> Result<bool, String> {
        let record = self.repo_record(repo)?;
        Ok(record["allow_forking"].as_bool() != Some(false))
    }

    fn create_tag(&self, repo: &Repo, name: &TagName, at: &HeadSha) -> Result<(), String> {
        self.0
            .write(
                self.0.person(),
                Method::Post,
                &format!("{}/git/refs", Self::repo_path(repo)),
                vec![
                    Field::text("ref", &format!("refs/tags/{}", name.as_str())),
                    Field::text("sha", at.as_str()),
                ],
                "making the tag",
            )
            .map(|_| ())
    }

    fn create_release(
        &self,
        repo: &Repo,
        tag: &TagName,
        title: &str,
        notes: &str,
        draft: bool,
        prerelease: bool,
    ) -> Result<(), String> {
        // No `target_commitish`: the tag is the only thing the release names.
        self.0
            .write(
                self.0.person(),
                Method::Post,
                &format!("{}/releases", Self::repo_path(repo)),
                vec![
                    Field::text("tag_name", tag.as_str()),
                    Field::text("name", title),
                    Field::text("body", notes),
                    Field::typed("draft", &draft.to_string()),
                    Field::typed("prerelease", &prerelease.to_string()),
                ],
                "making the release",
            )
            .map(|_| ())
    }

    fn make_fork(&self, source: &Repo, source_id: &str) -> Result<Forked, String> {
        // GitHub forks by name: the repo must still be the one the ask showed (a name can be
        // renamed or taken over between the ask and the press).
        if self.source_id(source)? != source_id {
            return Err("the repo is not the one the ask showed (its id differs)".to_owned());
        }
        let record = self.0.write(
            self.0.person(),
            Method::Post,
            &format!("{}/forks", Self::repo_path(source)),
            Vec::new(),
            "making the fork",
        )?;
        let path = text(&record, "full_name")
            .ok_or_else(|| "the forge named no fork".to_owned())?
            .to_owned();
        let id = record["id"]
            .as_u64()
            .map(|id| id.to_string())
            .unwrap_or_default();
        Ok(Forked { path, id })
    }

    fn push_tip(
        &self,
        repo: &Repo,
        branch: &Base,
        tip: &HeadSha,
        base: &HeadSha,
        new_branch: bool,
        workflow: Option<&Secret>,
    ) -> Result<(), String> {
        let Some(secret) = workflow else {
            return Err(
                "a push to GitHub that changes CI files needs the second sign-in".to_owned(),
            );
        };
        let over = self.0.workflow.with(&self.0.forge, secret)?;
        self.push(over.as_ref(), repo, branch, tip, base, new_branch)
    }
}

// ---------------------------------------------------------------------------------------
// GitLab                                                                                  #
// ---------------------------------------------------------------------------------------

struct GitLabHeld(Rest);

impl GitLabHeld {
    fn project(repo: &Repo) -> String {
        format!("projects/{}", quote(repo.as_str()))
    }

    fn project_record(&self, repo: &Repo) -> Result<Value, String> {
        self.0.get(
            self.0.person(),
            &Self::project(repo),
            &format!("reading {}", repo.as_str()),
        )
    }

    fn commit_id(value: &Value, what: &str) -> Result<HeadSha, String> {
        sha_in(&value["id"], what)
    }
}

impl HeldForge for GitLabHeld {
    fn kind(&self) -> Kind {
        Kind::GitLab
    }

    fn host(&self) -> String {
        self.0.forge.host.clone()
    }

    fn rev_commit(&self, repo: &Repo, rev: &str) -> Result<HeadSha, String> {
        if !valid_rev(rev) {
            return Err("that is not a commit id or a ref name".to_owned());
        }
        let record = self.0.get(
            self.0.person(),
            &format!("{}/repository/commits/{}", Self::project(repo), quote(rev)),
            &format!("reading {rev}"),
        )?;
        Self::commit_id(&record, rev)
    }

    fn default_branch(&self, repo: &Repo) -> Result<(Base, HeadSha), String> {
        let record = self.project_record(repo)?;
        let name = text(&record, "default_branch")
            .ok_or_else(|| "the forge named no default branch".to_owned())?;
        let base =
            Base::try_from(name).map_err(|_| "the default branch's name is odd".to_owned())?;
        let tip = self
            .branch_tip(repo, &base)?
            .ok_or_else(|| "the default branch has no tip".to_owned())?;
        Ok((base, tip))
    }

    fn branch_tip(&self, repo: &Repo, branch: &Base) -> Result<Option<HeadSha>, String> {
        ref_name(branch.as_str())?;
        let path = format!(
            "{}/repository/branches/{}",
            Self::project(repo),
            quote(branch.as_str())
        );
        match self.0.find(self.0.person(), &path)? {
            None => Ok(None),
            Some(record) => Self::commit_id(&record["commit"], branch.as_str()).map(Some),
        }
    }

    fn on_branch(&self, repo: &Repo, branch: &Base, commit: &HeadSha) -> Result<bool, String> {
        // The merge base of the two is the commit itself exactly where the branch contains it.
        ref_name(branch.as_str())?;
        let path = format!(
            "{}/repository/merge_base?refs[]={}&refs[]={}",
            Self::project(repo),
            commit.as_str(),
            quote(branch.as_str())
        );
        let record = self.0.get(self.0.person(), &path, "comparing")?;
        Ok(Self::commit_id(&record, "the merge base")
            .map(|base| base == *commit)
            .unwrap_or(false))
    }

    fn tag_at(&self, repo: &Repo, name: &TagName) -> Result<Option<HeadSha>, String> {
        let path = format!(
            "{}/repository/tags/{}",
            Self::project(repo),
            quote(name.as_str())
        );
        match self.0.find(self.0.person(), &path)? {
            None => Ok(None),
            Some(record) => Self::commit_id(&record["commit"], name.as_str()).map(Some),
        }
    }

    fn ci_config_path(&self, repo: &Repo) -> Result<Option<String>, String> {
        let record = self.project_record(repo)?;
        // `null` and an empty string are the default pipeline file; a record that does not carry
        // the key at all is one purlis cannot read the path from, and a guess is never made.
        if record.get("ci_config_path").is_none() {
            return Err("the project's record does not say where its pipeline file is".to_owned());
        }
        Ok(text(&record, "ci_config_path").map(str::to_owned))
    }

    fn source_id(&self, repo: &Repo) -> Result<String, String> {
        let record = self.project_record(repo)?;
        record["id"]
            .as_u64()
            .map(|id| id.to_string())
            .ok_or_else(|| "the forge gave the project no id".to_owned())
    }

    fn account(&self) -> Result<String, String> {
        let record = self
            .0
            .get(self.0.person(), "user", "reading who is signed in")?;
        text(&record, "username")
            .map(str::to_owned)
            .ok_or_else(|| "the forge named no account".to_owned())
    }

    fn may_fork(&self, repo: &Repo) -> Result<bool, String> {
        let record = self.project_record(repo)?;
        Ok(record["forking_access_level"].as_str() != Some("disabled"))
    }

    fn create_tag(&self, repo: &Repo, name: &TagName, at: &HeadSha) -> Result<(), String> {
        self.0
            .write(
                self.0.person(),
                Method::Post,
                &format!("{}/repository/tags", Self::project(repo)),
                vec![
                    Field::text("tag_name", name.as_str()),
                    Field::text("ref", at.as_str()),
                ],
                "making the tag",
            )
            .map(|_| ())
    }

    fn create_release(
        &self,
        repo: &Repo,
        tag: &TagName,
        title: &str,
        notes: &str,
        draft: bool,
        prerelease: bool,
    ) -> Result<(), String> {
        if draft || prerelease {
            return Err("GitLab has no draft or pre-release releases".to_owned());
        }
        // No `ref`: the tag exists, and it is the only thing the release names.
        let mut fields = vec![Field::text("tag_name", tag.as_str())];
        if !title.is_empty() {
            fields.push(Field::text("name", title));
        }
        fields.push(Field::text("description", notes));
        self.0
            .write(
                self.0.person(),
                Method::Post,
                &format!("{}/releases", Self::project(repo)),
                fields,
                "making the release",
            )
            .map(|_| ())
    }

    fn make_fork(&self, source: &Repo, source_id: &str) -> Result<Forked, String> {
        // By numeric id where the ask has one, so a project renamed since is not forked instead.
        let project = if !source_id.is_empty() && source_id.bytes().all(|b| b.is_ascii_digit()) {
            format!("projects/{source_id}")
        } else {
            Self::project(source)
        };
        let record = self.0.write(
            self.0.person(),
            Method::Post,
            &format!("{project}/fork"),
            Vec::new(),
            "making the fork",
        )?;
        let path = text(&record, "path_with_namespace")
            .ok_or_else(|| "the forge named no fork".to_owned())?
            .to_owned();
        let id = record["id"]
            .as_u64()
            .map(|id| id.to_string())
            .unwrap_or_default();
        Ok(Forked { path, id })
    }

    fn push_tip(
        &self,
        repo: &Repo,
        branch: &Base,
        tip: &HeadSha,
        base: &HeadSha,
        new_branch: bool,
        _: Option<&Secret>,
    ) -> Result<(), String> {
        ref_name(branch.as_str())?;
        // The lease: the branch is read right before, and must be where the ask said.
        let now = self.branch_tip(repo, branch)?;
        match (new_branch, &now) {
            (false, Some(at)) if at == base => {}
            (false, _) => {
                return Err(format!(
                    "the branch is no longer at {}, so nothing was pushed",
                    &base.as_str()[..12]
                ));
            }
            (true, Some(_)) => {
                return Err("the branch exists now, so nothing was pushed".to_owned());
            }
            (true, None) => {}
        }
        // The commit's parent is the base the ask showed, never the default branch read again.
        let parent = base.clone();
        let changes = self.0.files.changes(&parent, tip)?;
        if changes.is_empty() {
            return Err("the tip changes nothing against the base".to_owned());
        }
        let mut fields = vec![
            Field::text("branch", branch.as_str()),
            Field::text("commit_message", &self.0.files.message(tip)?),
        ];
        if new_branch {
            fields.push(Field::text("start_sha", parent.as_str()));
        }
        for change in &changes {
            let action = match (&change.content, change.added) {
                (None, _) => "delete",
                (Some(_), true) => "create",
                (Some(_), false) => "update",
            };
            fields.push(Field::text("actions[][action]", action));
            fields.push(Field::text("actions[][file_path]", &change.path));
            if let Some(content) = &change.content {
                fields.push(Field::text("actions[][content]", content));
                fields.push(Field::text("actions[][encoding]", "text"));
                fields.push(Field::typed(
                    "actions[][execute_filemode]",
                    &change.executable.to_string(),
                ));
            }
        }
        let made = self.0.write(
            self.0.person(),
            Method::Post,
            &format!("{}/repository/commits", Self::project(repo)),
            fields,
            "making the commit",
        )?;
        // A branch that moved between the read and the commit takes the commit on its new tip:
        // said, since the push is then not on the base the person was shown.
        let on = made["parent_ids"]
            .as_array()
            .is_some_and(|parents| parents.iter().any(|p| p.as_str() == Some(parent.as_str())));
        if on {
            Ok(())
        } else {
            Err(format!(
                "the branch moved while the push was made, so the commit is on its new tip and \
                 not on {}. Look at the branch before anything else",
                &parent.as_str()[..12]
            ))
        }
    }
}

#[cfg(test)]
mod tests;
