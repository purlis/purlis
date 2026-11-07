// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser login a chat proposes and the person confirms** (#4595, the person's rulings of
//! 2026-11-01, which amend PC-1 and PC-5 of #1975 for this one case).
//!
//! `browser_login` on a login its persona doesn't declare suggests one here: a site, a vault and
//! the keys of that vault. The window shows it as one notice, and the person answers it. Here is
//! what such a suggestion may be, whichever way it's answered:
//!
//! - **The origin is what's confirmed** ([`site_of`]): `https` alone, a name a browser spells the
//!   same way, and never an address or a name of one label. The page is rebuilt from that origin
//!   and the path, so nothing a chat wrote survives that the notice doesn't show.
//! - **So is the form's** ([`form_from`], #4995, the person's ruling of 2026-11-03): before the
//!   person is asked, purlis opens the page in a browser of its own and follows it to its login
//!   form. The origin the form showed on is the one place the password is typed, held to the
//!   same rules, and the notice names it beside the page, with a warning where they differ.
//! - **A key a `mint` line names is never offered** ([`proposal`]), nor any key while the
//!   project's file can't be read to tell.
//! - **What the person is warned of** ([`Warning`]): a password key used on another site, and a
//!   name written in another alphabet. The warnings are words: the choices stay the person's.
//! - **The tool comes from the very line that's written** ([`Proposal::tool`]): the line is read
//!   by the one reader every declaration goes through, so a login allowed once and a login in the
//!   project's file are one thing.
//! - **Always writes that line and nothing else** ([`with_line`]): the file is edited in place,
//!   then read back and compared whole.
//! - **A call naming another site for a login already declared asks to replace it**
//!   ([`Proposal::replacing`], #4996, the person's ruling of 2026-11-03): the notice shows the
//!   site it has and the one proposed. Only the site moves: the vault and the keys stay the
//!   declared line's, and Always swaps that one line, and only while it still reads as shown.
//!
//! Nothing here reads a value, and nothing here asks: `crate::secrets::brokered` fills and the
//! app holds the notice.

use std::path::Path;

use sha2::Digest as _;

use crate::sandbox::tools::{self, Tool, login, mint};

/// **How long a chat's call waits for the person's answer.** Past it the notice goes, and the
/// chat is told it may ask again.
pub const THE_PERSON_IS_WAITED_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// The longest site a chat may suggest. A login page's address is short, and the notice shows it
/// whole.
const MOST_SITE: usize = 512;

/// The longest vault key or vault name a notice shows.
const MOST_NAME: usize = 128;

/// What a chat is told where it names another vault or other keys for login `login`, which is
/// declared already (#4996): a call moves its site alone.
pub fn only_its_site_said(login: &str) -> String {
    format!(
        "{login} is declared with another vault or other keys, and a call may change its site \
         alone, so nothing was filled. Call browser_login without site to log in as it is \
         declared"
    )
}

/// **Where a page sends the browser to sign in** (#4995): the address its login form showed on, as
/// the browser recorded it, or why none did.
pub type FindsForm<'a> = &'a dyn Fn(&str) -> Result<String, String>;

/// **What a chat suggests**, as its `browser_login` call says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Suggested {
    /// The login page's address, or the site's name.
    pub site: String,
    pub vault: String,
    /// The key of the user name, for a page that asks for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_key: Option<String>,
    /// The key of the password.
    pub key: String,
}

/// **What the person is warned of**, in the notice, above the choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// The password's key is used on these other origins.
    KeyElsewhere { origins: Vec<String> },
    /// The name is written in another alphabet: this is how a browser may show it.
    OtherAlphabet { shown_as: String },
    /// The site is on a port of its own, so it's another server than the site's usual one.
    OtherPort { port: u16 },
    /// The page isn't the site's front page (the person's ruling of 2026-11-05). A page can sign
    /// the browser in to another account as it opens, on a shared site it can be anyone's, and it
    /// sees what's filled on it.
    ChatsPage { path: String },
    /// The vault isn't one this persona uses: the press lets this one login read it.
    NotItsVault { vault: String, persona: String },
    /// A key is also a declared tool's credential.
    ToolsKey { tool: String, key: String },
    /// The persona is the project's default, so Always reaches every chat that names no persona.
    DefaultPersona { persona: String },
    /// The page sent the browser to a login form on another host (#4995): the password goes
    /// there, not to the page's.
    FormElsewhere {
        page_host: String,
        form_host: String,
    },
    /// The vault holds no value under these keys yet ([`not_in_vault`]): the person is asked for
    /// each as the login is filled (#5074).
    NotInVault { vault: String, keys: Vec<String> },
}

