// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No multi-step git operation runs at a plane root** (#5824 F2, D2).
//!
//! A `rebase`, a `merge` that makes a merge, a `cherry-pick`, an `am`, a `revert` or a `pull`
//! writes its state into the git directory, then works step by step. A quit, a crash or sleep in
//! the middle leaves the root part-way, and on 2026-11-07 one did: a settle's `git rebase` killed
//! mid-pick left the plane on a detached HEAD, and every save after it refused. So the root
//! changes only by a commit and by one `reset --keep` (or `symbolic-ref` and a mixed reset), and
//! every merge is made off to the side with `merge-tree` and `commit-tree`.
//!
//! This reads the code that runs git at a plane root (`planegit` and the callers that hold a
//! plane's claim) and fails on any such verb outside [`ALLOWED`]. A line there that stops being
//! found fails too, so the list only shrinks. Allowed without a line of their own:
//!
//! - `merge --ff-only`: one checkout, as `reset --keep` is. It never makes a merge, so it
//!   never leaves `MERGE_HEAD`.
//! - `rebase --abort` and `rebase --quit`: one step that puts a stop away.

use std::path::{Path, PathBuf};

/// The verbs that start an operation git works through step by step.
const MULTI_STEP: [&str; 6] = ["rebase", "merge", "cherry-pick", "am", "revert", "pull"];

/// Where a verb's name is a step named in the root step marker (`recover.rs`), not git run.
const NOT_GIT: [&str; 3] = ["recover::mark(", "\"op\":", "field(\"op\")"];

/// What runs git at a plane root, relative to the crate.
const SCANNED: [&str; 4] = [
    "src/planegit.rs",
    "src/planegit",
    "src/autosave.rs",
    "src/doctor/fix",
];

/// The lines still allowed, each with why. Every one is a debt with somewhere to go.
const ALLOWED: [(&str, &str, &str); 1] = [(
    "src/planegit.rs",
    r#"rebase.extend(["rebase", onto]);"#,
    "rebase_onto: a push-mode save's retry, Update and Resolve replay the plane's own commits \
     with it, and a conflict stops it for Resolve to settle. Resolve moves off to the side in \
     #5824 F7 (lane C); until then the step is marked, and one cut off is recovered (F3)",
)];

/// Whole files allowed, each with why.
const ALLOWED_FILES: [(&str, &str); 1] = [(
    "src/planegit/resolve.rs",
    "Resolve settles a conflict in a merge or rebase stopped at the root. #5824 F7 (lane C) \
     replaces it with a resolve folder off to the side",
)];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every Rust file under `path`, a file or a folder.
fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for entry in entries {
        rust_files(&entry, out);
    }
}

/// Whether `relative` is a file of tests, which runs git as a test's own setup.
fn is_tests(relative: &str) -> bool {
    relative.ends_with("/tests.rs") || relative.ends_with("/twoclones.rs")
}

/// The file's lines up to its inline test module, each without its `//` comment.
fn code_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "#[cfg(test)]"
            && lines
                .peek()
                .is_some_and(|next| next.trim().starts_with("mod tests {"))
        {
            break;
        }
        let code = match trimmed.find("//") {
            // `//` inside a string, as in a URL, is not a comment: kept whole.
            Some(at) if trimmed[..at].matches('"').count() % 2 == 0 => &trimmed[..at],
            _ => trimmed,
        };
        out.push(code.trim().to_owned());
    }
    out
}

/// The multi-step verbs this line runs, with the line, where they are not one of the steps
/// allowed everywhere.
fn offending(lines: &[String], at: usize) -> Option<&str> {
    let line = lines[at].as_str();
    let verb = MULTI_STEP
        .iter()
        .find(|verb| line.contains(&format!("\"{verb}\"")))?;
    // A key of a JSON value, or the name of a step in the root step marker, not a git verb.
    if line.contains(&format!(".get(\"{verb}\")"))
        || line.contains(&format!("[\"{verb}\"]"))
        || NOT_GIT.iter().any(|not| line.contains(not))
    {
        return None;
    }
    let next = lines[at + 1..]
        .iter()
        .find(|next| !next.is_empty())
        .map_or("", String::as_str);
    let with = |flag: &str| {
        line.contains(flag) || (line == format!("\"{verb}\",") && next.starts_with(flag))
    };
    let allowed_everywhere = match *verb {
        "merge" => with("\"--ff-only\""),
        "rebase" => with("\"--abort\"") || with("\"--quit\""),
        _ => false,
    };
    (!allowed_everywhere).then_some(line)
}

#[test]
fn no_rebase_or_merge_runs_at_a_plane_root_but_the_steps_allowed_by_name() {
    let top = crate_dir();
    let mut files = Vec::new();
    for scanned in SCANNED {
        rust_files(&top.join(scanned), &mut files);
    }
    assert!(
        files.len() > 5,
        "the scan found the code it reads: {files:?}"
    );
    let mut found = Vec::new();
    let mut seen = vec![false; ALLOWED.len()];
    for file in &files {
        let relative = file
            .strip_prefix(&top)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if is_tests(&relative) || ALLOWED_FILES.iter().any(|(f, _)| *f == relative) {
            continue;
        }
        let text = std::fs::read_to_string(file).unwrap();
        let lines = code_lines(&text);
        for at in 0..lines.len() {
            let Some(line) = offending(&lines, at) else {
                continue;
            };
            match ALLOWED
                .iter()
                .position(|(f, allowed, _)| *f == relative && *allowed == line)
            {
                Some(i) => seen[i] = true,
                None => found.push(format!("{relative}:{}: {line}", at + 1)),
            }
        }
    }
    assert!(
        found.is_empty(),
        "a multi-step git operation runs at a plane root (#5824 D2): make it off to the side \
         with merge-tree and commit-tree, and move the root with one `reset --keep`:\n{}",
        found.join("\n")
    );
    for (i, (file, line, _)) in ALLOWED.iter().enumerate() {
        assert!(
            seen[i],
            "{file} no longer has `{line}`: take it off ALLOWED"
        );
    }
    for (file, _) in ALLOWED_FILES {
        assert!(
            top.join(file).is_file(),
            "{file} is gone: take it off ALLOWED_FILES"
        );
    }
}
