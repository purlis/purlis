// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type About, type DiagnosticsHere, type MachineProfileRow } from "./bindings";
import { recentSlowLog } from "./slowLog";

/**
 * **Copy diagnostics' report** (#4774): plain text a person pastes into an issue or a chat in
 * one step. The app's version and build, the system, each harness's installed version, the open
 * and hot chats and, where this build keeps it, the slow-answer and stall log's most recent
 * entries.
 *
 * **Nothing secret goes in.** Every line is built here from fields picked by name, never by
 * passing an answer through: a harness profile carries its folders and how it signs in, and
 * the report takes its harness's name and installed version alone. A refusal's words are left
 * out for the same reason, since they may name a path.
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
  slow: readonly string[] | null;
}): string {
  const lines = [
    "purlis diagnostics",
    `Version: ${about.version}${buildSaid(about)}, ${about.profile} profile`,
    `System: ${here.os}`,
    `Harnesses: ${harnessesSaid(profiles)}`,
    `Chats: ${here.open} open, ${here.hot} hot`,
  ];
  if (slow !== null) {
    if (slow.length === 0) lines.push("Slow answers and stalls: none recorded");
    else lines.push("Slow answers and stalls, most recent last:", ...slow.map((one) => `  ${one}`));
  }
  return lines.join("\n");
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
