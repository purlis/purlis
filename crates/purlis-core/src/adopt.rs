// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter update`, minus the install, and `charter version` — what charter this is.
//!
//! `charter/commands_update.py` converges three things that all get called "updating charter",
//! and they have three different blast radii: the **CLI**, one machine-global install shared by
//! every plane on the machine; the **harness artifact**, per project; and the **pin** in
//! `charter.toml`, shared with every teammate once it is pushed. Every one of them moves a
//! *Python package*: `uv tool install charter-cp==X`, a version PyPI publishes, a pin naming
//! that version. None of them describes a Rust binary shipped inside a signed app, which moves
//! as the app moves — through Tauri's updater (ADR 0042) — and cannot be moved by this command
//! at all. So `update` says which half it cannot do, which channel the app updates from, and
//! where to read what a version brought: `charter news`, the app's own CHANGELOG.md (#352).
//!
//! **That question — what a version means for a binary that is not a Python package — is
//! answered here too, and it is why `charter version` shares this file.** ADR 0045 settles it,
//! amending ADR 0030: this charter's version is the app's, a plane's pin names a version of the
//! app, and a pin on the Python charter's line is named as that rather than called drift. The
//! two commands live together because they turn on one fact — [`THE_APP_MOVES_IT`] — and a
//! fact stated twice is a fact that drifts.
//!
//! **A charter that tells an operator to adopt something it cannot install is worse than one
//! that says which half it does.** So the refusal is the first line of the output.
//!
//! **No update baseline.** The Python charter stamped the version a plane updated FROM and
//! reported the news range from it. Nothing in this app writes one, and `charter news` no
//! longer reports a range, so nothing here reads one either (#352).

use std::path::Path;

use crate::news::Report;
use crate::scaffold::Say;

/// The fact `charter update` and `charter version` both turn on, written once.
///
/// Two commands saying this in two sentences is two sentences to keep in step, and the day
/// they disagree an operator gets two different accounts of the same artifact. M2.12 is what
/// made it a constant rather than a paragraph in one command's output.
pub const THE_APP_MOVES_IT: &str = "This purlis is a binary inside the app, and the app is what moves it — not a package \
     manager this command can call.";

/// What `charter update` says about the half it does not do.
///
/// One sentence, and it names the mechanism rather than apologising: an operator who reads
/// "charter update did nothing" and an operator who reads this take different next steps.
fn not_the_installer() -> String {
    format!("purlis update does not install anything here. {THE_APP_MOVES_IT}")
}

/// Where to read what a version brought, said in the same breath, so the refusal is not the
/// whole message.
const WHAT_IT_BROUGHT: &str = "  what each version of the app brought:  purlis news";

/// `--to <version>`: a version to install, which is the one thing this command cannot do.
const NO_TARGET: &str = "--to names a published version to install, and this command installs nothing — the \
     version was ignored. The app installs its own updates, from the channel `purlis update \
     --channel` picks.";

/// `--bump`: moving the pin to what was installed, when nothing was.
const NO_BUMP: &str = "--bump moves this plane's `[charter] version` pin to the version it installs, and this \
     command installs nothing, so the pin was left alone. What this purlis is, and what the \
     plane pins: purlis version";

/// What `charter update` was asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateArgs {
    /// `--to <version>`, as typed.
    pub to: String,
    /// `--bump`.
    pub bump: bool,
}

/// `charter update`: say what this command cannot do, and where to read what a version brought.
pub fn update_report(args: &UpdateArgs) -> Report {
    let mut report = Report::new_public();
    report.said.push(Say::Warn(not_the_installer().into()));
    report.said.push(Say::Info(WHAT_IT_BROUGHT.into()));
    if !args.to.is_empty() {
        report.said.push(Say::Warn(NO_TARGET.into()));
    }
    if args.bump {
        report.said.push(Say::Warn(NO_BUMP.into()));
    }
    report
}

// ------------------------------------------------------------------------------------------
// `charter version` (M2.12)
// ------------------------------------------------------------------------------------------

