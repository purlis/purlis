// purlis-policy 1
// The shipped default, a stand-in (M75 slice 01): a handful of the real 150 to 250 rules, which
// later slices write family by family. Every rule is @level("project"), owned by "purlis default
// <version>", and a project's policy.cedar sits on top of it (Q316.2, Q316.5). It allows by
// default: named forbids and asks, then the catch-all permit `policy-everything-else` (Q316.1,
// Q326).

// ── guards ────────────────────────────────────────────────────────────────────────────────
@id("guards-merge-hold") @level("project") @ask("you") @keep("once")
forbid (principal, action in [Action::"merge", Action::"land"], resource);

@id("guards-release-hold") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"release", resource);

@id("guards-git-protected-push") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"push", resource in BranchSet::"protected-branches");

@id("guards-git-force-push") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"push", resource) when { context.force };

// ── sandbox ───────────────────────────────────────────────────────────────────────────────
@id("sandbox-secret-stores") @level("project")
forbid (principal, action in [Action::"read", Action::"write"], resource in PathSet::"secret-stores");

@id("sandbox-other-workspace-read") @level("project") @ask("you")
forbid (principal, action == Action::"read", resource)
  when { resource has workspace && resource.workspace != principal.workspace };

// ── network and browser ───────────────────────────────────────────────────────────────────
@id("network-production-host") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"connect", resource in HostSet::"production-hosts");

@id("network-unknown-host") @level("project") @ask("you")
forbid (principal, action == Action::"connect", resource)
  unless { resource in HostSet::"known-hosts" };

@id("browser-unknown-site") @level("project") @ask("you")
forbid (principal, action == Action::"browse", resource)
  unless { resource in HostSet::"known-hosts" };

// ── vaults and secrets ────────────────────────────────────────────────────────────────────
@id("vaults-use") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"use", resource);

@id("vaults-other-persona") @level("project")
forbid (principal, action == Action::"use", resource)
  unless { resource in principal.persona };

@id("secrets-reveal") @level("project")
forbid (principal, action == Action::"reveal", resource);

// ── dispatch ──────────────────────────────────────────────────────────────────────────────
@id("dispatch-other-persona") @level("project") @ask("you")
forbid (principal, action == Action::"dispatch-to", resource)
  unless { resource == principal.persona };

@id("dispatch-depth") @level("project")
forbid (principal, action == Action::"dispatch-to", resource) when { context.depth >= 3 };

// ── skills ────────────────────────────────────────────────────────────────────────────────
@id("skills-unlisted") @level("project") @ask("you")
forbid (principal, action in [Action::"install", Action::"load"], resource)
  unless { resource in SkillSet::"allowed-skills" };

// ── the catch-all (Q316.1; its id, Q326) ──────────────────────────────────────────────────
@id("policy-everything-else") @level("project")
permit (principal, action, resource);
