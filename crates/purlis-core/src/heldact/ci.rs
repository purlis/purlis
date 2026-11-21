// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which pushes touch CI files, and what they change** (ADR 0128 D8.3).
//!
//! A pipeline file runs code as the person (GitLab) or with the repository's secrets (both), so a
//! push that changes one is held for the person's press. **What decides it is the tree diff of the
//! new tip against the base**, never a ref's name or a commit's message:
//!
//! - the base is **the tip the forge has of that branch**, or, for a branch the forge has no tip
//!   for, **the default branch's tip** as the forge reports it for this push, so a new branch that
//!   adds or changes a workflow against the default branch is in the diff;
//! - the same diff covers **every other way a ref moves**: a REST `POST …/git/refs`, a `PATCH`,
//!   GraphQL's `createRef` and `updateRef`. Their target is a commit, and it is diffed the same
//!   way: its **tree** against the base's, wherever the objects came from. GitHub shares objects
//!   between a repository and its forks, so a ref can be created at a commit nobody pushed to this
//!   repository; a commit's own change from its parent says nothing of how its tree differs from
//!   the base's.
//! - **a diff that cannot be computed refuses the push** ([`PushCheck::Refuse`]): an object the
//!   clone does not have, a diff too large to show, a path the app cannot read, a `local:` include
//!   whose path is a variable. It is never allowed and never guessed.
//!
//! CI files are `.github/workflows/**` on GitHub, and on GitLab the project's pipeline file
//! (`.gitlab-ci.yml`, or the project's `ci_config_path` where it names a file of this repository),
//! every file its `include:` names in this repository (a `local:` path, a glob of paths, a plain
//! string), and every file a job's `trigger: include:` names. An include of **this** project
//! (`project:`, one of its components, a raw address of it) is a local one. An include of another
//! project, a remote address, a template or a component is the forge's to serve, not this push's:
//! it is named beside the diff so the person reads that the pipeline also runs it.
//!
//! **A pipeline file is read only in a strict subset of YAML** (`strict_subset`): outside it the
//! file is unresolved and every YAML file of the push counts as a CI file.

use super::Trees;
use crate::forge::Kind;
use crate::mergehold::HeadSha;

/// The most of a diff the ask shows. A larger one cannot be shown whole, and what cannot be shown
/// is refused.
pub const MOST_DIFF: usize = 256 * 1024;

/// The deepest chain of `local:` includes read, and the most files read for one push.
const DEEPEST: usize = 8;
const MOST_FILES: usize = 64;

/// The pipeline file GitLab reads where the project names none.
pub const GITLAB_DEFAULT: &str = ".gitlab-ci.yml";

/// **What a push changes in CI files**, as the ask shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CiDiff {
    /// The commit the new tip was compared with.
    pub against: HeadSha,
    /// The CI files the push adds, changes or removes, sorted.
    pub files: Vec<String>,
    /// The changed lines of those files, as git writes a patch. Only CI files are in it.
    pub diff: String,
    /// What the pipeline runs that this repository does not hold (an include of another project,
    /// a remote address, a template, a component), as the pipeline names it, sorted.
    pub outside: Vec<String>,
    /// How many other files the same push changes, which are not CI files and are not shown.
    pub others: usize,
    /// The message of the commit the push makes, as it will be sent.
    pub message: String,
}

/// What a push's tree diff says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushCheck {
    /// The push changes no CI file: the proxy lets it through its ordinary rules.
    NoCiFiles,
    /// The push changes CI files: it is held, with this diff.
    Hold(CiDiff),
    /// The diff could not be computed, or cannot be shown: the push is refused, for this reason.
    Refuse(String),
}

/// **Whether pushing `tip`, as `against`'s successor, touches CI files** on a forge of `kind`.
///
/// `ci_config` is the project's own `ci_config_path` as the forge reports it for this push (GitLab
/// only; `None` where the project sets none), read at each push and never cached.
pub fn check_push(
    kind: Kind,
    ci_config: Option<&str>,
    trees: &dyn Trees,
    against: &HeadSha,
    tip: &HeadSha,
) -> PushCheck {
    check_push_in(kind, ci_config, None, trees, against, tip)
}

/// [`check_push`] for the project at `project` (`group/name`) on a forge: an include of that same
/// project (`project:`, one of its components, a raw address of it) is one of this repository's
/// files, never "outside".
pub fn check_push_in(
    kind: Kind,
    ci_config: Option<&str>,
    project: Option<&str>,
    trees: &dyn Trees,
    against: &HeadSha,
    tip: &HeadSha,
) -> PushCheck {
    let changed = match trees.changed(against, tip) {
        Ok(changed) => changed,
        Err(why) => {
            return PushCheck::Refuse(format!(
                "purlis could not compare the push with {}: {why}",
                short(against)
            ));
        }
    };
    if changed.is_empty() {
        return PushCheck::NoCiFiles;
    }
    let (matcher, outside) = match kind {
        Kind::GitHub => (Matcher::always(), Vec::new()),
        Kind::GitLab => match gitlab_matcher(ci_config, project, trees, against, tip) {
            Ok(found) => found,
            Err(why) => return PushCheck::Refuse(why),
        },
    };
    let total = changed.len();
    let mut files: Vec<String> = changed
        .into_iter()
        .filter(|path| matcher.matches(path))
        .collect();
    files.sort();
    files.dedup();
    if files.is_empty() {
        return PushCheck::NoCiFiles;
    }
    let diff = match trees.patch(against, tip, &files) {
        Ok(diff) => diff,
        Err(why) => {
            return PushCheck::Refuse(format!(
                "purlis could not read what the push changes in {}: {why}",
                files.join(", ")
            ));
        }
    };
    if diff.len() > MOST_DIFF {
        return PushCheck::Refuse(format!(
            "the push changes {} by more than purlis can show ({} KiB at most), so it is not \
             held and not allowed",
            files.join(", "),
            MOST_DIFF / 1024
        ));
    }
    if diff.lines().any(|line| line.starts_with("Binary files ")) {
        // What git would not show as text is what a chat can hide a change in: never held on a
        // diff the person cannot read.
        return PushCheck::Refuse(format!(
            "the push changes {} in a way git shows as binary, so it cannot be shown and is not \
             allowed",
            files.join(", ")
        ));
    }
    if diff.trim().is_empty() {
        // A change git shows no lines for (a mode change, a binary file): nothing to read, so
        // nothing the person could approve knowingly.
        return PushCheck::Refuse(format!(
            "the push changes {} in a way that has no lines to show, so it is not held and not \
             allowed",
            files.join(", ")
        ));
    }
    let others = total.saturating_sub(files.len());
    let message = match trees.message(tip) {
        Ok(message) => message,
        Err(why) => {
            return PushCheck::Refuse(format!(
                "purlis could not read the commit message of {}: {why}",
                short(tip)
            ));
        }
    };
    PushCheck::Hold(CiDiff {
        against: against.clone(),
        files,
        diff,
        outside,
        others,
        message,
    })
}