/// What Python prints in the `locked` row when a plane pins nothing — its words, because this
/// row means the same thing in both implementations.
const NO_PIN: &str = "— (this control plane pins no version)";

/// `instance.locked_version`: `[charter] version` as the manifest holds it, or `None`.
///
/// The pin is reported **as written**, a value that is not a version included: refusing here
/// would fold "pinned something malformed" into "pinned nothing", and a plane that opted into
/// conformance would then behave exactly like one that never did.
///
/// **One divergence, and it is Python's crash.** `(cfg.get("charter") or {}).get("version")`
/// raises `AttributeError` when `charter` is a scalar, so `charter version` tracebacks on a
/// manifest `doctor`'s `version lock` row reports calmly. Answering `None` is this binary
/// declining to reproduce a traceback; nothing downstream can tell the two apart, because the
/// only caller is a row that then prints the no-pin line.
pub fn locked_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(crate::names::manifest(root)).ok()?;
    let doc: toml::Table = text.parse().ok()?;
    let pinned = doc.get("charter")?.as_table()?.get("version")?.as_str()?;
    let pinned = crate::memstore::py_strip(pinned);
    (!pinned.is_empty()).then(|| pinned.to_owned())
}

/// The last release of the Python charter — `charter-cp` on PyPI (ADR 0045).
///
/// The boundary a pin is read against: a pin at or below it names the Python line, not this
/// app. A constant, because a boundary that moved without anybody deciding it would change what
/// every plane's pin means.
pub const PYTHON_LINE_LAST: &str = "0.62.1";

/// This charter's version: the app's, which every crate in the workspace shares (ADR 0045).
///
/// The one number `charter version` prints for "which charter is this", and the one a plane's
/// `[charter] version` pin is compared with.
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The Cargo profile this build was made with: `release` for a stable build, `dev-release` for
/// a dev channel build, `debug` for a local one (ADR 0092). `build.rs` says where it is read.
pub fn build_profile() -> &'static str {
    env!("PURLIS_BUILD_PROFILE")
}

/// **The version this build says it is** (#2749): the one its release stamped it with, a dev
/// build's `<version>-dev.<run>` as its app announces it, else [`app_version`]. What a pool's
/// runner is held to: the runner image a release builds is stamped as its desktop is.
pub fn release_version() -> &'static str {
    env!("PURLIS_RELEASE_VERSION")
}

/// What `purlis --version` prints after the program's name: the version, then the profile.
///
/// A dev channel build and a stable build of the same source are not the same binary, and a
/// report of a slow or a crashing one has to say which it was. The release workflow reads this
/// line off the binary it built and refuses a build whose profile is not the one it asked for.
pub const VERSION_LINE: &str = concat!(
    env!("PURLIS_RELEASE_VERSION"),
    " (",
    env!("PURLIS_BUILD_PROFILE"),
    " profile)"
);

/// What a plane's `[charter] version` pin says against this charter — ADR 0045, and the one
/// comparison every surface asks (`charter version`, doctor's `version lock` row, the status
/// line's alert row, the window's pin dialog). ADR 0030's rule, kept: no surface compares a pin
/// its own way. A pin beside the dev channel is a warning before any verdict, on the status
/// line and in doctor alike, by one test ([`crate::alerts`], #1036).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinVerdict {
    /// The plane pins nothing.
    Unpinned,
    /// The pin is this charter's version.
    Met(String),
    /// The pin names a release of the Python charter (`charter-cp`) — an older charter line,
    /// not a version of this app. Not drift: there is nothing here to compare it with.
    PythonLine(String),
    /// The pin names a version of this app that this charter is not, or is not a version.
    Drift(String),
}

impl PinVerdict {
    /// `charter version`'s exit status for this verdict: 1 on drift, 0 otherwise.
    pub fn code(&self) -> u8 {
        u8::from(matches!(self, Self::Drift(_)))
    }
}