impl Warning {
    /// The warning in the words the notice says it.
    pub fn said(&self) -> String {
        match self {
            Self::KeyElsewhere { origins } => format!(
                "This password is also used on {}. Filling it here gives it to a different site.",
                origins.join(", ")
            ),
            Self::OtherAlphabet { shown_as } => format!(
                "This name uses letters from another alphabet. It can look like {shown_as}, and \
                 it may not be the site you know."
            ),
            Self::OtherPort { port } => format!(
                "This site is on port {port}, so a different server may answer than the one you \
                 know."
            ),
            Self::ChatsPage { path } => format!(
                "The chat chose the page {path}, not the site's front page. Opening a page can \
                 sign the browser in to someone else's account, on a shared site a page can be \
                 anyone's, and a page sees what's filled on it. Allow only if you know {path} as \
                 this site's sign-in page."
            ),
            Self::NotItsVault { vault, persona } => format!(
                "Vault {vault} isn't one {persona} uses. Allowing this lets this login read from \
                 it."
            ),
            Self::ToolsKey { tool, key } => {
                format!("{key} is the credential of the tool {tool}, not a website login.")
            }
            Self::DefaultPersona { persona } => format!(
                "{persona} is this project's default persona, so Always allow reaches every chat \
                 that names no persona too."
            ),
            Self::FormElsewhere {
                page_host,
                form_host,
            } => format!(
                "The password goes to {form_host}, not {page_host}: {page_host} sent the browser \
                 there to sign in. Allow only if you know {form_host} as this site's sign-in."
            ),
            Self::NotInVault { vault, keys } => format!(
                "Vault {vault} has nothing under {} yet. If you allow this, purlis asks you for \
                 it when it signs in. Check the name if you expected it there.",
                keys.join(" or ")
            ),
        }
    }
}

/// **A login as the notice shows it**: all the person is asked to confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub persona: String,
    pub login: String,
    pub vault: String,
    pub user_key: Option<String>,
    pub key: String,
    /// `https://host[:port]`: exactly where it's filled.
    pub origin: String,
    /// The origin, then the path. Never a query or a fragment.
    pub page: String,
    /// **The origin the login form showed on** (#4995): the one place the password is typed.
    /// [`Self::origin`] where the form is on the page's own.
    pub form: String,
    /// **The site the declared login has now, which this one replaces** (#4996).
    pub replaces: Option<Replaced>,
    pub warnings: Vec<Warning>,
}

/// **The site a declared login has, which a proposal replaces** (#4996): its page, and the origin
/// its password is typed on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    pub page: String,
    pub form: String,
}

/// **A suggested site, as purlis will fill it**: its origin and its page, or why it's not one the
/// person is asked about. Stricter than a declaration's ([`login::origin_of`]), which the person
/// wrote themselves: `https` alone, and never a bare name.
pub fn site_of(site: &str) -> Result<(String, String), String> {
    let site = site.trim();
    if site.is_empty() || site.len() > MOST_SITE {
        return Err("give `site` the login page's https address".to_owned());
    }
    // A `%` too: nothing is shown decoded, and nothing a browser reads as a separator is left to
    // its reading.
    if site
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\' || c == '%')
    {
        return Err(
            "`site` has a space, a backslash, a percent sign or a control character in it"
                .to_owned(),
        );
    }
    let lowered = site.to_lowercase();
    let rest = match site.split_once("://") {
        None => site,
        Some(_) if lowered.starts_with("https://") => &site["https://".len()..],
        Some(_) => {
            return Err(format!(
                "{site} is not https, and purlis fills a suggested login on https alone"
            ));
        }
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let path = if path.is_empty() { "/" } else { path };
    // Printable ASCII: a path that draws as what it is, with no marks that turn text around.
    if !path.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(
            "`site` has a page address with letters a notice can't show as they are".into(),
        );
    }
    let origin = login::origin_of(&format!("https://{}/", authority.to_lowercase()))
        .map_err(|why| why.replace("browser = ", "site = "))?;
    let host = host_of(&origin);
    let labels: Vec<&str> = host.split('.').collect();
    let last = labels.last().copied().unwrap_or_default();
    // Every spelling a browser reads as an address ends in a number or a hex number.
    let numeric = last.bytes().all(|b| b.is_ascii_digit())
        || last
            .strip_prefix("0x")
            .is_some_and(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()));
    let private = matches!(
        last,
        "localhost" | "local" | "internal" | "lan" | "home" | "corp"
    );
    if labels.len() < 2 || numeric || host.starts_with('[') || private {
        return Err(format!(
            "{host} is an address, a name on this machine or a bare name, so it can't be \
             confirmed by reading it. A login there is declared in the project's settings"
        ));
    }
    let (_, spelled) = idna::domain_to_unicode(host);
    if spelled.is_err() {
        return Err(format!("{host} is not a name a browser spells this way"));
    }
    let page = format!("{origin}{path}");
    Ok((origin, page))
}