fn short(sha: &HeadSha) -> &str {
    &sha.as_str()[..12]
}

/// Which paths are CI files.
struct Matcher {
    /// A path that starts with one of these.
    prefixes: Vec<String>,
    /// A path that is exactly one of these.
    exact: Vec<String>,
    /// A path a glob matches.
    globs: Vec<String>,
    /// Every YAML file: set where a pipeline's includes cannot all be resolved (a YAML merge
    /// key builds one), so no changed YAML file can be said not to be one the pipeline runs.
    any_yaml: bool,
}

/// Files beside the workflows that decide what runs or who approves it, on both forges.
const ALWAYS: &[&str] = &[
    ".github/dependabot.yml",
    ".github/dependabot.yaml",
    "renovate.json",
    "renovate.json5",
    ".renovaterc",
    ".renovaterc.json",
    ".renovaterc.json5",
    ".github/renovate.json",
    ".github/renovate.json5",
    ".gitlab/renovate.json",
    "CODEOWNERS",
    ".github/CODEOWNERS",
    "docs/CODEOWNERS",
    ".gitlab/CODEOWNERS",
];

impl Matcher {
    /// What runs, or decides who may approve what runs, on either forge: workflows and the
    /// actions they use, dependency bots' configuration (a bot's pull request is merged by
    /// rules the file sets), and who owns a path.
    fn always() -> Matcher {
        Matcher {
            prefixes: vec![
                ".github/workflows/".to_owned(),
                ".github/actions/".to_owned(),
            ],
            exact: ALWAYS.iter().map(|path| (*path).to_owned()).collect(),
            globs: Vec::new(),
            any_yaml: false,
        }
    }

    fn matches(&self, path: &str) -> bool {
        self.prefixes.iter().any(|prefix| path.starts_with(prefix))
            || self.exact.iter().any(|exact| exact == path)
            || self.globs.iter().any(|glob| glob_matches(glob, path))
            || (self.any_yaml && (path.ends_with(".yml") || path.ends_with(".yaml")))
    }
}

/// What GitLab's `include:` names, in this repository and outside it.
#[derive(Debug, Default)]
struct Includes {
    local: Vec<String>,
    outside: Vec<String>,
    /// An include a YAML merge key (`<<`) builds: it cannot be read from the tree as parsed.
    unresolved: bool,
    /// This project's path, lowercased, where it is known: an include of it is a local one.
    project: Option<String>,
}

fn gitlab_matcher(
    ci_config: Option<&str>,
    project: Option<&str>,
    trees: &dyn Trees,
    against: &HeadSha,
    tip: &HeadSha,
) -> Result<(Matcher, Vec<String>), String> {
    let mut roots = vec![GITLAB_DEFAULT.to_owned()];
    if let Some(path) = ci_config {
        // `path@group/project`, an address and a file this repository does not hold are not one of
        // its files; a path it does hold is.
        if !path.contains('@') && !path.contains("://") && !path.is_empty() {
            roots.push(path.trim_start_matches('/').to_owned());
        }
    }
    let mut matcher = Matcher::always();
    matcher.exact.extend(roots.iter().cloned());
    let mut outside = Vec::new();
    // Read at the tip and at the base: an include the push removes is a change of the root file,
    // and a file the base included is still one a push of it changes.
    for commit in [tip, against] {
        let mut queue: Vec<(String, usize)> = roots.iter().map(|root| (root.clone(), 0)).collect();
        let mut read = 0;
        while let Some((path, depth)) = queue.pop() {
            if read >= MOST_FILES || depth > DEEPEST {
                return Err(format!(
                    "the pipeline of this project includes more files than purlis reads ({MOST_FILES}), so the push is not held and not allowed"
                ));
            }
            let Some(text) = trees.read(commit, &path).map_err(|why| {
                format!("purlis could not read {path} at {}: {why}", short(commit))
            })?
            else {
                continue;
            };
            read += 1;
            let found = includes_of_in(&text, project).map_err(|why| {
                format!(
                    "purlis could not read the includes of {path} at {}: {why}, so the push is not held and not allowed",
                    short(commit)
                )
            })?;
            outside.extend(found.outside);
            if found.unresolved {
                matcher.any_yaml = true;
                outside.push(
                    "uses YAML purlis does not resolve (a merge key, a tag, an unusual key or \
                     character): every YAML file of the push is read as a pipeline file"
                        .to_owned(),
                );
            }
            for local in found.local {
                if local.contains(['*', '?']) {
                    // A glob names the files of the tree it matches: each is read in turn, for
                    // the includes it makes.
                    let files = trees.files(commit).map_err(|why| {
                        format!(
                            "purlis could not list the files at {}: {why}",
                            short(commit)
                        )
                    })?;
                    for file in files.into_iter().filter(|file| glob_matches(&local, file)) {
                        if !matcher.exact.contains(&file) {
                            matcher.exact.push(file.clone());
                            queue.push((file, depth + 1));
                        }
                    }
                    if !matcher.globs.contains(&local) {
                        matcher.globs.push(local);
                    }
                    continue;
                }
                if !matcher.exact.contains(&local) {
                    matcher.exact.push(local.clone());
                    queue.push((local, depth + 1));
                }
            }
        }
    }
    outside.sort();
    outside.dedup();
    Ok((matcher, outside))
}

