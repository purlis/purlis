### Added

- **Create the profile a persona names, from its tab.** A persona whose definition names a
  profile this machine does not have (`profile: qa-bot`) now says so on its tab and offers
  **Create profile qa-bot…**: pick the harness and press Create, and the profile is made as
  Settings › You › Profiles' Add profile makes one, ready to sign in with your own account.
  Nothing is made until you press Create. Dispatch lists, the approval of a clone's own
  profiles and the refusal of a command that switches permission prompts off hold for it as
  for any other profile (#2039).
- **A harness declaration lists the plans a subscription signs in to.** `[sign_in] plans =
  [{ provider = "openai", title = "ChatGPT" }, …]` is an allow-list of the plans Sign in offers,
  passed as the harness's own provider id; a harness that lists none signs in to its own
  vendor's plan. opencode's built-in declaration now lists ChatGPT and GitHub Copilot, and
  `anthropic` is refused in any declaration but Claude Code's own (#2039).
