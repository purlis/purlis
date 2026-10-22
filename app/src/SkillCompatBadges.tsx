// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { SkillCompat, SkillFit } from "./bindings";

/** How well a skill fits, as its badge's word says it. */
export function fitSaid(fit: SkillFit): string {
  switch (fit) {
    case "full":
      return "full";
    case "partial":
      return "partial";
    case "widens":
      return "widens the chat";
    case "refused":
      return "handed to no chat";
  }
}

/**
 * **A skill's compatibility badges** (SK-8, #2771; spec decision 18): one per harness that would
 * load it, saying how well it fits (full, partial, widens the chat, or handed to no chat) and,
 * opened, the core's
 * lines that say why: what applies, what purlis leaves out because a skill may only narrow a
 * chat, what is carried over by purlis's equivalent, and what is not applied. Nothing is
 * silently half-applied.
 */
export function SkillCompatBadges({
  name,
  compat,
  lint = [],
}: {
  name: string;
  compat: SkillCompat[];
  /** What the built-in lints say of a skill from outside the project, one line each. */
  lint?: readonly string[];
}) {
  if (compat.length === 0 && lint.length === 0) return null;
  const widens = compat.some((one) => one.fit === "widens");
  return (
    <ul className="skill-compat" aria-label={`Where ${name} works`}>
      {widens && (
        <li className="skill-compat-widens" data-testid={`widens-${name}`}>
          Widens the chat
        </li>
      )}
      {compat.map((one) => (
        <li key={one.harness}>
          <details
            className="skill-compat-badge"
            data-fit={one.fit}
            data-testid={`compat-${name}-${one.harness}`}
          >
            <summary>{`${one.title}: ${fitSaid(one.fit)}`}</summary>
            <ul>
              {one.says.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
          </details>
        </li>
      ))}
      {lint.map((line) => (
        <li key={line} className="honest">
          {line}
        </li>
      ))}
    </ul>
  );
}