/// **What a pipeline file includes**: `include:` at the top, and `trigger: include:` of a job.
/// A `local:` path with a variable in it cannot be resolved, and is an error.
#[cfg(test)]
fn includes_of(text: &str) -> Result<Includes, String> {
    includes_of_in(text, None)
}

fn includes_of_in(text: &str, project: Option<&str>) -> Result<Includes, String> {
    use yaml_rust2::{Yaml, YamlLoader};
    let mut found = Includes {
        project: project.map(|name| name.trim_matches('/').to_ascii_lowercase()),
        ..Includes::default()
    };
    let Some(body) = strict_subset(text)? else {
        found.unresolved = true;
        return Ok(found);
    };
    let docs = YamlLoader::load_from_str(body).map_err(|why| why.to_string())?;
    for doc in &docs {
        let Yaml::Hash(top) = doc else { continue };
        // A merge key at **any depth**: `<<: *anchor` copies the anchor's keys into a job, so a
        // job can get `trigger: include:` from an anchor this reading would never see in it.
        if has_merge_key(doc) {
            found.unresolved = true;
        }
        for (key, value) in top {
            if key.as_str() == Some("include") {
                one_include(value, &mut found)?;
            } else if let Yaml::Hash(job) = value
                && let Some(Yaml::Hash(trigger)) = job.get(&Yaml::String("trigger".to_owned()))
                && let Some(include) = trigger.get(&Yaml::String("include".to_owned()))
            {
                one_include(include, &mut found)?;
            }
        }
    }
    Ok(found)
}

