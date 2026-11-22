### Security

- **A chat's write to the policy that governs it is held** (M75-16, #7025). A file tool or a shell
  command that writes the plane root's `policy.cedar`, or that changes `[policy]` in the project's
  settings, is refused and names the rule, `policy-self-change`, under every mode and template: it
  is one of the three things outside the policy. A chat's sandbox now holds `policy.cedar` at the
  root as it holds the settings files. A branch folder's own copy stays the chat's to edit and
  commit, and a commit that loosens the policy still changes nothing until you press Accept.
  The hold fails closed under every mode when its guard dies, reads case-folded spellings on
  macOS, sees a leading redirection, `--output` of a git reader and a settings file copied into the
  root, and holds any other tool's write by its path.
