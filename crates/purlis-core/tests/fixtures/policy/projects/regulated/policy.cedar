// purlis-policy 1
// Regulated project: deny-by-default. Every allow is written below.
// The shipped forbids and asks still apply on top of these permits.

@id("allow-own-workspace-files") @level("project")
permit (principal, action in [Action::"read", Action::"write"], resource)
  when { resource has workspace && resource.workspace == principal.workspace };

@id("allow-known-hosts") @level("project")
permit (principal, action == Action::"connect", resource in HostSet::"known-hosts");

@id("allow-feature-branch-push") @level("project")
permit (principal, action == Action::"push", resource)
  unless { resource in BranchSet::"protected-branches" || context.force };

@id("allow-own-persona-dispatch") @level("project")
permit (principal, action == Action::"dispatch-to", resource)
  when { resource == principal.persona };

@id("allow-own-vault-use") @level("project")
permit (principal, action == Action::"use", resource) when { resource in principal.persona };

@id("allow-listed-tools") @level("project")
permit (principal, action == Action::"run", resource in ToolSet::"allowed-tools");

@id("allow-listed-skills") @level("project")
permit (principal, action == Action::"load", resource in SkillSet::"allowed-skills");

// Merges are allowed only through the ask (the shipped guards-merge-hold).
@id("allow-merge-via-ask") @level("project")
permit (principal, action == Action::"merge", resource);

// Production is never reached from a chat here.
@id("network-production-host") @level("project")
forbid (principal, action == Action::"connect", resource in HostSet::"production-hosts");
