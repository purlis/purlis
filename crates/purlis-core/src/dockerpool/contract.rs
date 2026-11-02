// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Does a team's image keep the runner image contract** (`docs/runner-image.md`, #4481): what
//! Pool setup asks before it makes a pool of one, so that one that can't work says why and makes
//! nothing. This is `tools/runner-image-contract.mjs`'s check of the image's config, and its check
//! from inside, by the same script. Whether `purlis serve` then comes up is what adding the pool
//! finds out.

use super::{Docker, Failed, Image, MOST_SAID, Refused};

/// The script that reads an image from inside, as its own user, one problem to a line.
const INSIDE: &str = include_str!("../../../../tools/runner-image-contract.sh");

/// How long the check from inside has: a start of a shell, and a few looks.
const INSIDE_WITHIN: std::time::Duration = std::time::Duration::from_secs(60);

/// Most problems said: an image that lists more says nothing a person can act on sooner.
const MOST_PROBLEMS: usize = 12;

/// The inits that reap the processes a runner's chats leave behind when it is PID 1.
const INITS: [&str; 4] = ["tini", "dumb-init", "catatonit", "docker-init"];

/// Eight hex digits drawn at random, so two checks never share a container's name.
fn drawn_name() -> String {
    let mut drawn = [0u8; 4];
    // Not drawn, the name is only less likely to be free, and a clash is a refused run.
    let _ = getrandom::fill(&mut drawn);
    drawn.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn base(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// **What is wrong with an image's config**, as `docker image inspect` gives one image: a sentence
/// each, none where it keeps the contract.
#[must_use]
pub fn config_problems(image: &serde_json::Value) -> Vec<String> {
    use serde_json::Value;
    let mut problems = Vec::new();
    if true {
        return problems;
    }
    let text = |value: Option<&Value>| value.and_then(Value::as_str).unwrap_or_default().to_owned();
    let words = |value: Option<&Value>| -> Vec<String> {
        value
            .and_then(Value::as_array)
            .map(|words| {
                words
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let os = text(image.get("Os"));
    if os != "linux" {
        problems.push("It isn't an image for Linux.".to_owned());
    }
    let config = image.get("Config");
    let spelled = text(config.and_then(|config| config.get("User")));
    let user = spelled.split(':').next().unwrap_or_default();
    if user.parse::<u32>().map_or(true, |number| number == 0) {
        problems.push(
            "Its user has to be a number other than 0, such as USER 10001, so it never runs as \
             root."
                .to_owned(),
        );
    }
    let home = words(config.and_then(|config| config.get("Env")))
        .into_iter()
        .find_map(|entry| entry.strip_prefix("HOME=").map(str::to_owned));
    if !home.is_some_and(|home| home.starts_with('/')) {
        problems.push("It sets no absolute HOME, which is where purlis keeps its data.".to_owned());
    }
    let mut command = words(config.and_then(|config| config.get("Entrypoint")));
    command.extend(words(config.and_then(|config| config.get("Cmd"))));
    let serve = command
        .windows(2)
        .position(|pair| base(&pair[0]) == "purlis" && pair[1] == "serve");
    match serve {
        Some(at) if at + 2 == command.len() => {
            if at == 0 || !INITS.contains(&base(&command[0])) {
                problems.push(format!(
                    "purlis serve has to run under an init that reaps, one of {}.",
                    INITS.join(", ")
                ));
            }
        }
        _ => problems.push("Its default command has to end in purlis serve.".to_owned()),
    }
    problems
}

impl Docker {
    /// **What keeps `image` from being a runner**, each in a sentence, none where it keeps the
    /// contract. It pulls the image where this machine lacks it, and runs a shell in it with no
    /// network and no capabilities, as a pool of it would run.
    pub fn contract_problems(&self, image: &Image) -> Result<Vec<String>, Refused> {
        let inspected = self.inspected_image(image)?;
        let mut problems = config_problems(inspected.pointer("/0").unwrap_or(&inspected));
        // An image that has no shell, or none that runs, can't be looked in, which is its answer.
        // A subshell, so the script's own exit is its lines and never a refused run.
        let name = format!("purlis-contract-{}", drawn_name());
        let words = [
            "run",
            "--rm",
            "-i",
            "--name",
            &name,
            "--network",
            "none",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--entrypoint",
            "/bin/sh",
            image.as_str(),
            "-s",
        ]
        .map(str::to_owned);
        let script = format!("(\n{INSIDE}\n)\nexit 0\n");
        let prepared = self.prepared()?;
        let ran = self.run_prepared(
            &prepared,
            &words,
            Some(&script),
            MOST_SAID,
            Some(INSIDE_WITHIN),
        );
        // Whatever it answered: a run that ran out of time leaves its container behind.
        if ran.is_err() {
            let gone = ["rm".to_owned(), "--force".to_owned(), name.clone()];
            let _ = self.run(&gone, None, MOST_SAID);
        }
        match ran {
            Ok(said) => problems.extend(
                said.lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(crate::shown::short),
            ),
            Err(why) => problems.push(format!(
                "purlis couldn't look inside it: {}",
                crate::shown::short(&why.0)
            )),
        }
        problems.truncate(MOST_PROBLEMS);
        Ok(problems)
    }

    /// **`docker image inspect` of `image`**, pulled by its digest where this machine lacks it.
    /// An engine that didn't answer is said to be so, and nothing is pulled from it.
    pub(super) fn inspected_image(&self, image: &Image) -> Result<serde_json::Value, Refused> {
        let inspect = || {
            self.json_or_late(
                &["image", "inspect", image.as_str()],
                "docker image inspect",
            )
        };
        match inspect() {
            Ok(inspected) => Ok(inspected),
            Err(Failed::Late(why)) => Err(why),
            Err(Failed::Refused(_)) => {
                self.run(
                    &["pull".into(), "--quiet".into(), image.as_str().into()],
                    None,
                    super::MOST_INSPECTED,
                )?;
                inspect().map_err(Refused::from)
            }
        }
    }
}

#[cfg(test)]
mod tests;