/// **The origin of the address a login form showed on** (#4995), as the browser recorded it, held
/// to [`site_of`]'s rules, or why the password may not be typed there. Only its scheme and host
/// are ever said: the rest of a form's address may carry what the site handed the browser.
pub fn form_from(found: &str) -> Result<String, String> {
    let (scheme, rest) = found.split_once("://").unwrap_or(("", found));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let shown = crate::shown::short(&format!("{scheme}://{authority}"));
    if !scheme.eq_ignore_ascii_case("https") || authority.is_empty() {
        return Err(format!(
            "the page sends the browser to a login form on {shown}, which is not https, so \
             purlis records nothing and fills nothing there. Give the https address of a login \
             page whose form is on https"
        ));
    }
    let (origin, _) = site_of(&format!("https://{authority}/")).map_err(|why| {
        format!(
            "the page sends the browser to a login form on {shown}, so purlis records nothing \
             and fills nothing there: {why}"
        )
    })?;
    Ok(origin)
}

/// The host of `origin`, as [`site_of`] and [`login::origin_of`] write one.
fn host_of(origin: &str) -> &str {
    let host = origin.strip_prefix("https://").unwrap_or(origin);
    match host.rsplit_once(':') {
        Some((name, port)) if port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    }
}

/// Whether `name` is a vault's or a key's plain name: one word, with nothing a provider reads as
/// a path to another field, and nothing a notice would draw as a sentence.
pub(crate) fn plain(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MOST_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// **What the project at `root` holds of a suggestion**, or why the person isn't asked: the
/// login's name and the vault's keys are plain, no key is one a `mint` line names, and the
/// project's file can be read to tell. `granted` are the logins the person allowed for now,
/// counted as declared for [`Warning::KeyElsewhere`].
///
/// `finds` follows the page to its login form, once all else is checked, and answers the address
/// the form showed on ([`form_from`]), or why none did.
pub fn proposal(
    root: &Path,
    persona: &str,
    login: &str,
    suggested: &Suggested,
    finds: FindsForm<'_>,
    granted: &[Tool],
) -> Result<Proposal, String> {
    if let Some(why) = tools::refused_name(login) {
        return Err(why);
    }
    if !plain(&suggested.vault) {
        return Err("give `vault` the name of a vault".to_owned());
    }
    if suggested.user_key.as_ref() == Some(&suggested.key) {
        return Err("`key` and `user_key` must be two different keys of the vault".to_owned());
    }
    let Some(marked) = mint::marked(root) else {
        return Err(mint::unreadable_said());
    };
    let keys = std::iter::once(&suggested.key).chain(suggested.user_key.as_ref());
    for key in keys {
        if !plain(key) {
            return Err("give `key` and `user_key` the names of keys in the vault".to_owned());
        }
        // A provider may read a key in any case: a marked key is refused however it's spelled.
        if marked.iter().any(|(vault, one)| {
            vault.eq_ignore_ascii_case(&suggested.vault) && one.eq_ignore_ascii_case(key)
        }) {
            return Err(mint::refused_said(&suggested.vault, key));
        }
    }
    let (origin, page) = site_of(&suggested.site)?;
    let form = form_from(&finds(&page)?)?;
    let mut proposal = Proposal {
        persona: persona.to_owned(),
        login: login.to_owned(),
        vault: suggested.vault.clone(),
        user_key: suggested.user_key.clone(),
        key: suggested.key.clone(),
        origin,
        page,
        form,
        replaces: None,
        warnings: Vec::new(),
    };
    proposal.warnings = warnings_of(root, &proposal, granted);
    Ok(proposal)
}

/// **What the person is warned of** for `proposal` as it stands.
pub fn warnings_of(root: &Path, proposal: &Proposal, granted: &[Tool]) -> Vec<Warning> {
    let mut warnings = Vec::new();
    let declared = declared_tools(root);
    let origins = elsewhere(&declared, proposal, granted);
    if !origins.is_empty() {
        warnings.push(Warning::KeyElsewhere { origins });
    }
    let keys = std::iter::once(&proposal.key).chain(proposal.user_key.as_ref());
    for key in keys {
        let tool = declared.iter().find(|tool| {
            tool.login.is_none()
                && tool.vault == proposal.vault
                && tool
                    .values
                    .iter()
                    .any(|value| value.key.eq_ignore_ascii_case(key))
        });
        if let Some(tool) = tool {
            warnings.push(Warning::ToolsKey {
                tool: tool.name.clone(),
                key: key.clone(),
            });
        }
    }
    if proposal.form != proposal.origin {
        warnings.push(Warning::FormElsewhere {
            page_host: host_of(&proposal.origin).to_owned(),
            form_host: host_of(&proposal.form).to_owned(),
        });
    }
    // The page's name and the form's are each read as a browser shows them.
    let mut origins = vec![proposal.origin.as_str()];
    if proposal.form != proposal.origin {
        origins.push(proposal.form.as_str());
    }
    for origin in origins {
        let host = host_of(origin);
        if host.split('.').any(|label| label.starts_with("xn--")) {
            let (shown_as, _) = idna::domain_to_unicode(host);
            warnings.push(Warning::OtherAlphabet { shown_as });
        }
        if let Some((_, port)) = origin["https://".len()..].rsplit_once(':')
            && let Ok(port) = port.parse()
        {
            warnings.push(Warning::OtherPort { port });
        }
    }
    // Any page but the front one (the person's ruling of 2026-11-05): a usual login address too,
    // as the site decides what any of its pages does.
    let path = &proposal.page[proposal.origin.len()..];
    if path != "/" {
        warnings.push(Warning::ChatsPage {
            path: path.to_owned(),
        });
    }
    if crate::sandbox::persona::is_default(root, &proposal.persona) {
        warnings.push(Warning::DefaultPersona {
            persona: proposal.persona.clone(),
        });
    }
    warnings
}

/// The providers that list a vault's key names without asking anyone, from files or an index of
/// purlis's own: the system's credential store's index, a plain file, references. Any other, as
/// 1Password's CLI may, can ask the person to unlock it just to list.
const LISTS_QUIETLY: &[&str] = &["keyring", "plain-file", "reference"];

/// **The keys of `proposal` its vault holds no value under**, said, where its provider lists its
/// key names without asking anyone ([`LISTS_QUIETLY`]): a suggested name that's wrong, or one an
/// Always line would write, is shown before the press. Read by key names alone. `None` where
/// each is there, or the vault can't be listed quietly.
pub fn not_in_vault(ctx: &crate::secrets::Ctx, proposal: &Proposal) -> Option<Warning> {
    let vault = crate::secrets::cmd::provider(ctx, &proposal.vault).ok()?;
    if !LISTS_QUIETLY.contains(&vault.provider.as_str()) {
        return None;
    }
    let held = crate::secrets::cmd::keys(ctx, &vault).ok()?;
    let keys: Vec<String> = proposal
        .user_key
        .iter()
        .chain(std::iter::once(&proposal.key))
        .filter(|key| !held.contains(key))
        .cloned()
        .collect();
    (!keys.is_empty()).then(|| Warning::NotInVault {
        vault: proposal.vault.clone(),
        keys,
    })
}

/// Every tool the project's file declares, each persona's.
fn declared_tools(root: &Path) -> Vec<Tool> {
    let said = crate::sandbox::Plane::read(root).said();
    said.policy
        .or(said.unset)
        .map(|policy| {
            policy
                .personas
                .values()
                .flat_map(|grants| grants.tools.iter().cloned())
                .collect()
        })
        .unwrap_or_default()
}

/// The other origins either of the proposal's keys is on, in any spelling: a password named as a
/// user name goes to the site all the same. Each persona's logins, and `granted`.
///
/// The very login a proposal replaces is not another site.
fn elsewhere(declared: &[Tool], proposal: &Proposal, granted: &[Tool]) -> Vec<String> {
    let replaced = |tool: &Tool| {
        tool.name == proposal.login
            && proposal.replaces.as_ref().is_some_and(|old| {
                tool.login
                    .as_ref()
                    .is_some_and(|login| login.url == old.page && login.fills_on() == old.form)
            })
    };
    let same = |key: &str| {
        key.eq_ignore_ascii_case(&proposal.key)
            || proposal
                .user_key
                .as_ref()
                .is_some_and(|user| key.eq_ignore_ascii_case(user))
    };
    let mut origins: Vec<String> = declared
        .iter()
        .chain(granted)
        .filter(|tool| tool.vault == proposal.vault && !replaced(tool))
        .filter(|tool| {
            tool.login_keys()
                .is_some_and(|(user, key)| same(key) || user.is_some_and(same))
        })
        // Where its password is typed elsewhere, that origin; where it's typed on this form for
        // another site's page, that site: a sign-in it already serves.
        .filter_map(|tool| {
            let login = tool.login.as_ref()?;
            if login.fills_on() != proposal.form {
                Some(login.fills_on().to_owned())
            } else if login.origin != proposal.origin {
                Some(login.origin.clone())
            } else {
                None
            }
        })
        .collect();
    origins.sort();
    origins.dedup();
    origins
}

impl Proposal {
    /// **This proposal as a replacement of `declared`'s site** (#4996), its warnings worked out
    /// again, or why it can't be one: `declared` is no browser login, or names another vault or
    /// other keys, which a call never changes. Its line in the project's file stays as it is.
    pub fn replacing(self, root: &Path, declared: &Tool, granted: &[Tool]) -> Result<Self, String> {
        let keys = declared.login_keys();
        let Some(login) = declared.login.as_ref().filter(|_| {
            declared.vault == self.vault
                && keys == Some((self.user_key.as_deref(), self.key.as_str()))
        }) else {
            return Err(only_its_site_said(&self.login));
        };
        let mut replacing = Self {
            replaces: Some(Replaced {
                page: login.url.clone(),
                form: login.fills_on().to_owned(),
            }),
            ..self
        };
        replacing.warnings = warnings_of(root, &replacing, granted);
        Ok(replacing)
    }

    /// Whether this proposal names the very site `declared` has: nothing to replace.
    pub fn is_where(&self, declared: &Tool) -> bool {
        declared
            .login
            .as_ref()
            .is_some_and(|login| login.url == self.page && login.fills_on() == self.form)
    }

    /// **The same login where the person typed**: read as a suggestion is, a typed page taking the
    /// place of the page and a bare domain keeping it, its warnings worked out again, and the
    /// page followed to its form again with `finds`.
    pub fn at(
        &self,
        root: &Path,
        typed: &str,
        finds: FindsForm<'_>,
        granted: &[Tool],
    ) -> Result<Self, String> {
        let (origin, page) = site_of(typed)?;
        let bare = typed.trim().rsplit("://").next().unwrap_or_default();
        let page = if bare.contains('/') {
            page
        } else {
            format!("{origin}{}", &self.page[self.origin.len()..])
        };
        let form = form_from(&finds(&page)?)?;
        let mut moved = Self {
            page,
            origin,
            form,
            warnings: Vec::new(),
            ..self.clone()
        };
        moved.warnings = warnings_of(root, &moved, granted);
        Ok(moved)
    }

    /// **What a press is bound to**: everything the notice shows, each field tagged and
    /// length-prefixed, so a press on one proposal never answers another.
    pub fn digest(&self) -> String {
        let mut digest = sha2::Sha256::new();
        digest.update(b"purlis proposed browser login v2\0");
        let warnings: Vec<String> = self.warnings.iter().map(Warning::said).collect();
        let warnings = warnings.join("\n");
        for field in [
            self.persona.as_str(),
            self.login.as_str(),
            self.vault.as_str(),
            self.user_key.as_deref().unwrap_or("\0"),
            self.key.as_str(),
            self.origin.as_str(),
            self.page.as_str(),
            self.form.as_str(),
            self.replaces.as_ref().map_or("\0", |old| old.page.as_str()),
            self.replaces.as_ref().map_or("\0", |old| old.form.as_str()),
            warnings.as_str(),
        ] {
            digest.update((field.len() as u64).to_be_bytes());
            digest.update(field.as_bytes());
        }
        crate::extension::hex(&digest.finalize())
    }

    /// **The line Always writes**: `<login> = { vault = …, browser = …, user = …, key = … }`.
    pub fn line(&self) -> String {
        format!("{} = {}", self.login, self.table())
    }

    /// The login's table, spelled as TOML, each value quoted by the writer that reads it.
    fn table(&self) -> toml_edit::InlineTable {
        let mut table = toml_edit::InlineTable::new();
        table.insert("vault", self.vault.as_str().into());
        table.insert(login::KEY, self.page.as_str().into());
        // Written only where the form is on another origin: a line without it is the line a
        // login always had (#4995).
        if self.form != self.origin {
            table.insert(login::FORM_KEY, self.form.as_str().into());
        }
        if let Some(user) = &self.user_key {
            table.insert(login::USER_KEY, user.as_str().into());
        }
        table.insert("key", self.key.as_str().into());
        table.fmt();
        table
    }

    /// **The login as a tool**, read from its own line by the reader of the project's file, so
    /// what's filled is exactly what would be written. Fails closed where the reader refuses it.
    pub fn tool(&self) -> Result<Tool, String> {
        let text = format!("{}\n", self.line());
        let table: toml::Table = text
            .parse()
            .map_err(|_| "this login can't be written as a line".to_owned())?;
        let (tools, refused) =
            tools::keyed(Some(&toml::Value::Table(table)), "login", "the notice", &[]);
        if let Some(refused) = refused.first() {
            return Err(refused.why.clone());
        }
        match tools.as_slice() {
            [tool]
                if tool
                    .login
                    .as_ref()
                    .is_some_and(|l| l.url == self.page && l.fills_on() == self.form) =>
            {
                Ok(tool.clone())
            }
            _ => Err("this login can't be written as a line".to_owned()),
        }
    }
}

/// **The project's file `text` with `proposal`'s line in `[sandbox.personas.<persona>]`**, or why
/// not: a login of that name there already, a file whose parts aren't tables, or one with no
/// `[sandbox]`, which an added table would turn into a project that says something about its
/// sandbox. Read back and compared: the file after it holds what it held, and this login, and
/// nothing else moved. A proposal that replaces a login's site (#4996) swaps that login's line,
/// only while it reads as the site the notice showed it had.
pub fn with_line(text: &str, proposal: &Proposal) -> Result<String, String> {
    let unwritten = || {
        format!(
            "purlis could not add {} to the project's settings, and wrote nothing",
            proposal.login
        )
    };
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|_| unwritten())?;
    let sandbox = doc
        .as_table_mut()
        .get_mut("sandbox")
        .and_then(toml_edit::Item::as_table_mut)
        .ok_or_else(unwritten)?;
    let personas = sandbox
        .entry("personas")
        .or_insert_with(|| {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            toml_edit::Item::Table(table)
        })
        .as_table_mut()
        .ok_or_else(unwritten)?;
    let persona = personas
        .entry(&proposal.persona)
        .or_insert_with(toml_edit::table)
        .as_table_mut()
        .ok_or_else(unwritten)?;
    let name = proposal.login.as_str();
    let line = toml_edit::Value::InlineTable(proposal.table());
    if let Some(old) = &proposal.replaces {
        let declared = declared_in(text, &proposal.persona, name).ok_or_else(unwritten)?;
        let still = declared.vault == proposal.vault
            && declared.login_keys() == Some((proposal.user_key.as_deref(), proposal.key.as_str()))
            && declared
                .login
                .as_ref()
                .is_some_and(|login| login.url == old.page && login.fills_on() == old.form);
        if !still {
            return Err(unwritten());
        }
        match persona.get_mut(tools::KEY) {
            Some(toml_edit::Item::Table(tools)) if tools.contains_key(name) => {
                tools.insert(name, toml_edit::Item::Value(line));
            }
            Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(tools)))
                if tools.contains_key(name) =>
            {
                tools.insert(name, line);
                tools.fmt();
            }
            _ => return Err(unwritten()),
        }
        let after = doc.to_string();
        return if only_this_line(
            text,
            &after,
            &proposal.persona,
            &proposal.login,
            &proposal.table(),
        ) {
            Ok(after)
        } else {
            Err(unwritten())
        };
    }
    added(text, doc, &proposal.persona, name, line, &proposal.table()).ok_or_else(unwritten)
}

