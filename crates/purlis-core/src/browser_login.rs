// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis browser _login`: **a browser-login tool's run** (D-PE-browser-1). The app runs it,
//! never a chat: a sandboxed chat runs a persona's browser-login tool by its name, and the app's
//! broker runs this by purlis's absolute path, with the declared page, Playwright's program as
//! the app found it where no chat writes, the session the chat named, and the vault's login in
//! two private files ([`crate::sandbox::tools::login`]).
//!
//! **What it does, with no shell anywhere.** It opens a browser of its own, in a session whose
//! name nobody chose (`playwright-cli -s=purlis-login-<random> open`), and runs one Playwright
//! function in it from a private file ([`script`]): go to the declared page, and fill the user
//! name and the password only while the page is on the declared origin, with every request to
//! any other origin refused while a field is filled. Then it submits, keeps the session's
//! cookies and storage, and closes that browser. The cookies are written, private, to
//! `.playwright-cli/<session>.login.json` where the chat runs, for the chat to load into its
//! own browser (`playwright-cli -s=<session> state-load …`). The chat's browser never holds the
//! login, so no later step of the chat's can type it into another site.
//!
//! **Bound to its host.** The page is the declaration's; a redirect off its origin, a page the
//! chat steered elsewhere, or a field on another origin fills nothing, and the run fails with a
//! sentence. Playwright's program starts from an environment of a few named variables
//! ([`env_for`]), so nothing in the chat's environment chooses its config, its browser or its
//! Node options.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::repocmd::{Say, Sink};
use crate::sandbox::tools::login as declared;

/// How each step of Playwright's program is run, so a test can stand one in without a browser.
pub trait Steps {
    /// Run `argv` (its program first) in `cwd` from `env` alone: `(exit status, combined
    /// output)`, or why it could not run.
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &[(String, String)],
    ) -> Result<(i32, String), String>;
}

/// The real one: the program by its absolute path, bounded as `browser install`'s is.
pub struct Cli;

impl Steps for Cli {
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &[(String, String)],
    ) -> Result<(i32, String), String> {
        super::bounded(&argv[0], argv, cwd, Some(env))
    }
}

/// The variables Playwright's program is handed from the run's environment, and no other: its
/// search path (which the app already took the chat's folders off), the person's home and
/// account, the language, the run's own temporary folder, and the run's egress proxy.
pub const KEPT_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TMPDIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
];

/// The variable Playwright's program reads its browser's proxy from.
pub const PROXY_VAR: &str = "PLAYWRIGHT_MCP_PROXY_SERVER";

/// **What Playwright's program starts from**: [`KEPT_ENV`] from `env`, and the run's proxy as
/// its browser's ([`PROXY_VAR`]). Nothing else: a config, a browser, a secrets file or a Node
/// option named in the chat's environment is never one the login runs with.
pub fn env_for(env: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = KEPT_ENV
        .iter()
        .filter_map(|name| Some(((*name).to_owned(), env(name)?)))
        .collect();
    let proxy = ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"]
        .iter()
        .find_map(|name| env(name).filter(|value| !value.is_empty()));
    if let Some(proxy) = proxy {
        out.push((PROXY_VAR.to_owned(), proxy));
    }
    out
}

/// `text` as a JavaScript string literal.
fn literal(text: &str) -> String {
    serde_json::Value::String(text.to_owned()).to_string()
}

