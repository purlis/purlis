# One sign-in, then every chat's git and forge work unblocked

Research, 2026-11-20. Facts first, then the options compared. The decision is the proposed
ADR 0128, which the steward and the person rule on. Repo facts are read from `origin/main` at
`4568a3c7b4`. Forge facts are read on 2026-11-20 from docs.github.com (the GitHub Enterprise
Server pages are the 3.22 edition) and docs.gitlab.com (which describes GitLab up to 19.x).
Each fact says which page it comes from. Where a page does not say, this note says "not
documented" and lists it under "Unverified" at the end. Nothing was run against a live forge.

**The requirement.** A person signs in to a forge account with purlis. A chat then runs `gh`,
`glab` and native `git fetch`, `pull` and `push` with no command the person runs by hand.
**Anyone with access** to an organization or personal repo (owner, member, outside collaborator,
read-only) must be served, on github.com, GHES, gitlab.com and self-hosted GitLab.

> **Round 2 (2026-11-20).** Sections 1 to 3 are facts and stand. **Section 4's recommendation,
> (b), and section 5 are superseded** by the steward's ruling: per-user tokens are the base
> (section 6), and the token never enters the chat. Round 1's reviewers rejected the per-owner
> App and GitLab project tokens because they do not serve every collaborator and bind a token by
> installation, not by the person's access. Read section 6 first.

> **Round 3 (2026-11-20).** The person rejected two modes. **Sections 4, 5 and 6 are superseded
> by section 7**, which holds the facts for ADR 0128's one approach (an OAuth app sign-in on
> every host, renewed by the app, brokered by one proxy). Sections 1 to 3 stay as facts; the
> GitHub App and bot-token material in them is now history.

## 1. GitHub

### 1.1 Creating an App without a browser

- **There is no REST endpoint that creates a GitHub App.** The Apps REST page
  (docs.github.com/en/rest/apps/apps) lists reads, installation tokens and "Create a GitHub App
  from a manifest", which only completes a handshake with a code. Registering is done in the UI,
  or by the manifest flow.
- **The manifest flow needs the person's click on GitHub's page.** The caller sends the person's
  browser as a POST to `https://github.com/settings/apps/new` (personal account) or
  `https://github.com/organizations/ORG/settings/apps/new` (organization). The person may edit the
  name, and "when the person clicks Create GitHub App, GitHub redirects back to the
  `redirect_url` with a temporary code". The code is exchanged at
  `POST /app-manifests/{code}/conversions` for the App id, private key (`pem`) and webhook secret
  ("Registering a GitHub App from a manifest").
- **Limits.** All three steps must finish within one hour. The conversion endpoint is rate
  limited. The App is owned by whoever clicked, who "can choose to edit the registration's
  settings, delete it, or transfer it". `public` is a manifest field: true means other accounts
  may install it, false means "only accessible to the owner of the app" (same page).
- **GitHub Enterprise Server has the same flow** (3.22 docs), with one note: manifests "are not
  available for enterprise-owned GitHub Apps".

### 1.2 Who may create and who may install

- **Registering.** An App can be registered under a personal account, "under an organization you
  own", or under an organization that "has granted you permission to manage all its apps" (an
  App manager). A user or organization may register up to 100 Apps ("Registering a GitHub App").
- **App manager is not an installer.** "The GitHub App manager role does not grant recipients
  access to install and uninstall GitHub Apps on an organization" ("About GitHub App managers").