/// **Whether a pipeline file sits in the strict, boring subset of YAML that purlis and GitLab
/// (Psych over libyaml) read the same way.** Four rounds each found another key the two readers
/// spell differently; the answer is not another spelling but this one gate. Outside it the file is
/// *unresolved* (`Ok(None)`): every YAML file of the push then counts as a CI file, so the cost is
/// an extra ask and never a hidden include.
///
/// Inside it (`Ok(Some(body))`, the text to parse) all of these hold:
///
/// 1. The text is valid UTF-8 (the reader of a file refuses anything else before this sees it,
///    [`GitTrees`]), so no UTF-16 or UTF-32 file is read.
/// 2. At most one U+FEFF, and only at the start; it is stripped (libyaml strips it, this parser
///    would glue it to the first key, so a BOM before `include:` hid it).
/// 3. No control character but tab, LF and CR: no other C0, no DEL, no C1 (U+0085 among them), no
///    U+2028 or U+2029, no U+FFFE or U+FFFF. A form feed or vertical tab splits GitLab's textual
///    `---` handling; NUL, NEL, LS and PS end lines for one reader and not the other.
/// 4. No explicit tag of any kind (`!!null` on a plain key is a non-string here and a string in
///    Psych), no `%` directive, and no `!reference` text.
/// 5. No mapping key starts with `:` (Psych reads it as a Ruby Symbol and GitLab symbolizes keys).
/// 6. No `$[[` (spec:inputs interpolation can turn a key into `include` or `trigger`).
/// 7. A line that starts with `---` is exactly `---`, or `--- ` and content with no tab.
/// 8. Every mapping key is a scalar: no complex key, no alias as a key.
///
/// `Err` is a file that does not parse, or whose aliases expand past a bound.
fn strict_subset(text: &str) -> Result<Option<&str>, String> {
    // Rule 2.
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    // Rules 2, 3 and 6.
    if body.contains("$[[")
        || body.chars().any(|c| {
            matches!(
                c,
                '\u{feff}' | '\u{2028}' | '\u{2029}' | '\u{7f}' | '\u{fffe}' | '\u{ffff}'
            ) || (c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
        })
    {
        return Ok(None);
    }
    // Rules 4 (text) and 7. A lone CR ends a line for libyaml, so it splits one here too.
    if body.contains("!reference")
        || body.split(['\n', '\r']).any(|line| {
            line.starts_with('%')
                || line.strip_prefix("---").is_some_and(|rest| {
                    !(rest.is_empty() || (rest.starts_with(' ') && !rest.contains('\t')))
                })
        })
    {
        return Ok(None);
    }
    // Rules 4, 5 and 8, from the parser's own events.
    if audit(body)?.outside {
        return Ok(None);
    }
    Ok(Some(body))
}

/// The most nodes a pipeline file may expand to once its aliases are resolved, and the most bytes
/// of one file read: past either, the push is refused.
const MOST_NODES: usize = 200_000;
/// The most bytes the scalars of a file may expand to once aliases are resolved.
const MOST_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MOST_FILE: usize = 1024 * 1024;
/// The most bytes of a listing of a tree's files, or of the paths a diff changes.
pub(super) const MOST_LISTING: usize = 16 * 1024 * 1024;

#[derive(Default)]
struct Audit {
    /// Per open collection: its anchor (0 for none), and the nodes and scalar bytes it holds so far.
    open: Vec<(usize, usize, usize)>,
    /// The nodes and scalar bytes each anchored node expands to.
    sizes: std::collections::HashMap<usize, (usize, usize)>,
    /// Per open collection: whether it is a mapping, and how many children it has had.
    kinds: Vec<(bool, usize)>,
    /// A tag, a key starting with `:`, or a key that is not a scalar: outside the subset.
    outside: bool,
    over: bool,
}

impl Audit {
    fn add(&mut self, nodes: usize, bytes: usize) {
        if let Some(top) = self.open.last_mut() {
            top.1 = top.1.saturating_add(nodes);
            top.2 = top.2.saturating_add(bytes);
            self.over |= top.1 > MOST_NODES || top.2 > MOST_BYTES;
        } else {
            self.over |= nodes > MOST_NODES || bytes > MOST_BYTES;
        }
    }

    /// Any explicit tag at all is outside the subset: the readers disagree on `!!null`, `!!bool`,
    /// `!!int` of a plain key, and decode `!!binary`, `!!omap`, `!!pairs` and `!!set`.
    fn tag(&mut self, tag: &Option<yaml_rust2::parser::Tag>) {
        self.outside |= tag.is_some();
    }

    /// A node begins: where it is a mapping's key it must be a scalar not starting with `:`.
    fn child(&mut self, key: Option<&str>) {
        if let Some((is_map, seen)) = self.kinds.last_mut() {
            if *is_map && *seen % 2 == 0 {
                self.outside |= key.is_none_or(|key| key.starts_with(':'));
            }
            *seen += 1;
        }
    }
}

impl yaml_rust2::parser::EventReceiver for Audit {
    fn on_event(&mut self, event: yaml_rust2::parser::Event) {
        use yaml_rust2::parser::Event;
        if self.over {
            return;
        }
        let is_map = matches!(event, Event::MappingStart(..));
        match event {
            Event::Scalar(value, _, anchor, tag) => {
                self.tag(&tag);
                self.child(Some(&value));
                let bytes = value.len();
                if anchor > 0 {
                    self.sizes.insert(anchor, (1, bytes));
                }
                self.add(1, bytes);
            }
            Event::SequenceStart(anchor, tag) | Event::MappingStart(anchor, tag) => {
                self.tag(&tag);
                self.child(None);
                self.kinds.push((is_map, 0));
                self.open.push((anchor, 1, 0));
            }
            Event::SequenceEnd | Event::MappingEnd => {
                self.kinds.pop();
                if let Some((anchor, nodes, bytes)) = self.open.pop() {
                    if anchor > 0 {
                        self.sizes.insert(anchor, (nodes, bytes));
                    }
                    self.add(nodes, bytes);
                }
            }
            Event::Alias(anchor) => {
                self.child(None);
                let (nodes, bytes) = self.sizes.get(&anchor).copied().unwrap_or((1, 0));
                self.add(nodes, bytes);
            }
            _ => {}
        }
    }
}

/// **A pipeline file read once without expanding anything**: its tags, and how large its aliases
/// would make it (a chat writes the file, and the loader copies an anchor at every alias, so a
/// few nested ones are a billion nodes).
fn audit(text: &str) -> Result<Audit, String> {
    let mut audit = Audit::default();
    yaml_rust2::parser::Parser::new_from_str(text)
        .load(&mut audit, true)
        .map_err(|why| why.to_string())?;
    if audit.over {
        return Err(format!(
            "its anchors and aliases expand to more than {MOST_NODES} nodes or {MOST_BYTES} bytes"
        ));
    }
    Ok(audit)
}

/// Whether a YAML merge key (`<<`) appears anywhere in `yaml`.
fn has_merge_key(yaml: &yaml_rust2::Yaml) -> bool {
    use yaml_rust2::Yaml;
    match yaml {
        Yaml::Hash(map) => map.iter().any(|(key, value)| {
            key.as_str() == Some("<<") || has_merge_key(key) || has_merge_key(value)
        }),
        Yaml::Array(items) => items.iter().any(has_merge_key),
        _ => false,
    }
}

fn one_include(value: &yaml_rust2::Yaml, found: &mut Includes) -> Result<(), String> {
    use yaml_rust2::Yaml;
    match value {
        Yaml::String(text) => string_include(text, found),
        Yaml::Array(items) => items.iter().try_for_each(|item| one_include(item, found)),
        Yaml::Hash(map) => {
            if map.contains_key(&Yaml::String("<<".to_owned())) {
                found.unresolved = true;
            }
            let get = |name: &str| map.get(&Yaml::String(name.to_owned()));
            if let Some(local) = get("local") {
                match local {
                    Yaml::String(path) => local_include(path, found)?,
                    Yaml::Array(paths) => {
                        for path in paths {
                            match path.as_str() {
                                Some(path) => local_include(path, found)?,
                                None => return Err("a local include is not a path".to_owned()),
                            }
                        }
                    }
                    _ => return Err("a local include is not a path".to_owned()),
                }
            }
            if let Some(Yaml::String(named)) = get("remote") {
                remote_include(named, found)?;
            }
            if let Some(Yaml::String(named)) = get("template") {
                found.outside.push(format!("a template: {named}"));
            }
            if let Some(Yaml::String(named)) = get("component") {
                component_include(named, found);
            }
            if let Some(Yaml::String(named)) = get("project") {
                project_include(named, get("file"), found);
            }
            if get("artifact").is_some() {
                found
                    .outside
                    .push("a pipeline another job generates".to_owned());
            }
            Ok(())
        }
        _ => Err("an include is not a path or a list".to_owned()),
    }
}

fn string_include(text: &str, found: &mut Includes) -> Result<(), String> {
    if text.starts_with("http://") || text.starts_with("https://") {
        remote_include(text, found)
    } else {
        local_include(text, found)
    }
}

/// A `project:` include: of another project it is outside; of **this** one it is the named
/// `file`s of this repository, and where they cannot be named the pipeline is unresolved.
fn project_include(named: &str, file: Option<&yaml_rust2::Yaml>, found: &mut Includes) {
    use yaml_rust2::Yaml;
    if named.contains('$') {
        // `$CI_PROJECT_PATH` is this project and anything else may be: not decidable.
        found.unresolved = true;
    }
    let this = found.project.as_deref().is_some_and(|project| {
        named
            .trim_matches('/')
            .trim_end_matches(".git")
            .eq_ignore_ascii_case(project)
    });
    if !this {
        found.outside.push(format!("another project: {named}"));
        return;
    }
    let files: Option<Vec<&str>> = match file {
        Some(Yaml::String(path)) => Some(vec![path.as_str()]),
        Some(Yaml::Array(paths)) => paths.iter().map(Yaml::as_str).collect(),
        _ => None,
    };
    match files {
        Some(files) if !files.iter().any(|path| path.contains('$')) => {
            found.local.extend(
                files
                    .iter()
                    .map(|path| path.trim_start_matches('/').to_owned()),
            );
        }
        _ => found.unresolved = true,
    }
}

/// A `component:` include (`host/group/project/name@version`): of **this** project it is
/// `templates/name.yml` or `templates/name/template.yml` of this repository.
fn component_include(named: &str, found: &mut Includes) {
    if named.contains('$') {
        found.unresolved = true;
    }
    let address = named.split('@').next().unwrap_or(named);
    let path = address
        .split_once('/')
        .map_or("", |(_, path)| path)
        .to_ascii_lowercase();
    let own = found
        .project
        .as_deref()
        .and_then(|project| path.strip_prefix(&format!("{project}/")))
        .map(str::to_owned);
    match own {
        Some(name) if !name.is_empty() && !name.contains('$') => {
            found.local.push(format!("templates/{name}.yml"));
            found.local.push(format!("templates/{name}/template.yml"));
        }
        Some(_) => found.unresolved = true,
        None => found.outside.push(format!("a component: {named}")),
    }
}

/// A `remote:` include: outside, unless it is a raw address of **this** repository
/// (`…/group/project/-/raw/<ref>/<path>`), which is the files its `<path>` could be (a ref with a
/// slash in it leaves the split uncertain, so every tail is one).
fn remote_include(url: &str, found: &mut Includes) -> Result<(), String> {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = after_scheme.split_once('/').map_or("", |(_, path)| path);
    let lower = path.to_ascii_lowercase();
    let Some(project) = found.project.clone() else {
        found.outside.push(format!("a remote address: {url}"));
        return Ok(());
    };
    let marker = [format!("{project}/-/raw/"), format!("{project}/raw/")]
        .into_iter()
        .find(|marker| lower.starts_with(marker.as_str()));
    let Some(marker) = marker else {
        let name = project.rsplit('/').next().unwrap_or(&project);
        if lower.contains('%') && lower.contains(name) {
            // An encoded spelling of this project's address cannot be told from another's.
            found.unresolved = true;
        }
        found.outside.push(format!("a remote address: {url}"));
        return Ok(());
    };
    // Paths are case-sensitive: the tail is read from the original spelling.
    let after = path[marker.len()..].split(['?', '#']).next().unwrap_or("");
    if after.contains(['$', '%']) {
        found.unresolved = true;
        return Ok(());
    }
    let mut any = false;
    let mut rest = after;
    while let Some((_, deeper)) = rest.split_once('/') {
        if !deeper.is_empty() {
            local_include(deeper, found)?;
            any = true;
        }
        rest = deeper;
    }
    if !any {
        found.unresolved = true;
    }
    Ok(())
}

fn local_include(path: &str, found: &mut Includes) -> Result<(), String> {
    if path.contains('$') {
        return Err(format!(
            "the include {path} has a variable in its path, which purlis cannot resolve"
        ));
    }
    found.local.push(path.trim_start_matches('/').to_owned());
    Ok(())
}

/// GitLab's glob for a `local:` include: `*` is any run of characters within one folder, `**`
/// any run across folders, `?` one character. **Linear in the glob times the path** (a table
/// over the two, never a backtrack), so `**a**a**a**b` against a long path is no hang.
fn glob_matches(glob: &str, path: &str) -> bool {
    enum Token {
        Lit(u8),
        One,
        Within,
        Across,
    }
    let glob = glob.trim_start_matches('/').as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < glob.len() {
        match glob[at] {
            b'*' if glob.get(at + 1) == Some(&b'*') => {
                at += 2;
                if glob.get(at) == Some(&b'/') {
                    at += 1;
                }
                tokens.push(Token::Across);
            }
            b'*' => {
                at += 1;
                tokens.push(Token::Within);
            }
            b'?' => {
                at += 1;
                tokens.push(Token::One);
            }
            c => {
                at += 1;
                tokens.push(Token::Lit(c));
            }
        }
    }
    let path = path.as_bytes();
    // `next[j]`: the tokens after this one match `path[j..]`.
    let mut next = vec![false; path.len() + 1];
    next[path.len()] = true;
    for token in tokens.iter().rev() {
        let mut row = vec![false; path.len() + 1];
        for j in (0..=path.len()).rev() {
            row[j] = match token {
                Token::Lit(c) => path.get(j) == Some(c) && next[j + 1],
                Token::One => path.get(j).is_some_and(|d| *d != b'/') && next[j + 1],
                Token::Within => next[j] || (path.get(j).is_some_and(|d| *d != b'/') && row[j + 1]),
                Token::Across => next[j] || (j < path.len() && row[j + 1]),
            };
        }
        next = row;
    }
    next[0]
}

/// **The trees of a clone, read by git** (the standard tool): the diff of two commits' trees, the
/// patch of some paths of it, and a file at a commit.
pub struct GitTrees {
    clone: std::path::PathBuf,
}

impl GitTrees {
    /// Trees of the repository at `store`, **which only purlis writes**: the quarantine
    /// ([`super::Quarantine`]) or a test's own repository. **Never a chat's clone**, whose objects
    /// a chat can overwrite so that this shows other lines than the commit the forge receives;
    /// `no_chat_clone_is_read_for_a_held_act` fails on any other use in the source.
    pub fn of_trusted_store(store: impl Into<std::path::PathBuf>) -> GitTrees {
        let clone = store;
        GitTrees {
            clone: clone.into(),
        }
    }
}

impl GitTrees {
    /// git, read with nothing of the clone's own to say about what it shows: **replace refs are
    /// not followed** (a `refs/replace/<sha>` makes git read another object for a commit's id, so
    /// a tree diff of the real commits would be empty), and every call that shows a patch is
    /// `--text` (a `-diff` attribute in `.git/info/attributes` or a `.gitattributes` makes git
    /// print "Binary files differ" for a file whose lines are the change).
    ///
    /// Its output is held to `most` bytes: past them git is stopped and the answer is an error, so
    /// a huge patch, listing or blob is never held whole.
    pub(super) fn git_capped(&self, args: &[&str], most: usize) -> Result<String, String> {
        self.git_capped_bytes(args, most)
            .map(|out| String::from_utf8_lossy(&out).into_owned())
    }

    /// [`Self::git_capped`], the answer as git wrote it.
    fn git_capped_bytes(&self, args: &[&str], most: usize) -> Result<Vec<u8>, String> {
        use crate::worktree::git;
        let mut argv = vec![
            "--no-replace-objects",
            "-c",
            "core.attributesFile=/dev/null",
        ];
        argv.extend_from_slice(args);
        let done = git::run_capped_bytes(&self.clone, &argv, git::READ, most)
            .map_err(|why| why.to_string())?;
        let Some(done) = done else {
            return Err(format!(
                "git's answer is more than {most} bytes, more than purlis can show"
            ));
        };
        if done.code == Some(0) {
            Ok(done.out)
        } else {
            Err(String::from_utf8_lossy(&done.err)
                .trim()
                .lines()
                .next()
                .unwrap_or("git failed")
                .to_owned())
        }
    }
}

impl Trees for GitTrees {
    fn changed(&self, against: &HeadSha, tip: &HeadSha) -> Result<Vec<String>, String> {
        // Both objects must be here: a commit the clone lacks has no tree to compare.
        for sha in [against, tip] {
            self.git_capped(
                &["cat-file", "-e", &format!("{}^{{commit}}", sha.as_str())],
                1024,
            )
            .map_err(|why| format!("the clone has no commit {}: {why}", short(sha)))?;
        }
        let out = self.git_capped(
            &[
                "diff-tree",
                "-r",
                "-z",
                "--raw",
                "--text",
                "--no-renames",
                "--no-textconv",
                "--no-ext-diff",
                against.as_str(),
                tip.as_str(),
            ],
            MOST_LISTING,
        )?;
        let mut paths = Vec::new();
        let mut tokens = out.split('\0').filter(|token| !token.is_empty());
        while let Some(meta) = tokens.next() {
            if !meta.starts_with(':') {
                return Err("git's diff was not in the form purlis reads".to_owned());
            }
            let path = tokens
                .next()
                .ok_or_else(|| "git's diff ended inside an entry".to_owned())?;
            if path.contains('\u{fffd}') {
                return Err("a changed path is not text purlis can read".to_owned());
            }
            paths.push(path.to_owned());
        }
        Ok(paths)
    }

    fn patch(&self, against: &HeadSha, tip: &HeadSha, paths: &[String]) -> Result<String, String> {
        let mut args = vec![
            "--literal-pathspecs",
            "diff-tree",
            "-r",
            "-p",
            "--text",
            "--no-renames",
            "--no-textconv",
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
            against.as_str(),
            tip.as_str(),
            "--",
        ];
        args.extend(paths.iter().map(String::as_str));
        // Stopped past what can be shown, never buffered whole first.
        self.git_capped(&args, MOST_DIFF + 1).and_then(|out| {
            if out.len() > MOST_DIFF {
                Err(format!(
                    "the diff is more than purlis can show ({} KiB at most)",
                    MOST_DIFF / 1024
                ))
            } else {
                Ok(out)
            }
        })
    }

    fn message(&self, tip: &HeadSha) -> Result<String, String> {
        super::TipFiles::message(self, tip)
    }

    fn files(&self, commit: &HeadSha) -> Result<Vec<String>, String> {
        let out = self.git_capped(
            &["ls-tree", "-r", "-z", "--name-only", commit.as_str()],
            MOST_LISTING,
        )?;
        Ok(out
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect())
    }

    fn read(&self, commit: &HeadSha, path: &str) -> Result<Option<String>, String> {
        let at = format!("{}:{path}", commit.as_str());
        let listed = self.git_capped(
            &[
                "--literal-pathspecs",
                "ls-tree",
                "-z",
                commit.as_str(),
                "--",
                path,
            ],
            64 * 1024,
        )?;
        if listed.is_empty() {
            return Ok(None);
        }
        // A pipeline file is what a chat writes: never read past a bound, never held whole.
        let bytes = self
            .git_capped_bytes(&["cat-file", "blob", &at], MOST_FILE)
            .map_err(|why| {
                if why.contains("more than") {
                    format!("{path} is more than {} KiB", MOST_FILE / 1024)
                } else {
                    why
                }
            })?;
        // Never a lossy reading: a UTF-16 or UTF-32 file, or any bytes that are not UTF-8, is a
        // file the two YAML readers may read as different text (`strict_subset` rule 1).
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("{path} is not valid UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glob_with_many_double_stars_against_a_long_path_finishes_at_once() {
        let path = format!("{}b", "a/".repeat(2_000));
        let started = std::time::Instant::now();
        assert!(!glob_matches("**a**a**a**a**a**c", &path));
        assert!(glob_matches("**a**a**a**a**a**b", &path));
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn a_glob_keeps_gitlabs_meaning() {
        assert!(glob_matches("ci/*.yml", "ci/a.yml"));
        assert!(!glob_matches("ci/*.yml", "ci/sub/a.yml"));
        assert!(glob_matches("ci/**/*.yml", "ci/sub/deep/a.yml"));
        assert!(glob_matches("ci/**/*.yml", "ci/a.yml"));
        assert!(glob_matches("a?c", "abc"));
        assert!(!glob_matches("a?c", "a/c"));
    }

    #[test]
    fn a_reference_tag_anywhere_makes_every_yaml_file_a_pipeline_file() {
        let found = includes_of(
            "build:\n  trigger:\n    include:\n      - !reference [.shared, include]\n",
        )
        .unwrap();
        assert!(found.unresolved);
        assert!(
            !includes_of("build:\n  script: [make]\n")
                .unwrap()
                .unresolved
        );
    }

    #[test]
    fn an_encoded_or_redirected_reference_tag_is_as_unresolvable_as_the_plain_one() {
        // libyaml resolves each of these to `!reference`; this parser reads them as other tags.
        for text in [
            "build:\n  trigger:\n    include: !ref%65rence [x, y]\n",
            "%TAG !e! !refer\n---\nbuild:\n  trigger:\n    include: !e!ence [x, y]\n",
            "%TAG ! !refer\n---\nbuild:\n  trigger:\n    include: !ence [x, y]\n",
            "build:\n  trigger:\n    include: !custom [x, y]\n",
        ] {
            assert!(includes_of(text).unwrap().unresolved, "{text}");
        }
        // No tag at all is in the strict subset, a core one included.
        assert!(
            includes_of("build:\n  retry: !!int 2\n  script: [make]\n")
                .unwrap()
                .unresolved
        );
    }

    #[test]
    fn aliases_that_expand_past_a_bound_refuse_the_file_before_anything_is_built() {
        let mut text = String::from("a0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
        for level in 1..9 {
            let before = level - 1;
            let refs = vec![format!("*a{before}"); 10].join(", ");
            text.push_str(&format!("a{level}: &a{level} [{refs}]\n"));
        }
        let why = includes_of(&text).unwrap_err();
        assert!(why.contains("expand"), "{why}");
        assert!(includes_of("a: &a [x]\nb: *a\n").is_ok());
    }

    #[test]
    fn a_decoded_core_tag_hides_nothing_from_the_reader() {
        for text in [
            "!!binary aW5jbHVkZQ==: [evil.yml]\n",
            "build:\n  !!binary dHJpZ2dlcg==:\n    include: [evil.yml]\n",
            "include: !!binary ZXZpbC55bWw=\n",
            "a: &x {k: v}\nb:\n  !!binary PDw=: *x\n",
            "--- !!omap\n- include: evil.yml\n",
            "build:\n  trigger: !!omap\n    - include: evil.yml\n",
            "a: !!set {x}\n",
            "a: !!pairs [[x, y]]\n",
        ] {
            assert!(includes_of(text).unwrap().unresolved, "{text}");
        }
        for tag in ["str", "int", "float", "bool", "null", "seq", "map"] {
            let text = format!("a: !!{tag} x\n");
            assert!(includes_of(&text).unwrap().unresolved, "{text}");
        }
    }

    fn unresolved(text: &str) -> bool {
        includes_of(text).unwrap().unresolved
    }

    #[test]
    fn a_core_tag_on_a_key_that_psych_keeps_a_string_hides_nothing() {
        for tag in ["null", "bool", "int"] {
            for text in [
                format!("!!{tag} include: [evil.yml]\n"),
                format!("build:\n  !!{tag} trigger:\n    include: [evil.yml]\n"),
                format!("a: &x {{k: v}}\nb:\n  !!{tag} <<: *x\n"),
            ] {
                assert!(unresolved(&text), "{text}");
            }
        }
    }

    #[test]
    fn a_byte_order_mark_before_include_is_stripped_and_the_include_is_read() {
        let found = includes_of("\u{feff}include: [ci/a.yml]\n").unwrap();
        assert_eq!(found.local, vec!["ci/a.yml".to_owned()]);
        assert!(!found.unresolved);
        // A second one, or one inside the file, is outside the subset.
        assert!(unresolved("\u{feff}\u{feff}include: [ci/a.yml]\n"));
        assert!(unresolved("a: 1\n\u{feff}include: [ci/a.yml]\n"));
    }

    #[test]
    fn a_control_character_other_than_tab_lf_and_cr_is_unresolved() {
        for c in [
            '\0', '\u{1}', '\u{b}', '\u{c}', '\u{1b}', '\u{7f}', '\u{90}', '\u{fffe}',
        ] {
            let text = format!("a: x{c}y\ninclude: [ci/a.yml]\n");
            assert!(unresolved(&text), "{text:?}");
        }
        assert!(!unresolved("a: x\t# c\r\ninclude: [ci/a.yml]\r\n"));
    }

    #[test]
    fn a_key_that_starts_with_a_colon_is_unresolved_everywhere_a_key_can_be() {
        for text in [
            ":include: [evil.yml]\n",
            "build:\n  :trigger:\n    include: [evil.yml]\n",
            "build:\n  trigger:\n    :include: [evil.yml]\n",
            "include:\n  - :local: evil.yml\n",
            "include:\n  - ':local': evil.yml\n",
            "{:include: [evil.yml]}\n",
        ] {
            assert!(unresolved(text), "{text}");
        }
        // A colon inside a value, or later in a key, is plain text for both.
        assert!(!unresolved("a: ':x'\nb:c: [y]\n"));
    }

    #[test]
    fn a_document_marker_followed_by_other_whitespace_is_unresolved() {
        for gap in ["\u{c}", "\u{b}", "\t", " \t"] {
            let text = format!("a: 1\n---{gap}\ninclude: [evil.yml]\n");
            assert!(unresolved(&text), "{text:?}");
        }
        assert!(unresolved("a: 1\n---include: [evil.yml]\n"));
        assert!(unresolved("a: 1\r---\u{c}\rinclude: [evil.yml]\n"));
        assert!(!unresolved("a: 1\n---\ninclude: [ci/a.yml]\n"));
        assert!(!unresolved("a: 1\r\n---\r\ninclude: [ci/a.yml]\r\n"));
        assert!(!unresolved("a: 1\n--- # next\ninclude: [ci/a.yml]\n"));
    }

    #[test]
    fn spec_inputs_interpolation_is_unresolved_wherever_it_stands() {
        for text in [
            "$[[ inputs.x ]]: [evil.yml]\n",
            "build:\n  trigger:\n    $[[ inputs.k ]]: [evil.yml]\n",
            "a: $[[ inputs.x ]]\n",
        ] {
            assert!(unresolved(text), "{text}");
        }
    }

    #[test]
    fn a_key_that_is_not_a_scalar_is_unresolved() {
        for text in [
            "? [include]\n: evil.yml\n",
            "? {a: b}\n: evil.yml\n",
            "k: &k include\n*k : [evil.yml]\n",
        ] {
            assert!(unresolved(text), "{text}");
        }
        assert!(!unresolved(
            "'include': [ci/a.yml]\n\"build\": {script: [x]}\n"
        ));
    }

    #[test]
    fn an_ordinary_pipeline_file_is_in_the_subset() {
        let text = ".base: &base\n  image: ruby\nbuild:\n  <<: *base\n  script: [make]\n";
        // A merge key is unresolved for its own reason; everything else about it is in.
        assert!(strict_subset(text).unwrap().is_some());
        assert!(
            strict_subset("stages: [build]\nbuild:\n  script:\n    - echo \"hi!\"\n")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn this_projects_own_project_component_and_raw_includes_are_local_files() {
        let found = includes_of_in(
            "include:\n  - project: Group/Sub/App\n    file: [/ci/a.yml, ci/b.yml]\n  - component: gitlab.example.com/group/sub/app/build@1.0\n  - remote: https://gitlab.example.com/group/sub/app/-/raw/main/ci/c.yml\n  - remote: https://gitlab.example.com/group/sub/app/-/raw/feature/x/ci/d.yml\n",
            Some("group/sub/app"),
        )
        .unwrap();
        for path in [
            "ci/a.yml",
            "ci/b.yml",
            "templates/build.yml",
            "templates/build/template.yml",
            "ci/c.yml",
            "ci/d.yml",
        ] {
            assert!(
                found.local.contains(&path.to_owned()),
                "{path}: {:?}",
                found.local
            );
        }
        assert!(found.outside.is_empty(), "{:?}", found.outside);
        assert!(!found.unresolved);
    }

    #[test]
    fn another_projects_includes_stay_outside_and_an_undeterminable_own_one_is_unresolved() {
        let found = includes_of_in(
            "include:\n  - project: group/other\n    file: x.yml\n  - component: gitlab.example.com/group/other/build@1\n  - remote: https://example.com/group/other/-/raw/main/x.yml\n",
            Some("group/app"),
        )
        .unwrap();
        assert_eq!(found.outside.len(), 3, "{:?}", found.outside);
        assert!(found.local.is_empty() && !found.unresolved);
        for text in [
            "include:\n  - project: group/app\n",
            "include:\n  - project: group/app\n    file: /ci/$X.yml\n",
            "include:\n  - project: $CI_PROJECT_PATH\n    file: a.yml\n",
            "include:\n  - component: $CI_SERVER_FQDN/group/app/build@1\n",
            "include:\n  - remote: https://x.example/group/app/-/raw/main\n",
        ] {
            assert!(
                includes_of_in(text, Some("group/app")).unwrap().unresolved,
                "{text}"
            );
        }
    }

    #[test]
    fn aliases_that_expand_past_a_bound_of_bytes_refuse_the_file() {
        let big = "x".repeat(300_000);
        let refs = vec!["*a"; 1000].join(", ");
        let text = format!("a: &a \"{big}\"\nb: [{refs}]\n");
        let why = includes_of(&text).unwrap_err();
        assert!(why.contains("bytes"), "{why}");
        assert!(includes_of("a: &a \"xxx\"\nb: [*a, *a]\n").is_ok());
    }

    #[test]
    fn a_line_break_only_libyaml_knows_makes_the_file_unresolved() {
        for mark in ['\u{85}', '\u{2028}', '\u{2029}'] {
            for text in [
                format!("a: 1 # c{mark}include: [evil.yml]\n"),
                format!("a: >\n  one{mark}include: [evil.yml]\n"),
            ] {
                assert!(includes_of(&text).unwrap().unresolved, "{text:?}");
            }
        }
    }
}
