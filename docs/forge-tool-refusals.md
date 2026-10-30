# Forge tools: which commands never run with a vault's value

A persona's tool whose name or program is `gh` or `glab` (`[sandbox.personas.<persona>].tools.<program>`,
`docs/plane-format.md`) runs a chat's command line with a vault's value. Some of each command line's
commands never run that way, whatever the tool's `args` say: **those that show a credential or a secret,
or change the forge's credentials or secrets or the program's own settings** (the person's ruling of
2026-10-29, #3421). Run them in your own terminal.

This page is the audit behind that list (#3894): every command of `gh` 2.83.2 and `glab` 1.103.0, and the
routes `api` reaches on the forge's own host. The person's `args` stay the main control; this list only
has to be complete by its own test. Code: `crates/purlis-core/src/sandbox/tools/args.rs` (`Forge::refused`)
and `args/api.rs` (the routes).

A command refused "with" a word is refused where that word is any of the run's arguments; an option word
(`-F`, `--output`) also where its value is joined to it (`-Fjson`, `--output=json`).

**Aliases.** A command is refused by every name its program takes for it. `gh` names its aliases in each
help page; `glab` names none, so its aliases were found by trying names: `conf` (`config`), `var`
(`variable`), `tf` and `terraform` (`opentofu`), `rc` (`runner-controller`), `pipe` and `pipeline` (`ci`),
`project` (`repo`), `sched` and `skd` (`schedule`) and `v` (`version`). An installed extension or an alias the person
defined is never there: a run's home is its own (#3419), so none is installed for it.

A run with `--` among its arguments is refused for both: `repo clone` and `repo fork` hand what follows to
`git clone`, whose own options can name a template (and its hooks), a config or a program to run with the
token in its environment.

## gh 2.83.2

| Command | Decision | Why |
|---|---|---|
| `auth` (all) | refused | shows the token (`token`, `status --show-token`) or changes the sign-in (`login`, `logout`, `refresh`, `switch`, `setup-git`) |
| `config` (all) | refused | gh's own settings |
| `alias` (all) | refused | gh's own settings |
| `extension` / `extensions` / `ext` | refused | gh's own settings; an extension is another program run with the token |
| `ssh-key` (all) | refused | changes the account's keys |
| `gpg-key` (all) | refused | changes the account's keys |
| `secret` (all) | refused | changes Actions, Codespaces and Dependabot secrets; `list` is kept with it, so one rule covers the group and its `api` routes |
| `repo` with `deploy-key` | refused | adds a lasting key to the repository |
| `repo` with `set-default` | refused | writes gh's resolved-repository setting into the folder's git config |
| `variable` | runs | Actions variables are not secrets by GitHub's own definition |
| `codespace` | runs | `ssh` and `cp` keep their key in the run's own home; no command shows a credential |
| `agent-task`, `attestation`, `browse`, `cache`, `completion`, `gist`, `issue`, `label`, `org`, `pr`, `preview`, `project`, `release`, `repo` (other), `ruleset`, `run`, `search`, `status`, `workflow` | runs | none shows a credential or changes credentials or settings |
| `api` | runs, but see below | |

## glab 1.103.0

| Command | Decision | Why |
|---|---|---|
| `auth` (all) | refused | shows or changes the sign-in, including `configure-docker`, `docker-helper` and `dpop-gen` |
| `config` (`conf`, all) | refused | glab's own settings |
| `alias` (all) | refused | glab's own settings |
| `token` (all) | refused | makes, shows, rotates and revokes access tokens |
| `ssh-key`, `gpg-key`, `deploy-key` (all) | refused | changes the account's or project's keys |
| `variable` (`var`, all) | refused | `get`, `list` and `export` show CI/CD variables' values, which may be masked secrets |
| `securefile` (all) | refused | secure files are a project's secrets |
| `duo` | refused | `duo cli` downloads and runs another program with the token |
| `mcp` | refused | `mcp serve` offers a tool for nearly every glab command, the refused ones among them, so a run through it is judged by none of this list |
| `opentofu` (`tf`, `terraform`) with `init` or `download` | refused | `init` runs a chosen program with the token and leaves it in the state backend's settings; a downloaded state often holds secrets in plain text |
| `ci` (`pipe`, `pipeline`) with `--with-variables` | refused | shows a pipeline's variables' values |
| `repo view` (`project view`) with `-F`, `--output` or `--jq` | refused | the project as JSON carries its runner registration token for a maintainer; its lists do not, so `repo list -F json` runs |
| `schedule` (`sched`, `skd`) with `--variable` | refused | sets a pipeline schedule's variables, which may be masked secrets, as `variable` does |
| `orbit local` | refused | downloads a program and runs it with the run's arguments and environment |
| `cluster` with `get-token`, `token`, `token-cache`, `bootstrap` or `update-kubeconfig` | refused | makes or shows an agent's token, or writes kubectl's settings |
| `runner-controller` (`rc`) with `token` | refused | makes, shows and rotates runner controller tokens |
| `opentofu state list`, `lock`, `unlock`, `delete`; `cluster agent list`; `runner` | runs | none shows a credential |
| `skills`, `orbit` (but `local`) | runs | write agent skills into the folder, as a chat can |
| `attestation`, `changelog`, `check-update`, `ci`, `container-registry`, `incident`, `issue`, `iteration`, `job`, `label`, `milestone`, `mr`, `packages`, `release`, `repo` (but as above), `schedule` (but as above), `search`, `snippet`, `stack`, `todo`, `user`, `version`, `whatsnew`, `work-items` | runs | none shows a credential or changes credentials or settings |

## `api` on the forge's own host

`api` with `--hostname` or an endpoint as a whole URL is refused (#3421). On the forge's own host, an
endpoint under one of these routes is refused **whatever the method**, as the command it stands for is
refused whole (`*` is any one segment). Before it is matched, an endpoint's query is cut, each segment is
percent-decoded and lower-cased, empty segments drop, a leading `api/v3` or `api/v4` drops, and
`repositories/<id>` and `organizations/<id>` read as `repos/…` and `orgs/…`. An endpoint is matched every
way a server may read it: split at `/` (and `\`) before and after percent-decoding, with its `.` and `..`
segments as written and as resolved against each API base a server may resolve them from (`/`, `/api/v3`,
`/api/v4`). An endpoint with a placeholder the program fills with a value that can hold `/` (`{branch}`,
`:branch`, `:namespace`, `:group`, `:repo`) is refused, since the path sent is not the path read. A segment matches a route's name with a format after a dot
(`variables.json`), as GitLab routes it. A route ending in `$` is refused alone, not the paths under it.

| Forge | Routes | Stands for |
|---|---|---|
| GitHub | `user/keys`, `user/ssh_signing_keys`, `user/gpg_keys` | `ssh-key`, `gpg-key` |
| GitHub | `repos/*/*/keys` | `repo deploy-key` |
| GitHub | `repos/*/*/actions/secrets`, `…/actions/organization-secrets`, `…/environments/*/secrets`, `…/codespaces/secrets`, `…/codespaces/organization-secrets`, `…/dependabot/secrets`; `orgs/*/actions/secrets`, `…/codespaces/secrets`, `…/dependabot/secrets`, `orgs/*/private-registries`; `user/codespaces/secrets` | `secret` |
| GitHub | `{repos/*/*,orgs/*,enterprises/*}/actions/runners/{registration-token,remove-token,generate-jitconfig}` | makes a runner's credential |
| GitHub | `orgs/*/personal-access-tokens`, `orgs/*/personal-access-token-requests`, `orgs/*/credential-authorizations`, `installation/token`, `app-manifests/*/conversions`, `authorizations`, `applications/*/token`, `applications/*/grant(s)`, `app/installations/*/access_tokens`, `credentials/revoke`, `admin/keys`, `admin/users/*/authorizations` | `auth`: shows, makes or revokes tokens |
| GitLab | `user/keys`, `users/*/keys`, `user/gpg_keys`, `users/*/gpg_keys`, `deploy_keys`, `projects/*/deploy_keys` | `ssh-key`, `gpg-key`, `deploy-key` |
| GitLab | `deploy_tokens`, `{projects,groups}/*/deploy_tokens`, `personal_access_tokens`, `user/personal_access_tokens`, `users/*/personal_access_tokens`, `users/*/impersonation_tokens`, `groups/*/service_accounts/*/personal_access_tokens`, `{projects,groups}/*/access_tokens` | `token` |
| GitLab | `{projects,groups}/*/variables`, `admin/ci/variables`, `projects/*/pipeline_schedules`, `projects/*/pipelines/*/variables` | `variable`, `ci … --with-variables` |
| GitLab | `projects/*/terraform/state` | `opentofu … download` |
| GitLab | `projects/*$`, `groups/*$` (the project's or group's own page) | `repo view --output json`: a runner registration token |
| GitLab | `projects/*/secure_files` | `securefile` |
| GitLab | `projects/*/triggers`, `projects/*/cluster_agents/*/tokens`, `{,projects/*/,groups/*/}runners/reset_registration_token`, `runners/reset_authentication_token`, `runners/*/reset_authentication_token`, `user/runners` | shows or makes a trigger's, agent's or runner's token (`cluster … token`) |
| GitLab | `applications`, `user/support_pin`, `application/settings` | shows an application's secret, a support PIN, or the instance's own keys |

**GraphQL.** GitHub's schema has no mutation that adds or removes a key or a secret; four of its fields show
or make a credential (`tempCloneToken`, `recoveryCodes` and its regenerating mutation,
`verificationToken` and `regenerateVerifiableDomainToken`). `gh api graphql` is refused where its query
names one of them, and where the query is read from a file or standard input (`--input`, `-F query=@…`),
since it is not seen. GitLab's schema shows CI/CD variables' values and makes runner and agent tokens, so
`glab api graphql` is refused whole.

**Not refused**, and why: GitHub's `users/*/keys` and `users/*/gpg_keys` show public keys anyone can read;
GitHub and GitLab webhooks, integrations and mirrors keep their secrets write-only; a runner's
registration (`runners`) is sent the registration token in its body, which is refused wherever it is shown
(above), so the chat holds none from the vault.

When a new `gh` or `glab` release adds commands or aliases, this audit is done again for it, aliases tried by
name for `glab`.