/// **The project's file `text`, parsed as `doc`, with line `name` = `line` added to `persona`'s
/// tools** (#4595, #5116), where none of that name is there: none where the tools aren't a
/// table, or the file after it holds anything but what it held and this one line, read back and
/// compared.
fn added(
    text: &str,
    mut doc: toml_edit::DocumentMut,
    persona: &str,
    name: &str,
    line: toml_edit::Value,
    table: &toml_edit::InlineTable,
) -> Option<String> {
    let tools = doc
        .as_table_mut()
        .get_mut("sandbox")?
        .as_table_mut()?
        .get_mut("personas")?
        .as_table_mut()?
        .get_mut(persona)?
        .as_table_mut()?;
    match tools.get_mut(tools::KEY) {
        None => {
            let mut inline = toml_edit::InlineTable::new();
            inline.insert(name, line);
            inline.fmt();
            tools.insert(tools::KEY, toml_edit::value(inline));
        }
        Some(toml_edit::Item::Table(listed)) if !listed.contains_key(name) => {
            listed.insert(name, toml_edit::Item::Value(line));
        }
        Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(listed)))
            if !listed.contains_key(name) =>
        {
            listed.insert(name, line);
            listed.fmt();
        }
        Some(_) => return None,
    }
    let after = doc.to_string();
    only_this_line(text, &after, persona, name, table).then_some(after)
}

