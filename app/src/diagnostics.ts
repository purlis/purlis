// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  commands,
  type About,
  type DiagnosticsHere,
  type MachineProfileRow,
  type SlowMoment,
} from "./bindings";
import { recentSlowLog } from "./slowLog";

/**
 * How many of the slow-answer log's entries the report carries, the newest: the core keeps up to
 * a few thousand, each kind to its own cap, more than a person reads in an issue or a chat takes
 * in one paste.
 */
export const SLOW_ENTRIES_COPIED = 200;

/**
 * **Copy diagnostics' report** (#4774): plain text a person pastes into an issue or a chat in
 * one step. The app's version and build, the system, each harness's installed version, the open
 * and hot chats with the device's hot-chat target and RAM class and, where it could be read,
 * the slow-answer and stall log's newest {@link SLOW_ENTRIES_COPIED} entries (#4725), one JSON
 * line each.
 *
 * **Nothing secret goes in.** Every line is built here from fields picked by name, never by
 * passing an answer through: a harness profile carries its folders and how it signs in, and
 * the report takes its harness's name and installed version alone. A refusal's words are left
 * out for the same reason, since they may name a path. The slow-answer log's entries are the
 * one thing copied as written: a {@link SlowMoment} holds a command's name, times, counts and
 * the name of the tab in front, and nothing else.
 */
export async function gatherDiagnostics(): Promise<string> {
  const [about, here, profiles, slow] = await Promise.all([
    commands.aboutCharter(),
    commands.diagnosticsHere(),
    commands.machineProfiles(),
    recentSlowLog(),
  ]);
  return diagnosticsReport({
    about,
    here,
    profiles: profiles.status === "ok" ? profiles.data : null,
    slow,
  });
}

/** The report's text, out of what was gathered. */
export function diagnosticsReport({
  about,
  here,
  profiles,
  slow,
}: {
  about: About;
  here: DiagnosticsHere;
  /** `null` where the profiles could not be read. */
  profiles: readonly MachineProfileRow[] | null;
  /** The slow-answer log's entries, newest last; `null` where it could not be read. */
  slow: readonly SlowMoment[] | null;
}): string {
  const lines = [
    "purlis diagnostics",
    `Version: ${about.version}${buildSaid(about)}, ${about.profile} profile`,
    `System: ${here.os}`,
    `Harnesses: ${harnessesSaid(profiles)}`,
    `Chats: ${here.open} open, ${here.hot} hot, ${targetSaid(here)}`,
  ];
  if (slow !== null) {
    if (slow.length === 0) lines.push("Slow answers and stalls: none recorded");
    else {
      const copied = slow.slice(-SLOW_ENTRIES_COPIED);
      const which =
        copied.length < slow.length ? ` the newest ${copied.length} of ${slow.length},` : "";
      lines.push(
        `Slow answers and stalls,${which} most recent last:`,
        ...copied.map((one) => `  ${JSON.stringify(one)}`),
      );
    }
  }
  return lines.join("\n");
}

/**
 * The hot-chat target, and the RAM class it comes from (ADR 0082 §3). A memory that could not be
 * read has no class and so no target, which is not the same as a device under 8 GB.
 */
function targetSaid({ device }: DiagnosticsHere): string {
  if (device.kind === "reading") return "class still being read";
  if (device.kind === "not_known") return "class not known";
  const class_ = device.class_gb === null ? "under 8 GB" : `${device.class_gb} GB class`;
  return `target ${device.hot_target} (${class_})`;
}

function buildSaid({ build }: About): string {
  if (build.kind === "dev") return `, a dev build of ${build.of}`;
  if (build.kind === "unlisted") return ", not in the changelog it carries";
  return "";
}

/** Each harness once, as people call it, with the version installed here. */
function harnessesSaid(profiles: readonly MachineProfileRow[] | null): string {
  if (profiles === null) return "could not be read";
  const installed = new Map<string, string | null>();
  for (const { harness, installed: version } of profiles)
    if (!installed.has(harness) || installed.get(harness) === null) installed.set(harness, version);
  if (installed.size === 0) return "none set up";
  return [...installed]
    .map(([harness, version]) =>
      version === null ? `${harness} (no version found)` : `${harness} ${version}`,
    )
    .join(", ");
}
