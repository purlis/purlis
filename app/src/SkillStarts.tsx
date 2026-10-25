// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  FilePlus,
  FolderGit2,
  LoaderCircle,
  MessageSquarePlus,
  Sparkles,
  type LucideIcon,
} from "lucide-react";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";

/** One way to start a skill (decision 42): a chat's draft, a skill source, the blank template. */
export type SkillStart = "draft" | "sources" | "blank";

/** New skill's words, on the studio's toolbar, its starts and the Skills view alike. */
export const NEW_SKILL = "New skill";

/** **The Skills view's New skill** (#3266): the three starts as a menu, so its words are the
 *  studio's own and an ellipsis, and no button in the window is named twice. */
export const NEW_SKILL_MENU = `${NEW_SKILL}…`;

/** The words of the act that opens a chat to draft a skill, wherever it is offered. */
export const DRAFT_WITH_A_CHAT = "Ask a chat to draft a skill";

/**
 * **The three starts, in their order** (SK-22, #2785; decision 42), with each one's words, its
 * line and its mark: the studio's empty state draws them as buttons and the Skills view's
 * New skill as a menu (#3266), so a start is said and ordered in one place.
 */
export const SKILL_STARTS: readonly {
  start: SkillStart;
  label: string;
  about: string;
  mark: LucideIcon;
}[] = [
  {
    start: "draft",
    label: DRAFT_WITH_A_CHAT,
    about: "Say what you want in a chat. Its draft comes back here to try and accept.",
    mark: MessageSquarePlus,
  },
  {
    start: "sources",
    label: "Browse sources",
    about: "Install a reviewed copy of a skill from a git repository.",
    mark: FolderGit2,
  },
  {
    start: "blank",
    label: NEW_SKILL,
    about: "Write one yourself, from a blank template.",
    mark: FilePlus,
  },
];

/**
 * **What a project with no skill of its own is offered** (SK-22, decision 42): the claim that it
 * has none, that its chats still get the skills listed, and the three starts. The Skill studio
 * draws the starts as buttons; the Skills view (#3266), whose New skill… holds them as a menu,
 * names that instead (`from`), so each start is one button in the window, never two of a name.
 */
export function SkillStarts({
  busy,
  onStart,
  from,
  testid = "skill-studio-empty",
}: {
  busy: boolean;
  onStart: (start: SkillStart) => void;
  /** Where the starts are instead of here, by the words of what holds them. */
  from?: string;
  testid?: string;
}) {
  return (
    <EmptyState
      headline="This project has no skills of its own yet"
      body={
        from === undefined
          ? "Every chat still gets the skills listed here. Start one of the project's own:"
          : `Every chat still gets the skills listed here. Start one of the project's own from ${from}`
      }
      mark={Sparkles}
      testid={testid}
      action={
        from !== undefined ? undefined : (
          <ul className="skill-starts" aria-label="Ways to start a skill">
            {SKILL_STARTS.map(({ start, label, about, mark: Mark }) => (
              <li key={start}>
                <Button
                  icon={<Mark aria-hidden="true" />}
                  isDisabled={busy}
                  onPress={() => onStart(start)}
                >
                  {label}
                </Button>
                <p className="honest">{about}</p>
              </li>
            ))}
          </ul>
        )
      }
    />
  );
}

/** **The project's skills could not be read**: the core's sentence, under the claim. The Skill
 *  studio and the Skills view say it alike. */
export function SkillsNotRead({ refused, testid }: { refused: string; testid: string }) {
  return (
    <EmptyState
      headline="purlis could not read this project's skills"
      body={refused}
      testid={testid}
    />
  );
}

/** **The project's skills, being read**: what the Skill studio and the Skills view say meanwhile. */
export function SkillsReading() {
  return (
    <p className="pending" aria-busy="true">
      <LoaderCircle className="node-icon spinning" />
      Reading the project&apos;s skills…
    </p>
  );
}
