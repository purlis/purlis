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

// A branch folder another chat claimed first is not this chat's to write (M75-25, #7034; ADR 0125,
// amended 2026-11-27): `context.owner` is the host's, from the folder's first claim, which only the
// app seals. "none" (no record, or a lost one) is decided as today. The one-writer mutex beneath
// it is protocol, outside every mode: this rule follows `guards.git`'s.
@id("guards-git-claimed-folder") @level("project")
forbid (principal, action in [Action::"commit", Action::"update", Action::"discard"], resource)
  when { context.owner == "task" || context.owner == "asker" || context.owner == "other" };

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

// ── mode (M75-36; Dispatch #42, Mode #6, #9) ───────────────────────────────────────────────
// A task is capped at auto: one started with its prompts off is above it. The person's own chat
// is never judged by this (attended). `context.harness_mode` is the host's start record, never
// the chat's word (Q320e.R6-1).
@id("mode-task-above-auto") @level("project")
forbid (principal, action == Action::"enter", resource)
  when { !context.attended && context.harness_mode == "bypassPermissions" };

// ── runners (M75-36; Dispatch #76, Other B #7, #9) ─────────────────────────────────────────
// Work handed to a runner, and a request handed back from one, is the person's to approve, once.
@id("runners-offload") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"offload", resource);

@id("runners-forward") @level("project") @ask("you") @keep("once")
forbid (principal, action == Action::"forward", resource);

// ── consents (M75-36; Other A, Asks #44) ───────────────────────────────────────────────────
// A report or a promote releases a credential, publishes or sends data off the machine: only the
// window's press makes one, so a chat never does.
@id("consents-window-only") @level("project")
forbid (principal, action in [Action::"report", Action::"promote"], resource);

// ── processes (M75-36): what the floor and the full sandbox add beyond the escape denies ───
// The escape denies (`launchctl` and the rest) are the sandbox level's own step, never a rule
// (§3.5). A Command is read by its program's name, the last part of its path, so `/usr/bin/sudo`
// is `sudo`. A wrapper (`env sudo`, `command sudo`, `sh -c sudo`) is not read: a known gap (slice 37).
@id("processes-elevation") @level("project")
forbid (principal, action == Action::"exec", resource)
  when { context.sandbox != "off"
         && (resource == Command::"sudo" || resource == Command::"doas" || resource == Command::"pkexec"
             || resource == Command::"su" || resource == Command::"sudoedit" || resource == Command::"run0") };

// ── the catch-all (Q316.1; its id, Q326) ──────────────────────────────────────────────────
@id("policy-everything-else") @level("project")
permit (principal, action, resource);