/// The verdict for `pinned` — a pin as [`locked_version`] reads it.
///
/// **How a Python pin is told from an app pin.** Every pin a plane carries today was written by
/// the Python charter (`charter version bump`); this charter writes none. The Python line ended
/// at [`PYTHON_LINE_LAST`], so a pin that is a version, is not this app's own, and is no newer
/// than that release names the Python line. Anything newer, or anything that is not a version at
/// all, is a pin on this app's line that this charter does not meet.
///
/// **The cost, stated rather than hidden** (ADR 0045): while the app's own version is at or
/// below 0.62.1 the two lines share numbers, so a pin written by hand for this app below that
/// release reads as the Python line and is never called drift. The ambiguity ends when the app's
/// version passes 0.62.1, or when this charter starts writing pins of its own.
pub fn pin_verdict(pinned: Option<&str>) -> PinVerdict {
    let Some(pin) = pinned else {
        return PinVerdict::Unpinned;
    };
    let pin = pin.to_owned();
    if pin == app_version() {
        return PinVerdict::Met(pin);
    }
    let key = crate::version::version_key(&pin);
    if key.is_version() && key <= crate::version::version_key(PYTHON_LINE_LAST) {
        return PinVerdict::PythonLine(pin);
    }
    PinVerdict::Drift(pin)
}

/// `charter version`: which charter this is, what the plane asks for, and whether they agree.
///
/// **ADR 0045, amending ADR 0030.** This charter's version is the app's — [`app_version`] — and
/// a plane's `[charter] version` means a version of the app. ADR 0030 printed the newest release
/// the Python news corpus names as "the charter this brought", with the app's own number second
/// as the build; that corpus was never this app's, and has since been removed (#352).
///
/// * `charter` — [`app_version`].
/// * `pinned` — `[charter] version`, verbatim.
///
/// **The verdict does not reuse the Python charter's sentences** (ADR 0030's reason stands):
/// *in sync with the lock* would claim a parity with a release this app is not. The exit status
/// is still the one scripts read: 0 with no pin, 0 when the pin is met, 0 for a pin on the
/// Python charter's line — it is not drift — and 1 on drift.
pub fn version_report(root: Option<&Path>) -> Report {
    let mut report = Report::new_public();
    let version = app_version();
    let pinned = root.and_then(locked_version);

    report.out.push_str(&format!("  purlis    {version}\n"));
    report.out.push_str(&format!(
        "  pinned     {}\n\n",
        pinned.as_deref().unwrap_or(NO_PIN)
    ));

    let verdict = pin_verdict(pinned.as_deref());
    report.code = verdict.code();
    match verdict {
        PinVerdict::Unpinned => {
            report.said.push(Say::Info(
                "this control plane pins no purlis version, so there is nothing to conform \
                 to."
                .into(),
            ));
            report
                .said
                .push(Say::Info(format!("  {THE_APP_MOVES_IT}").into()));
        }
        PinVerdict::Met(pin) => {
            report.said.push(Say::Ok(
                format!("this purlis is {pin}, which is what this control plane pins.").into(),
            ));
        }
        PinVerdict::PythonLine(pin) => {
            report.said.push(Say::Info(format!(
                "this control plane pins {pin}, a release of the Python charter (charter-cp): an \
                 older charter line, not a version of this app, so it is not drift and there is \
                 nothing to compare."
            ).into()));
            report.said.push(Say::Info(
                format!(
                    "  to hold this plane to this app instead:  set `[charter] version = \
                 \"{version}\"` in charter.toml, or remove the pin."
                )
                .into(),
            ));
        }
        PinVerdict::Drift(pin) => {
            report.said.push(Say::Warn(
                format!("drift: this control plane pins {pin}, and this purlis is {version}.")
                    .into(),
            ));
            report
                .said
                .push(Say::Info(format!("  {THE_APP_MOVES_IT}").into()));
            report.said.push(Say::Info(
                format!(
                    "  conform the plane instead:  set `[charter] version = \"{version}\"` in \
                 charter.toml, or update the app to the version the plane pins."
                )
                .into(),
            ));
        }
    }
    report
}