- **Installing** ("Installing a GitHub App from a third party"):
  - Anyone may install on their personal account.
  - Organization owners may install on the organization.
  - A repository admin may install it in the owning organization only when the App asks for no
    organization permissions and not "Repository administration", and only on repositories they
    administer. An organization owner can switch this off.
  - Other members and outside collaborators may pick the organization during install, and GitHub
    then sends the owner a request: the button reads "Install and request" or "Request". The
    owner may change the repositories chosen.
  - Organization owners can turn member requests off. They are on by default ("Limiting OAuth
    app and GitHub App access requests and installations").
- **Private against public decides where an App can be installed.** A private App "can only be
  installed on the account that owns the app"; a public one "any user on GitHub can install"
  ("Making a GitHub App public or private"). Purlis's manifest today sets `public: false`
  (`forge/agentidentity.rs:139-183`). An App owned by a person's personal account and private
  therefore cannot be installed on an organization. An organization's App must be registered by
  an owner or an App manager. A member who is neither can only register a **public** App under
  their own account and send a request for the owner to install it.
- **The installation is also where the repositories are chosen** (All, or select), and the App
  has read access to public repositories whatever is chosen.

### 1.3 Installation tokens, and the private key

- **What a chat needs.** "The installation access token will expire after 1 hour", and may be
  limited with `repositories` or `repository_ids` and `permissions`, never beyond what the App
  was granted ("Authenticating as a GitHub App installation"). It "can also [authenticate]
  HTTP-based Git access" with the App's Contents permission, used as the HTTP password with the
  username `x-access-token` (same page). So the one token serves `gh`, git and the REST API.
- **Minting needs the App's private key.** The App signs a JWT with it ("Managing private keys
  for GitHub Apps"). Keys "do not expire", up to 25 per App, and "grant access to every account
  that the app is installed on" (best practices).
- **A public App's key cannot ship.** GitHub says: "If your app is a native client, client-side
  app, or runs on a user device… you must never ship your private key with your app", and "you
  should not generate installation access tokens since doing so requires a private key. Instead,
  you should generate user access tokens" ("Best practices for creating a GitHub App"). So a
  purlis-published App whose key purlis keeps must keep it on a server purlis runs. That server
  would sign for every installation of the App.
- **The per-owner flow does not have that problem.** The key is made for the owner's own App by
  the conversion step, and goes to the owner's vault on the owner's machine (ADR 0077 as amended
  2026-10-29, E2). It signs only for that owner's installations.

### 1.4 User-to-server tokens as the narrow identity

- Prefix `ghu_`. Expires after 8 hours with a `ghr_` refresh token that lasts 6 months, when the
  App has "Expire user authorization tokens" on (the default) ("Refreshing user access tokens").
  The refresh needs the App's client secret "unless the user access token was generated using
  the device flow".
- The token reaches only "resources that both the user and app can access", in accounts where the
  App is installed ("Authenticating with a GitHub App on behalf of a user"; "Generating a user
  access token"). It is attributed to the person ("the logs will list the user as the actor").
- **Narrowing per token.** The web and device flows take one `repository_id`. Narrowing an
  existing token to several repositories or fewer permissions is `POST
  /applications/{client_id}/token/scoped`, which uses Basic authentication with the App's
  **client secret** (REST apps page, "Create a scoped access token").
- The device flow must be switched on in the App; GitHub says not to unless the setting is a
  CLI or headless ("Don't enable device flow without reason"), because the flow lets an attacker
  impersonate the App in a phishing attack.
- A user token is a second credential **of the person**: the person authorises it, and it acts
  as them within the installation. No chat could get a narrow one without the person authorising
  again for each repository, and GitHub's "Best practices" say a public client "cannot secure
  your client secret".

### 1.5 GitHub Enterprise Server parity

- The manifest flow, installation tokens and user tokens all exist on GHES 3.22 (the same
  pages). Registration is on the GHES host, so an owner of an organization on that host does it
  (ADR 0077 section 1, `0077-…md:116`). GHES API paths differ (`/api/v3`), and a server may
  restrict Apps through its policy.
- Purlis mints on github.com only today: `HOST = "github.com"` (`apptoken.rs:52-54`), and `gh`
  reads a GHES token from `GH_ENTERPRISE_TOKEN` with `GH_HOST` (#3882).

## 2. GitLab

### 2.1 Project and group access tokens through the REST API

- **Create.** `POST projects/:id/access_tokens` with `name`, `scopes`, optional `access_level`
  (10 Guest, 15 Planner, 20 Reporter, 25 Security Manager, 30 Developer, 40 Maintainer,
  50 Owner; default 40) and `expires_at`. "You cannot create a token with an access level greater
  than your account." (docs.gitlab.com/api/project_access_tokens). Groups: `POST
  /groups/:id/access_tokens`, with the same fields (docs.gitlab.com/api/group_access_tokens).
- **Who may.** The project's Maintainer or Owner role; the group's Owner role
  ("Project access tokens", "Group access tokens").
- **Tier.** On GitLab.com both need **Premium or Ultimate**; during a trial a project is limited
  to one token. On Self-Managed and Dedicated they are available "with any license". So
  **GitLab.com Free cannot have a bot token of either kind.**
- **What authenticates the call.** The project API page says: "You must use a personal access
  token with this endpoint. You cannot authenticate with a project access token." It does not say
  whether an OAuth token is accepted. That is the first thing to test (gap G1).
- **The token's identity.** It is a separate (bot) identity: "treated as internal users", with
  its own role. It authenticates the REST API and "Git over HTTPS", with any non-blank username
  and the token as password. It "cannot… access resources in other projects, or… create other
  group, project, or personal access tokens" (user docs). A group token covers the group's
  projects.
- **One project each.** A project access token is scoped to one project. A chat with several
  GitLab clones needs one token per project, or one group token.
- **Lifetime.** Expires at midnight UTC on `expires_at`. Without it, 365 days. The most an
  administrator may allow is by default 365 days, 400 on GitLab 17.6 and later behind a flag
  (user docs). There is a rotate call, which "immediately revokes the previous token and creates
  a new token" (project tokens API). GitLab has no token that lives an hour, so the narrow
  identity is **per project, not per chat**, and lasts days to months (ADR 0077 amended
  2026-10-29, E5).
- **Scopes.** `api` for `glab`, `write_repository` for git over HTTPS. The API lists the scopes
  as a parameter and does not fix their names in the page read. Pick the least set: `api` and
  `write_repository` for a chat that opens merge requests (to be confirmed, gap G6).
- **Pushing to protected branches.** The token has the role given. A Developer pushes to a
  protected branch only where the branch rule allows Developers ("Allowed to push and merge",
  docs.gitlab.com/user/project/repository/branches/protected). A chat's work goes to a branch
  and a merge request, which a Developer can do.

### 2.2 Personal access tokens

- `POST /users/:user_id/personal_access_tokens` needs an instance administrator and only exists
  on Self-Managed and Dedicated ("User tokens API").
- `POST /user/personal_access_tokens` is "for your account", and takes only the `k8s_proxy` and
  `self_rotate` scopes or **`granular_scopes`** (introduced 19.2). A granular scope has `access`
  (`selected_memberships`), `permissions`, and `project_ids` or `group_ids` ("you must have
  access to each group and project you list"). **Fine-grained personal access tokens are
  Free, Premium and Ultimate, on GitLab.com too, generally available in 19.2.** They are still the
  **person's own identity**, narrowed: every request must pass the token's permission *and* "you,
  as the token owner, can perform that operation" (docs.gitlab.com/auth/tokens/
  fine_grained_access_tokens). That is #7215's "Free takes a Developer user's token", and its
  attribution is the person.
- The page read does not say whether this endpoint accepts an OAuth token (gap G1 again).

### 2.3 Purlis's own OAuth token

- GitLab OAuth gives an access token of 7200 seconds and a refresh token, by authorization code
  with PKCE or the device flow (17.1 and later) (docs.gitlab.com/api/oauth2). Its scopes are
  what the application is registered with and the request names. **The scopes cannot be
  narrowed to a project**: a scope is a kind of API access, not a repository. A token minted for a
  chat from the person's OAuth token would have the person's whole reach.
- So the OAuth token can be the **credential that creates** a project or group token, never the
  chat's token.

## 3. Repo facts

- **ADR 0077.** FI3: *"The human's UI token never reaches an agent; agents keep their own narrow
  identity (SD-7a/b…)"* (`0077-…md:11-12`). V16c: per-agent GitHub tokens *"come from a GitHub
  App the organisation registers itself… charter's shared GitHub App (FW-1) is for sign-in only
  and its private key never ships"* (`:18-20`); purlis's registration *"serves no agent, ever"*
  (`:33`, `:279-281`). Amendment 2026-10-12-01: GitHub sign-in is an **OAuth app with the device
  flow**, owned by purlis, with no client secret. Amendment 2026-10-29-01 builds SD-7a on this
  machine: the org's own App, key in a vault, marked so nothing but the mint reads it, a token
  per chat limited to the chat's repos. It lasts an hour. **GitLab is "for now" a token the
  organisation makes** (SD-7b).
- **ADR 0070, amended 2026-10-12-02** (#752): purlis's own git credential helper serves the
  person's account to the git the window runs, "never inside a chat's process tree", and "a call
  the app runs for a chat (a brokered call) never gets it" (`gitcredential.rs:12-17`, `:319-330`).
  ADR 0070 itself says SD-7a/b "close that gap" (`:66-68`).
- **ADR 0067**, amended 2026-11-19 (#7241): a chat that claims the forge lock has an empty
  `GH_CONFIG_DIR` and `GLAB_CONFIG_DIR`, git's `credential.helper` emptied after the person's own
  config is included, `GIT_CONFIG_SYSTEM=/dev/null`, `GIT_TERMINAL_PROMPT=0`, an ssh agent
  that is empty, browser logins to a forge refused, and no harness connector. **After #7241 a
  locked chat's own `git push` over HTTPS finds no credential at all**, which is the right
  closed door and the reason this feature is needed.
- **Mint.** `apptoken.rs`: the token goes to the tool declared with `mint = "github-app"` as
  `GH_TOKEN`, and nowhere else. No path hands it to git (`x-access-token` appears in no push
  path). github.com only (`apptoken.rs:11-13`, `:52-54`).
- **The brokered push.** `purlis worktree push` → `app/src-tauri/src/gitbroker.rs:239-257` →
  `pieceupdate.rs:1863` → `repocmd/sync.rs:147-157` → `forge::helper_for`
  (`forge.rs:806-823`), which with no window scope on the thread returns the `gh` or `glab`
  CLI's helper: **the person's own CLI login** (read from the code, not run).
- **The agent identity flow** (`forge/agentidentity.rs`, `app/src/AgentIdentityFlow.tsx`) is the
  manifest flow of section 1.1 with a private manifest, a loopback listener and a 4-second poll
  for the install. It is a Settings › Agent identity tab and a step of Pool setup.
- **Open issues.** #1351 proposed purlis's helper for sandboxed chats, answering *"from the
  operator's own helper or keychain"*. #2053 is SD-7a/b. #7196: a clone made from the CLI is
  silently left out of a mint. #7210: refuse a mint for an unlocked repo. #7215: a Developer-role
  GitLab chat credential, with "Free takes a Developer user's token (design 3)". #3882: GHES
  mint. #7230: a vault `glab` token handed as `GITLAB_TOKEN` and `GITLAB_HOST`.

## 4. Options for decision 3 (GitHub)

| | (a) A purlis-published App and a purlis token service | (b) The per-owner manifest flow, folded into sign-in | (c) A purlis-published App, user tokens only |
|---|---|---|---|
| What the chat holds | An installation token, 1 hour, limited to its repos | The same | A user token, attributed to the person |
| Where the private key is | A server purlis runs, for every installation | The owner's vault, one owner's installations | None |
| Server | Yes: a signing service, with its own sign-in, rate limits and uptime | None | None (the device flow needs no secret) |
| ADR 0077 V16c | Contradicted: purlis's App serves an agent | Kept | Contradicted in part, and FI3 stretched |
| Per-chat narrowing | Yes | Yes | Not without the person authorising again, or a client secret that cannot ship |
| Works with the lock and GHES | A GHES server cannot reach purlis's App unless it is published there too | Yes, the App is registered on the host | GHES needs the same publishing |
| Clicks | Install (and a request to the owner) | Create App, Install | Authorise, Install |
| Org without owner rights | Request to the owner, who trusts purlis's App | Request to the owner, who trusts the person's own App | Same |
| Cost to purlis | A service to run, secure, and be answerable for | The flow exists; fold it in and make the App public where needed | Least code, widest token |

**Recommendation: (b).** The facts that decide it:

1. GitHub's own advice for software on a user's device is "never ship your private key"
   (section 1.3). (a) makes purlis the holder of a signing key for **every installation**: one
   breach is every customer's repositories. It also makes a server a precondition for git to
   work. ADR 0077 amended 2026-10-29 chose the opposite, a key the organization holds itself.
2. (c) cannot narrow per chat without the App's client secret (section 1.4), and its token is the
   person's authority. It would break FI3 in spirit.
3. (b) keeps ADR 0077's design and its amendment E2, and needs no new server. The flow, the
   vault key, the mint and the "waiting for your admin" step are built. What (b) needs is
   folding into sign-in, a **public** App where the person is not an owner (section 1.2), and the
   token's new route to git.

## 5. What cannot be done as the requirement is worded

1. **"Nothing extra beyond signing in" is not possible for GitHub.** GitHub requires the
   person's click on "Create GitHub App" and then on "Install" (sections 1.1 and 1.2). The least
   is two page confirmations after the sign-in's own device-code approval. The flow can run
   straight on from the sign-in, with no Settings step and no command, which is the
   requirement's intent.
2. **No org rights means waiting, on both forges.** A GitHub organization's owner must install
   the App (or approve the request), and a GitLab project needs a Maintainer and a group an
   Owner (sections 1.2 and 2.1).
3. **GitLab.com Free cannot have a narrow bot identity.** Project and group access tokens need
   Premium on GitLab.com (section 2.1). Only the person's own fine-grained token (19.2 and
   later) is narrowed by project (section 2.2), and it is the person's identity. Decision 6 says
   never to fall back to the person's login, so this needs the steward's ruling (ADR 0128 gap 2).
4. **A GitLab chat's token is per project and long-lived**, not per chat and an hour. That is
   GitLab's limit, not a choice (section 2.1).

## Unverified (needs a probe, or a person with a live account)

- **G1.** Whether `POST /projects/:id/access_tokens` and `POST /user/personal_access_tokens`
  accept purlis's **OAuth** bearer token. The docs say "personal access token". If the answer is
  no, decision 4 cannot be built as worded.
- **G2.** That `x-access-token` with a `ghu_` or installation token works with every git
  operation the chat needs (the docs say so for installation tokens, with Contents).
- **G3.** Whether a repository admin who is not an owner can install the App on an
  organization's repository, as section 1.2's rule says for an App with no organization
  permissions. Purlis's manifest asks only repository permissions, but this was not tried, and an
  owner may have switched the rule off.
- **G4.** GHES: the exact `/api/v3` paths of the manifest conversion and the installation
  endpoints on a server older than 3.22.
- **G5.** Whether an installation token pushes to a branch protected by a rule that requires a
  pull request, and whether the App may be put on the bypass list. It does not push to a
  protected default branch without it.
- **G6.** The exact scope names a GitLab project token needs for `glab` plus git push.

## 6. Round 2: per-user tokens, brokered

What the ruling needs, from the facts above and from what is not yet read.

**GitHub, a public App for agents, device flow, user tokens (sections 1.2, 1.4).**

- It is option (c) of section 4, now the base. Section 4 rejected (c) on two grounds: the token
  cannot be narrowed per chat without a client secret, and it is the person's authority. Neither
  matters when **the token never leaves the app**: the narrowing is the proxy's, and the
  attribution to the person is wanted.
- The device flow needs the client id only. Its refresh "needs the client secret unless the user
  access token was generated using the device flow" (section 1.4), so a device-flow token
  refreshes with no secret. The refresh token is single use; the app must be the only refresher.
- The token is limited to what **both the App and the person** may reach, in installed accounts
  (1.4). An organization owner installs once; a member's install is a request the owners get
  (1.2). On someone else's personal account only the owner can install: a collaborator cannot.
- The device flow is an App setting; section 1.1's manifest fields were read with no device-flow
  field. For purlis's own App, staff tick it once. For a GHES App, the host's registrant must
  tick it by hand. **Unverified** against a live GHES.
- No `workflows` permission: GitHub then rejects a push that changes `.github/workflows`.

**GitLab, an agent OAuth application (sections 2.2, 2.3).**

- 2-hour access token and a rotating refresh token (2.3). Scopes name a kind of API access, not a
  project, so the project narrowing is the proxy's, or the fine-grained token's (2.2).
- A fine-grained personal access token can be limited to named projects and permissions and acts
  as the person (2.2). **Whether `POST /user/personal_access_tokens` accepts an OAuth token is
  unverified (G1).** If it does, the app can make one per chat, delete it at the end and omit
  `self_rotate`.
- A user-owned OAuth application (no administrator) is what a self-hosted host needs. Where the
  sign-in there is a pasted PAT (`forge/registration.rs:18`), it has the person's whole scopes and
  can only be narrowed by the proxy.

**Why the git credential helper cannot hold the token (new).** A helper answers whoever runs it,
including a chat that runs `git credential fill`. The token reaches the chat the moment a helper
answers with it. So the helper answers a stand-in and a proxy swaps (the key-proxy pattern of
#6762 and #6812, whose issues are open and which has no code in `crates/` at `4568a3c7b4`).

**What is not known (the probes).**

- **P1.** How `gh` (github.com has no base-URL variable) and `glab` reach a local proxy. `gh` has a
  per-configuration unix-socket option; `SSL_CERT_FILE` is ignored by Go on macOS. Neither read in
  a live `gh` or `glab` here.
- **P2.** Kernel peer identity for a TCP loopback connection (no `SO_PEERCRED`); `hookwire/reach.rs`
  reads a unix socket.
- **P3.** G1, and a manifest field for the device flow.
- **P4.** SAML organizations, enterprise-owned organizations, whether a user token needs a
  per-organization authorisation.
- **P5.** Whether GitLab or GitHub forks are enabled for a read-only person on a given host.

**Not run:** the sandbox this note was written in refused a fetch of the forge docs, so no fact in
this section was re-read on 2026-11-20 beyond sections 1 to 3. Re-read each before the ADR is
accepted.

## 7. Round 3: one approach (facts)

Re-read on 2026-11-20 by `curl` to stdout from docs.github.com and docs.gitlab.com (all pages
below fetched in this session; the GHES pages are the 3.22 edition). Tool facts are from the
`gh` 2.69.0 and `glab` 1.119.0 installed here, read from their own help and binary, not run
against a forge. **The sandbox refused every loopback bind and connect and the name
`api.github.localhost`, so no proxy run was made, and none was worked around.**

### 7.1 GitHub: OAuth app against GitHub App user token

- **Who may approve an OAuth app, and what restricts it** (*About OAuth app access restrictions*).
  "When you create a new organization, OAuth app access restrictions are enabled by default."
  With restrictions on, "organization members and outside collaborators cannot authorize OAuth
  app access to organization resources. Users can request owner approval... and organization
  owners receive a notification". Owners choose whether outside collaborators may request. An
  organization with no restrictions: "any OAuth app authorized by an organization member can also
  access the organization's private resources", and on authorizing, "the application will
  automatically be authorized" (*Authorizing OAuth apps*). An unapproved app reaches only the
  organization's public resources. Approval is the owner's, per organization.
- **Privileged OAuth apps** (*Privileged OAuth apps*): GitHub-owned apps, among them GitHub CLI,
  Git Credential Manager, GitHub Desktop and Visual Studio Code, which a user "can still authorize
  ... even if an organization owner has restricted access". purlis is not on the list and must
  not use another vendor's client id.
- **Reach** (*Differences between GitHub Apps and OAuth apps*): an OAuth token has access to "the
  user's accessible resources", "loses access to resources when the user loses access", and is
  "limited via scopes". A GitHub App must be installed per account and repository; installing "on
  your personal repository" is the owner's; "GitHub Apps aren't subject to organization application
  policies", but need an install, which a non-owner can only request.
- **Scopes** (*Scopes for OAuth apps*): `repo` is full access to code, statuses, invitations,
  collaborators, deployments and repository webhooks; `workflow` is separate ("the ability to add
  and update GitHub Actions workflow files"); `delete_repo`, `write:packages` and `admin:repo_hook`
  are separate. "You can't scope source code access to read-only."
- **Expiry and refresh** (*Authorizing OAuth apps*): tokens do not expire unless the app opts in,
  by an app setting or by requesting `offline_access` per sign-in; then "the access token expires
  after eight hours, and the refresh token expires after six months without use", both replaced
  on each refresh. The refresh's `client_secret` is "required unless the token was generated using
  the device flow". The device flow must be enabled on the app. Limits: ten tokens per
  user/application/scope, ten created per hour. If the refresh token expires, "send the user
  through the web application flow or device flow again".
- **SAML** (*About authentication with SAML single sign-on*): "You must have an active SSO session
  each time you authorize an OAuth app or GitHub App", and after SSO is enforced "you must
  reauthorize any OAuth apps or GitHub Apps".
- **Creating one** (*Creating an OAuth app*, github.com and GHES 3.22): any user, "under your
  personal account or any organization you have administrative access to", with an "Enable Device
  Flow" checkbox. No administrator.
- **Not read:** whether GHES 3.22 honours `offline_access`; the exact text of a restricted
  organization's refusal; whether an owner's approval widens an existing token; the "Requesting
  organization approval" and "Limiting requests" pages (both returned 404 at the paths tried).

### 7.2 GitLab

- **OAuth** (*OAuth 2.0 identity provider API*): access token 7200 s, a refresh token that "may be
  used even after the access_token itself expires" and invalidates the pair it replaces; PKCE
  needs no client secret; the device authorization grant is in 17.1 and later, generally
  available in 17.9. Scopes name a kind of access, not a project: `api`, `write_repository`
  ("read-write access to repositories on private projects using Git-over-HTTP (not using the
  API)"), `read_user`. The page states no refresh-token lifetime.
- **Self-managed** (*Configure GitLab as an OAuth 2.0 authentication identity provider*): a **user
  owned application** is made from Edit profile › Access › Applications, "user owned, group owned
  and instance-wide" exist, and the administrator-only kind is the instance-wide one. From 19.6 an
  upgrade also creates instance-wide applications for `glab`, the VS Code extension and the
  JetBrains plugin, with their gitlab.com client ids. Access-token expiry is administrator
  configurable from 19.1.
- **The administrator's switch** (*Application settings API*; *Account and limit settings*):
  `user_oauth_applications` — "Allow users to register any application to use GitLab as an OAuth
  provider. This setting does not affect group-level OAuth applications."

### 7.3 What `gh` and `glab` can be pointed at (read here; not run against a forge)

- **`gh` 2.69.0.** `help config` lists `http_unix_socket` ("a Unix socket through which to make an
  HTTP connection"), with no CA setting. `help environment` lists `GH_HOST`, `GH_TOKEN`,
  `GH_ENTERPRISE_TOKEN` and `GH_REPO` (`[HOST/]OWNER/REPO`). The binary's strings hold
  `github.localhost`, `http://api.%s/`, `http://api.%s/graphql`, `https://api.%s/` and
  `https://%s/api/v3/`. With `GH_HOST=github.localhost` a request failed with `error connecting to
  api.github.localhost`; with `GH_HOST=ghe.example.test` it printed `Get
  "https://ghe.example.test/api/v3/user"`. So the host `github.localhost` selects a plain-HTTP,
  `api.`-prefixed route, and an ordinary enterprise host never does.
- **`glab` 1.119.0.** `config --help` lists `host` (`GITLAB_HOST`), `api_host` (`GITLAB_API_HOST`,
  "hostname or hostname:port"), `api_protocol` (`GLAB_API_PROTOCOL`, `http`/`https`), `token`
  (`GITLAB_TOKEN`), `ca_cert` (`GLAB_CA_CERT`), `client_id` and `custom_headers`.
- **TLS.** Go on macOS reads no `SSL_CERT_FILE`, which is why a MITM certificate is rejected and
  the design avoids TLS to the chat's tools. That is from the Go documentation as known, not
  re-fetched here.
- **The sandbox itself.** A chat cannot bind a loopback port (`Operation not permitted`) and is
  refused a connect to a loopback port its profile does not list. That is what makes a per-chat
  port an identity (ADR 0128 D3d), and it is why the proxy run could not be made here.

### 7.4 Still unproven

A live `gh` and `glab` through the proxy (the first task of the build); git's proxying of
`*.localhost`; GHES `offline_access`; an owner's approval widening an existing token; the exact
refusal text of a restricted organization; the Linux sandbox's one-port-per-chat.
