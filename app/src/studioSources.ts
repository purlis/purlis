// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  commands,
  type ExtensionSkillSource,
  type Reviewed,
  type SkillGrant,
  type SkillLevel,
  type SkillRename,
  type SkillSource,
  type SkillUpdateOffer,
  type SourceOrigin,
} from "./bindings";
import { sourceKey } from "./skillStudio";

/**
 * **A source the Sources pages show** (#3092): a git source the project adds, or an extension
 * that declares `skill-source`. Each is browsed, reviewed and installed through the same pages;
 * only the commands they ask differ, and those are here, so no page asks which kind it holds.
 */
export type StudioSource =
  | { kind: "git"; key: string; label: string; source: SkillSource }
  | { kind: "extension"; key: string; label: string; extension: ExtensionSkillSource };

/** **An extension's source's key**: no git source's key starts with a control character. */
export function extensionKey(id: string): string {
  return `\u0001extension\u0000${id}`;
}

/** The key of the source an installed skill came from. */
export function originKey(origin: SourceOrigin): string {
  return origin.kind === "git" ? sourceKey(origin.remote) : extensionKey(origin.id);
}

export function gitSource(source: SkillSource): StudioSource {
  return { kind: "git", key: sourceKey(source.remote), label: source.label, source };
}

export function extensionSource(extension: ExtensionSkillSource): StudioSource {
  return { kind: "extension", key: extensionKey(extension.id), label: extension.label, extension };
}

/** Where a skill installed from `source` came from, as the core names it. */
export function originOf(source: StudioSource): SourceOrigin {
  return source.kind === "git"
    ? { kind: "git", remote: source.source.remote, label: source.source.remote.label }
    : { kind: "extension", id: source.extension.id, label: source.extension.id };
}

/** What a source's page says it was read at: a git source's commit, an extension's revision. */
export function pinWord(source: StudioSource): "commit" | "revision" {
  return source.kind === "git" ? "commit" : "revision";
}

/** What `source` offers now. */
export function browseSource(plane: string, source: StudioSource) {
  return source.kind === "git"
    ? commands.skillSourceBrowse(plane, source.source)
    : commands.skillExtensionSourceBrowse(plane, source.extension.id);
}

/** What is installed from `source`, as `workspace` has it. */
export function installedFrom(plane: string, source: StudioSource, workspace: string | null) {
  return commands.skillSourceInstalled(plane, originOf(source), workspace);
}

/** The review of installing `names` from `source` at `level`, each skill of `without` that only
 *  a hint brings along left out (#5874). */
export function reviewInstall(
  plane: string,
  source: StudioSource,
  names: string[],
  level: SkillLevel,
  without: string[] = [],
) {
  return source.kind === "git"
    ? commands.skillInstallReview(plane, source.source, names, level, without)
    : commands.skillExtensionInstallReview(plane, source.extension.id, names, level, without);
}

/** The review of installing `names` from `source` together, at `level`, each skill of `without`
 *  that only a hint brings along left out (#5874). */
export function reviewInstallSet(
  plane: string,
  source: StudioSource,
  names: string[],
  level: SkillLevel,
  without: string[] = [],
) {
  return source.kind === "git"
    ? commands.skillInstallSetReview(plane, source.source, names, level, without)
    : commands.skillExtensionInstallSetReview(plane, source.extension.id, names, level, without);
}

/** Installs what was reviewed from `source`: an extension's from what its review read. */
export function installFrom(
  plane: string,
  source: StudioSource,
  level: SkillLevel,
  reviewed: Reviewed,
  grant: SkillGrant | null,
  renames: SkillRename[],
) {
  return source.kind === "git"
    ? commands.skillInstall(plane, source.source, level, reviewed, grant, renames)
    : commands.skillExtensionInstall(plane, source.extension.id, level, reviewed, grant, renames);
}

/** The review of the update `offer`. */
export function reviewUpdate(plane: string, offer: SkillUpdateOffer) {
  return offer.source.kind === "git"
    ? commands.skillUpdateReview(plane, offer.level, offer.name)
    : commands.skillExtensionUpdateReview(plane, offer.level, offer.name);
}

/** Accepts the update `offer`, as reviewed: an extension's from what its review read. */
export function acceptUpdate(
  plane: string,
  offer: SkillUpdateOffer,
  reviewed: Reviewed,
  grant: SkillGrant | null,
) {
  return offer.source.kind === "git"
    ? commands.skillUpdate(plane, offer.level, offer.name, reviewed, grant)
    : commands.skillExtensionUpdate(plane, offer.level, offer.name, reviewed, grant);
}