/// **The Playwright function a login runs** in its own browser: go to `url`, fill `user` (where
/// there is one) and `pass` only on `origin` with every request elsewhere refused meanwhile,
/// submit, and keep the storage in `state` once the page no longer asks for a password.
pub fn script(origin: &str, url: &str, user: Option<&str>, pass: &str, state: &Path) -> String {
    let user = user.map_or_else(|| "null".to_owned(), literal);
    format!(
        r#"async page => {{
  const ORIGIN = {origin};
  const LOGIN = {url};
  const USER = {user};
  const PASS = {pass};
  const STATE = {state};
  const context = page.context();
  const fail = why => {{ throw new Error("purlis: " + why); }};
  const here = () => {{ try {{ return new URL(page.url()).origin === ORIGIN; }} catch {{ return false; }} }};
  const away = url => url.origin !== ORIGIN;
  const refuse = route => route.abort();
  const PASSWORD = 'input[type="password"]:visible';
  const NAMES = ['input[autocomplete~="username"]', 'input[autocomplete~="email"]', 'input[type="email"]',
    'input[name*="user" i]', 'input[name*="login" i]', 'input[name*="email" i]', 'input[id*="user" i]',
    'input[id*="email" i]', 'input[type="text"]'];
  const nameField = async () => {{
    for (const one of NAMES) {{
      const field = page.locator(one + ':visible').first();
      if (await field.count()) return field;
    }}
    return null;
  }};
  const asks = async () => here() && (await page.locator(PASSWORD).count()) > 0;
  const until = async (what, done) => {{
    const deadline = Date.now() + 30000;
    while (!(await done())) {{
      if (Date.now() > deadline) fail(what);
      await page.waitForTimeout(500);
    }}
  }};
  // Filled only on the declared origin, with every request to any other refused meanwhile.
  const fillHere = async (field, value) => {{
    await context.route(away, refuse);
    try {{
      if (!here()) fail("the login page is not on " + ORIGIN + ", so nothing was filled");
      await field.fill(value, {{ timeout: 15000 }});
      if (!here()) fail("the page left " + ORIGIN + " as the login was filled");
    }} finally {{
      await context.unroute(away, refuse);
    }}
  }};
  await page.goto(LOGIN, {{ waitUntil: "domcontentloaded" }});
  await until("no login form appeared on " + ORIGIN + " within 30 seconds",
    async () => (await asks()) || (USER !== null && here() && (await nameField()) !== null));
  if (USER !== null) {{
    const name = await nameField();
    if (name !== null) {{
      await fillHere(name, USER);
      if (!(await asks())) {{
        // The user name first, then the password on a step of its own.
        await name.press("Enter");
        await until("no password field appeared on " + ORIGIN + " within 30 seconds", asks);
      }}
    }}
  }}
  const password = page.locator(PASSWORD).first();
  await fillHere(password, PASS);
  await password.press("Enter");
  await until("the login did not go through: " + ORIGIN + " still asks for a password",
    async () => !(await asks()));
  await page.waitForLoadState("load").catch(() => {{}});
  await context.storageState({{ path: STATE }});
  return "logged in at " + ORIGIN;
}}
"#,
        origin = literal(origin),
        url = literal(url),
        pass = literal(pass),
        state = literal(&state.display().to_string()),
    )
}

/// What the app asked a login to do.
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a> {
    /// Playwright's program, as the app found it where no chat writes.
    pub cli: &'a Path,
    /// The declared login page.
    pub url: &'a str,
    /// The chat's session the cookies are for.
    pub session: &'a str,
    /// Where the chat runs: the cookies go under its `.playwright-cli/`.
    pub cwd: &'a Path,
}

/// Where a login's cookies are left for `session`, under `cwd`.
pub fn state_file(cwd: &Path, session: &str) -> PathBuf {
    cwd.join(super::OUTPUT_DIR)
        .join(format!("{session}.login.json"))
}

/// Reads the vault's value from the private file `var` names in `env`, as `secret exec --file`
/// handed it.
fn handed(env: &dyn Fn(&str) -> Option<String>, var: &str) -> Result<Option<String>, String> {
    let Some(path) = env(var).filter(|path| !path.is_empty()) else {
        return Ok(None);
    };
    std::fs::read(&path)
        .map_err(|e| format!("the login's {var} file could not be read ({e})"))
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| format!("the login's {var} is not text"))
        })
        .map(Some)
}

/// Writes `text` to a new file at `path` that only this user may read.
fn private(path: &Path, text: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(text)
}