/// `charter version sync` and `charter version bump`: what neither of them can do here.
///
/// **They are answered rather than left to clap**, for M2.21's reason: a usage error is what a
/// script meets for a verb a plane's scripts may already call, and it says nothing about why.
/// Both install a charter — `sync` the one the plane pins, `bump` a newer one whose version it
/// then writes into `charter.toml` — and this binary cannot install itself: the app moves it.
///
/// Exit 1, because the operator asked for something that did not happen.
pub fn version_move_refusal(verb: &str) -> Report {
    let mut report = Report::new_public();
    report.said.push(Say::Err(
        format!(
            "purlis version {verb} moves a pin by installing the purlis it names, and this \
         purlis cannot install itself."
        )
        .into(),
    ));
    report
        .said
        .push(Say::Info(format!("  {THE_APP_MOVES_IT}").into()));
    report.said.push(Say::Info(
        "  what this purlis is, and what this plane pins:  purlis version"
            .to_owned()
            .into(),
    ));
    report.code = 1;
    report
}

// ------------------------------------------------------------------------------------------
// `charter update --channel` (ADR 0042)
// ------------------------------------------------------------------------------------------

/// Which stream the app takes its next version from, said beside [`THE_APP_MOVES_IT`].
///
/// `update` is the command whose subject is "moving charter", and its first line already says
/// the app is what does the moving. The channel is which stream it moves along, so this is the
/// natural place to say it — and until M6.4's status bar exists, the only place an operator can
/// change it at all.
fn channel_line(channel: crate::updates::Channel) -> String {
    let other = match channel {
        crate::updates::Channel::Stable => crate::updates::Channel::Dev,
        crate::updates::Channel::Dev => crate::updates::Channel::Stable,
    };
    format!(
        "  the app on this machine updates from the {} channel.  to move it:  purlis update \
         --channel {}",
        channel.name(),
        other.name()
    )
}

/// `charter update` without `--channel`: the report as it was, plus the channel line.
///
/// `config_root` is `None` on a machine with no config home, which reads as the default — the
/// same answer the app gives, from the same store.
pub fn update_report_with_channel(config_root: Option<&Path>, args: &UpdateArgs) -> Report {
    let mut report = update_report(args);
    let channel = config_root
        .map(|at| crate::machine::read(at).store.channel)
        .unwrap_or_default();
    // Second, right under the refusal that names the app as the mover — not at the bottom,
    // where it would read as a footnote to a different subject.
    report
        .said
        .insert(1, Say::Info(channel_line(channel).into()));
    report
}

/// `charter update --channel <word>`: put this machine on a channel, or refuse the word.
///
/// Only the channel moves. The pointer to `charter news` is not said, because an operator
/// switching streams is not asking what a version brought, and a command that said two things
/// at once would bury the one sentence they typed it for.
///
/// **A word charter does not know exits 1 and changes nothing.** The exact reader
/// ([`crate::updates::Channel::named`]) is the rule; `Dev` or `nightly` is a typo, and guessing
/// what it meant would be guessing toward the less safe channel.
pub fn set_channel_report(config_root: Option<&Path>, word: &str) -> Report {
    let mut report = Report::new_public();
    let Some(channel) = crate::updates::Channel::named(word) else {
        report.said.push(Say::Err(
            format!("{word:?} is not an update channel. There are two: stable and dev.").into(),
        ));
        report.code = 1;
        return report;
    };
    let Some(config_root) = config_root else {
        report.said.push(Say::Err(
            "this machine has no config home, so purlis has nowhere to keep the channel.".into(),
        ));
        report.code = 1;
        return report;
    };
    match crate::machine::update(config_root, |store| store.channel = channel) {
        Ok(_) => {
            report.said.push(Say::Ok(
                format!(
                    "the app on this machine now updates from the {} channel.",
                    channel.name()
                )
                .into(),
            ));
            report.said.push(Say::Info(
                format!(
                    "  it reads {} at its next check; nothing is installed until you say so in the \
                 app.",
                    channel.endpoint()
                )
                .into(),
            ));
        }
        Err(why) => {
            report.said.push(Say::Err(
                format!("the channel could not be recorded: {why}").into(),
            ));
            report.code = 1;
        }
    }
    report
}

#[cfg(test)]
mod tests;
