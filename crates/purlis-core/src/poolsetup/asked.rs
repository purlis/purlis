// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a new Docker pool is asked to be** (#4481, Q30): one value, which the Pool step's button
//! makes and See what this does prints, and which `purlis pool add` reads back. So what is shown
//! is what the button runs, by construction: the words of [`Asked::command`] parse into the same
//! [`Asked`], and both reach [`create`].

use std::path::Path;
use std::time::Duration;

use purlis_session_protocol::hostlink::Pinned;

use crate::dockerpool::{Docker, Image, PURLIS_RUNNER_IMAGE};
use crate::runners::{Pool, Refused, Set};

/// What a pool is asked to be. Empty is purlis's default, as in the add pool form it replaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asked {
    pub name: String,
    pub image: String,
    /// How many CPUs, to the thousandth, as a person writes them.
    pub cpus: Option<String>,
    pub memory_mib: Option<u32>,
    pub processes: Option<u32>,
    pub tmp_mib: Option<u32>,
    /// How long its runner waits without a link before it suspends, spelled `12h`.
    pub suspend_after: Option<String>,
    pub frames: Option<u32>,
    pub width: Option<u32>,
}

/// What [`create`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created {
    Added,
    /// A team's image breaks the contract, each problem one sentence. Nothing was made.
    Breaks(Vec<String>),
}

/// `typed`, a time a pool's runner may be left without a link: `12h`, `90m`.
pub fn suspend_after_of(typed: &str) -> Result<Duration, String> {
    let typed = typed.trim();
    let after =
        crate::spelledtime::read(typed, crate::spelledtime::UP_TO_HOURS).ok_or_else(|| {
            format!(
                "\"{typed}\" is not a time; write a whole number followed by h, m or s, such as \
                 12h or 90m"
            )
        })?;
    crate::dockerpool::suspend_after(after).map_err(|why| why.0)
}

/// `typed`, a number of CPUs to the thousandth (`2`, `0.5`, `1.25`), in thousandths.
pub fn millicpus_of(typed: &str) -> Result<u32, String> {
    let not = || {
        format!(
            "\"{typed}\" is not a number of CPUs; write one such as 2 or 1.5, to the thousandth"
        )
    };
    let typed = typed.trim();
    let (whole, part) = typed.split_once('.').unwrap_or((typed, "0"));
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || !digits(part) || part.len() > 3 {
        return Err(not());
    }
    let whole: u32 = whole.parse().map_err(|_| not())?;
    let part: u32 = format!("{part:0<3}").parse().map_err(|_| not())?;
    whole
        .checked_mul(1000)
        .and_then(|whole| whole.checked_add(part))
        .ok_or_else(not)
}

impl Asked {
    /// **The pool this asks for**, held to the core's rules.
    pub fn pool(&self) -> Result<Pool, String> {
        let image = Image::pinned(self.image.trim()).map_err(|why| why.0)?;
        let filled = |text: &Option<String>| {
            text.as_deref()
                .filter(|text| !text.trim().is_empty())
                .map(str::to_owned)
        };
        let set = Set {
            millicpus: filled(&self.cpus)
                .as_deref()
                .map(millicpus_of)
                .transpose()?,
            memory_mib: self.memory_mib.map(u64::from),
            processes: self.processes,
            tmp_mib: self.tmp_mib.map(u64::from),
            suspend_after: filled(&self.suspend_after)
                .as_deref()
                .map(suspend_after_of)
                .transpose()?,
            frames: self.frames,
            width: self
                .width
                .map(|width| {
                    u16::try_from(width).map_err(|_| {
                        let widest = purlis_session_protocol::browsing::LinkFrames::WIDTH;
                        format!(
                            "a pool's widest browser frame is {} to {} pixels",
                            widest.start(),
                            widest.end()
                        )
                    })
                })
                .transpose()?,
            chosen: Default::default(),
        };
        Pool::new(self.name.trim(), image, set).map_err(|why| why.to_string())
    }

    /// **The command that does what the button does**: only what the person changed, and only
    /// values a pool could have, so a word of it never means more than it says in any shell. A
    /// name or image that isn't one yet is a plain stand-in to fill in.
    #[must_use]
    pub fn command(&self) -> String {
        let (name, image) = (self.name.trim(), self.image.trim());
        let name = if crate::runners::usable_pool_name(name).is_ok() {
            crate::shellseg::shell_quote(name)
        } else {
            "NAME".to_owned()
        };
        let image = if Image::pinned(image).is_ok() {
            crate::shellseg::shell_quote(image)
        } else {
            "IMAGE".to_owned()
        };
        let mut words = vec![
            "purlis pool add".to_owned(),
            name,
            "--image".to_owned(),
            image,
        ];
        let mut with = |flag: &str, value: Option<String>| {
            if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
                words.push(flag.to_owned());
                words.push(crate::shellseg::shell_quote(value.trim()));
            }
        };
        let number = |value: Option<u32>| value.map(|value| value.to_string());
        with(
            "--cpus",
            self.cpus.clone().filter(|cpus| millicpus_of(cpus).is_ok()),
        );
        with("--memory-mib", number(self.memory_mib));
        with("--processes", number(self.processes));
        with("--tmp-mib", number(self.tmp_mib));
        with(
            "--suspend-after",
            self.suspend_after
                .clone()
                .filter(|after| suspend_after_of(after).is_ok()),
        );
        with("--frames", number(self.frames));
        with("--width", number(self.width));
        words.join(" ")
    }

    /// Whether this names purlis's own image, which every release checks already.
    #[must_use]
    pub fn is_ours(&self) -> bool {
        self.image
            .trim()
            .split_once('@')
            .is_some_and(|(repository, _)| repository == PURLIS_RUNNER_IMAGE)
    }
}

/// **Makes the pool `asked` asks for**: the one way to it, from the button and from `purlis pool
/// add`. A team's image is held to the contract first, so one that breaks it makes nothing.
pub fn create(
    config: &Path,
    docker: &Docker,
    asked: &Asked,
    desktop: &Pinned,
) -> Result<Created, Refused> {
    let pool = asked.pool().map_err(Refused::Other)?;
    if !asked.is_ours() {
        let problems = docker
            .contract_problems(pool.image())
            .map_err(|why| Refused::Other(why.0))?;
        if !problems.is_empty() {
            return Ok(Created::Breaks(problems));
        }
    }
    crate::pools::add(config, docker, &pool, desktop)?;
    Ok(Created::Added)
}

#[cfg(test)]
mod tests;