/// **`purlis browser _login`**, and its exit code: the login, as the module says. `env` is the
/// run's environment, where the vault's files are named.
pub fn run(
    asked: &Asked<'_>,
    env: &dyn Fn(&str) -> Option<String>,
    steps: &mut dyn Steps,
    say: Sink,
) -> u8 {
    let origin = match declared::origin_of(asked.url) {
        Ok(origin) => origin,
        Err(why) => {
            say(Say::Fail(why));
            return 2;
        }
    };
    if let Err(why) =
        declared::session_of(&[format!("-s={}", asked.session)], "the login", asked.url)
    {
        say(Say::Fail(why));
        return 2;
    }
    if !asked.cli.is_absolute() {
        say(Say::Fail(
            "the login runs Playwright's program by its absolute path alone".to_owned(),
        ));
        return 2;
    }
    let (pass, user) = match (
        handed(env, declared::PASS_VAR),
        handed(env, declared::USER_VAR),
    ) {
        (Ok(Some(pass)), Ok(user)) => (pass, user),
        (Err(why), _) | (_, Err(why)) => {
            say(Say::Fail(why));
            return 1;
        }
        (Ok(None), _) => {
            say(Say::Fail(format!(
                "no password was handed to the login ({}); it runs only as a persona's \
                 browser-login tool",
                declared::PASS_VAR
            )));
            return 2;
        }
    };
    let temp = env("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let own = match tempfile::Builder::new()
        .prefix("purlis-login-")
        .tempdir_in(&temp)
    {
        Ok(own) => own,
        Err(e) => {
            say(Say::Fail(format!(
                "purlis could not make the login's private folder ({e})"
            )));
            return 1;
        }
    };
    let kept = own.path().join("state.json");
    let code = own.path().join("login.js");
    if let Err(e) = private(
        &code,
        script(&origin, asked.url, user.as_deref(), &pass, &kept).as_bytes(),
    ) {
        say(Say::Fail(format!(
            "purlis could not write the login's private file ({e})"
        )));
        return 1;
    }
    drop(pass);
    let session = format!(
        "purlis-login-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    );
    let cli = asked.cli.display().to_string();
    let step = |words: &[&str]| -> Vec<String> {
        std::iter::once(cli.clone())
            .chain(std::iter::once(format!("-s={session}")))
            .chain(words.iter().map(|word| (*word).to_owned()))
            .collect()
    };
    let started = env_for(env);
    let mut ran = |words: &[&str]| steps.run(&step(words), own.path(), &started);
    let opened = ran(&["open"]);
    let logged = match &opened {
        Ok((0, _)) => Some(ran(&[
            "run-code",
            &format!("--filename={}", code.display()),
        ])),
        _ => None,
    };
    let _ = std::fs::remove_file(&code);
    // Its browser is closed whatever happened, so nothing is left holding the page.
    let _ = ran(&["close"]);
    let failed = |output: &str| {
        let output = output.trim();
        if output.is_empty() {
            String::new()
        } else {
            format!(":\n{output}")
        }
    };
    match (opened, logged) {
        (Err(why), _) => {
            say(Say::Fail(format!(
                "purlis could not start Playwright's program for the login ({why})"
            )));
            return 1;
        }
        (Ok((status, output)), None) => {
            say(Say::Fail(format!(
                "purlis could not open a browser for the login (exit {status}){}",
                failed(&output)
            )));
            return 1;
        }
        (_, Some(Err(why))) => {
            say(Say::Fail(format!("the login did not run ({why})")));
            return 1;
        }
        (_, Some(Ok((status, output)))) if status != 0 || !kept.is_file() => {
            say(Say::Fail(format!(
                "the login at {origin} did not go through{}",
                failed(&output)
            )));
            return 1;
        }
        _ => {}
    }
    let state = state_file(asked.cwd, asked.session);
    let moved = std::fs::read(&kept).and_then(|bytes| {
        if let Some(dir) = state.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // A file already there, or a link planted in its place, is replaced, never followed.
        match std::fs::remove_file(&state) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        private(&state, &bytes)
    });
    if let Err(e) = moved {
        say(Say::Fail(format!(
            "logged in at {origin}, and the session could not be written to {} ({e})",
            state.display()
        )));
        return 1;
    }
    let shown = state
        .strip_prefix(asked.cwd)
        .unwrap_or(&state)
        .display()
        .to_string();
    say(Say::Done(format!(
        "Logged in at {origin} in a browser of purlis's own, and kept its session in {shown}."
    )));
    say(Say::Info(format!(
        "Load it into your browser, then delete it: `playwright-cli -s={s} open`, \
         `playwright-cli -s={s} state-load {shown}`, `rm {shown}`. It holds the logged-in \
         cookies, never the password.",
        s = asked.session
    )));
    0
}

#[cfg(all(test, unix))]
#[path = "browser_login_tests.rs"]
mod tests;
