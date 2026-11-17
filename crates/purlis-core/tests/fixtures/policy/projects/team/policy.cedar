// purlis-policy 1
// Five-person team. Rules here sit on top of the shipped default.

// main is protected: no push from any chat, no ask. Replaces the shipped
// ask of the same @id, so it now denies instead of asking.
@id("guards-git-protected-push") @level("project")
forbid (principal, action == Action::"push", resource in BranchSet::"protected-branches");

// Production hosts are asked, every time (shipped default already does
// this; restated so the team can read it here).
@id("network-production-host") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"connect", resource in HostSet::"production-hosts");

// No reading another workspace's files. Replaces the shipped ask with a deny.
@id("sandbox-other-workspace-read") @level("project")
forbid (principal, action == Action::"read", resource)
  when { resource has workspace && resource.workspace != principal.workspace };

// Vault use: Once. Already the shipped default (vaults-use, @keep("once")),
// so nothing is written here.

// The infra workspace wants its deploy chat to push infra's main.
// A workspace-level permit: it can't punch through the project forbid above.
@id("guards-git-infra-deployer-push-main") @level("workspace")
permit (principal in Workspace::"infra", action == Action::"push", resource == Branch::"acme/infra#main");
