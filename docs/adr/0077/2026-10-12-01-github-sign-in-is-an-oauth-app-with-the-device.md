# Amended 2026-10-12: GitHub sign-in is an OAuth app with the device flow (#1810, Q13)

The operator ruled, in the onboarding grill of 2026-10-12 (spec #1810, Q13): *"GitHub sign-in
uses an OAuth app with the device flow (no per-org install; sees and creates all the person's
repos); amends ADR 0077's GitHub App choice. GitLab unchanged (OAuth app + PKCE; token for
self-managed)."* This replaces V13's choice of a GitHub App for sign-in, and D-0077a with it.
The text above is left as accepted; where it says "GitHub App" for purlis's sign-in, read
"OAuth app" with the changes below.

**Why.** Onboarding (#1810) lists the person's repos and their organisations' repos, and creates
a repo under the person or an organisation. A GitHub App's user token reaches only owners where
the App is installed, and creating a repo would need a new App permission (Administration:
write) that every installation must approve. An OAuth app needs no installation, and its
`repo` scope covers listing, creating and pushing.

1. **§1, the github.com row:** purlis's registration on github.com is an **OAuth app**, owned by
   the purlis organisation, with **Enable Device Flow on** and **Expire user access tokens on**.
   It has **no client secret**: none is generated, and none ships. The device flow needs none,
   and neither does refreshing a token the device flow gave (GitHub's "Refreshing an access token
   with a refresh token": `client_secret` is *"Required unless the token was generated using the
   device flow"*). The access token lasts eight hours and the refresh token six months, as §3
   assumed. GitHub requires an authorization callback URL. It is registered as
   `http://127.0.0.1/purlis/signin`, and the device flow never uses it. Without a secret, the
   web flow cannot finish anyway. The client id is compiled in, as §1 says.
2. **§2, GitHub's permissions table:** an OAuth app has no permissions. It asks for scopes at
   sign-in, in the device-code request. purlis asks for **`repo read:org`**:
   - `repo` lists the person's private and organisation repos, creates repos, and pushes
     through the credential helper (ADR 0070, amended 2026-10-12). It also covers what the six
     permissions covered: contents, pull requests, issues, checks and statuses.
   - `read:org` lists the organisations the person belongs to, so a repo can be created under
     one. GitHub refuses `GET /user/orgs` to an OAuth token without `read:org` or `user`.

   `workflow` is not asked for. A push that adds or changes a file under `.github/workflows/`
   is refused without it. The ticket that first pushes such a file adds the scope, and GitHub
   asks the person to approve it again. The settings page explains each scope, as FI2 asks.
3. **What this costs, replacing D-0077a's first item:** `repo` reaches every private repo the
   person can reach. The token is the person's own and never reaches an agent (FI3, §7), so
   that reach is theirs. An organisation that turned on **OAuth app access restrictions**
   (GitHub turns them on for every new organisation) shows its private repos to purlis only after
   an owner approves purlis. The repo list says so per organisation, with GitHub's request link,
   and offers the token path. This replaces ADR 0055's "the App is not installed" line.
4. **Known risk and the alternative:** GitHub's best-practices page says *"Don't enable device
   flow without reason"*. Its reason is phishing: anyone can start a flow that shows purlis's
   name. GitHub suggests the authorization code flow with PKCE, with the client secret shipped,
   as GitHub Desktop and VS Code do. That breaks §1's "no client secret anywhere", so it stays
   the recorded alternative. The settings page shows a code only for a flow the person started
   (§3).
5. **Unchanged:** a GHES has no browser sign-in in v1, only a token with the scopes listed
   (#1810, Q10). A per-host registration on a GHES is later work, and it would be an OAuth app
   with the device flow on. Sign-out still cannot revoke a GitHub token, because revocation needs
   the secret. The settings page links to the person's authorised OAuth apps. The notifications
   API still takes only a classic PAT (FW-13). The GitLab rows, §3 to §8, and every rule about
   agents are unchanged.
6. **V16c and webhooks:** purlis's sign-in registration no longer serves webhooks. FG-12a decides
   in its own record what registration the relay uses. V16c's *"its private key never ships"*
   holds trivially, because an OAuth app has no private key.
7. **The registrations** are made by the operator, under the purlis organisation on github.com
   and the purlis group on gitlab.com (#1810, Q12). The steps are on #726.
8. **"What changes where", the FR-4 row:** the first run now has three doors. Creating a project
   asks where it lives: GitHub, GitLab, or only this Mac (#1810, Q1/Q2). The import offer of §5
   moves with sign-in, which is asked inline when an action needs it. The amendment to W10 and
   FR-4 is recorded in `docs/design-system.md` ("The interrupt budget").