/// **The project's file `text` with secret-fill line `name` = `table` in
/// `[sandbox.personas.<persona>]`'s tools** (#5116), or why not: a line of that name there
/// already, a file whose parts aren't tables, or one with no `[sandbox]`. Read back and
/// compared, as a login's line is ([`with_line`]).
pub fn with_tool_line(
    text: &str,
    persona: &str,
    name: &str,
    table: &toml_edit::InlineTable,
) -> Result<String, String> {
    let unwritten =
        || format!("purlis could not add {name} to the project's settings, and wrote nothing");
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|_| unwritten())?;
    let sandbox = doc
        .as_table_mut()
        .get_mut("sandbox")
        .and_then(toml_edit::Item::as_table_mut)
        .ok_or_else(unwritten)?;
    let personas = sandbox
        .entry("personas")
        .or_insert_with(|| {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            toml_edit::Item::Table(table)
        })
        .as_table_mut()
        .ok_or_else(unwritten)?;
    personas
        .entry(persona)
        .or_insert_with(toml_edit::table)
        .as_table_mut()
        .ok_or_else(unwritten)?;
    added(
        text,
        doc,
        persona,
        name,
        toml_edit::Value::InlineTable(table.clone()),
        table,
    )
    .ok_or_else(unwritten)
}

/// Login `name` as `persona`'s list in the project's file `text` declares it, read by the reader
/// every declaration goes through.
fn declared_in(text: &str, persona: &str, name: &str) -> Option<Tool> {
    let top: toml::Table = text.parse().ok()?;
    let line = top
        .get("sandbox")?
        .get("personas")?
        .get(persona)?
        .get(tools::KEY)?
        .get(name)?
        .clone();
    let mut one = toml::Table::new();
    one.insert(name.to_owned(), line);
    let (tools, _) = tools::keyed(
        Some(&toml::Value::Table(one)),
        "login",
        "the project's settings",
        &[],
    );
    tools.into_iter().next()
}

/// Whether the project's file `text` has the `[sandbox]` [`with_line`] writes in.
pub fn has_a_sandbox(text: &str) -> bool {
    text.parse::<toml::Table>()
        .is_ok_and(|top| top.get("sandbox").is_some_and(toml::Value::is_table))
}

/// Whether `after` is `before` with this one line, `name` = `table`, whole, added or in place of
/// its old line.
fn only_this_line(
    before: &str,
    after: &str,
    persona: &str,
    name: &str,
    table: &toml_edit::InlineTable,
) -> bool {
    let (Ok(mut expected), Ok(after)) =
        (before.parse::<toml::Table>(), after.parse::<toml::Table>())
    else {
        return false;
    };
    let Ok(line) = format!("x = {table}\n").parse::<toml::Table>() else {
        return false;
    };
    let Some(added) = line.get("x") else {
        return false;
    };
    let mut at = &mut expected;
    for step in ["sandbox", "personas", persona, tools::KEY] {
        let next = at
            .entry(step)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let Some(table) = next.as_table_mut() else {
            return false;
        };
        at = table;
    }
    at.insert(name.to_owned(), added.clone());
    after == expected
}

#[cfg(test)]
#[path = "loginask/tests.rs"]
mod tests;
