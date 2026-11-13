// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  type NoSandboxAtSetup,
  type ProtectionWord,
  type SetupProtection,
} from "./bindings";
import { Choice, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { LEVEL_SAID } from "./settings/yourLevel";

/** Where setup starts: Basic sandbox (Q115), unless this machine's policy says otherwise. */
export const PROTECTION_PRESET: ProtectionWord = "floor";

/** The options in the row's order: the recommended one first (R-061). */
const ORDER: readonly ProtectionWord[] = ["floor", "on", "off"];

/** How strict each level is, strictest first, as the core orders them. */
const STRICTNESS: Readonly<Record<ProtectionWord, number>> = { on: 0, floor: 1, off: 2 };

/** One line of help per option (R-062), Full sandbox's with R-295's sentence (Q159). */
export const PROTECTION_HELP: Readonly<Record<ProtectionWord, string>> = {
  floor: "Chats can't read your vaults or change what later programs run. They reach any host.",
  on: "Chats also write only inside the project. New hosts are asked about the first time a chat reaches them.",
  off: "Chats run as you, with nothing between them and your files.",
};

/** Whether `setup`'s bound lets the row offer `word`: every level until the core has said. */
function offered(setup: SetupProtection | undefined, word: ProtectionWord): boolean {
  return setup === undefined || STRICTNESS[word] <= STRICTNESS[setup.lowest];
}

/**
 * **What the row is told by the core** (Q241): the levels this machine lets a new project take,
 * the one to start at, and the local project's own level where it is there already. Undefined
 * until it answers, or where it could not, which leaves the row as it is: the core still checks
 * the policy before it writes anything.
 */
export function useSetupProtection(): SetupProtection | undefined {
  const [setup, setSetup] = useState<SetupProtection>();
  useEffect(() => {
    let gone = false;
    void commands
      .setupProtection()
      .then((answer) => {
        if (!gone && answer.status === "ok" && answer.data) setSetup(answer.data);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, []);
  return setup;
}

/**
 * **What No sandbox would do here, from the core** (T1-25; R-066, R-298): the vaults that move
 * into the keychain in the page's one press, the warning's key, and a team's vault that refuses
 * it. `folder` is where the project is made; null is this machine's local project. Undefined
 * until it answers, or where it could not: the core still refuses and moves when it writes.
 */
export function useNoSandbox(folder: string | null | undefined): NoSandboxAtSetup | undefined {
  const [said, setSaid] = useState<{ folder: string | null; report: NoSandboxAtSetup }>();
  useEffect(() => {
    if (folder === undefined) return;
    let gone = false;
    void commands
      .noSandboxAtSetup(folder)
      .then((answer) => {
        if (!gone && answer.status === "ok" && answer.data)
          setSaid({ folder, report: answer.data });
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [folder]);
  return said !== undefined && said.folder === folder ? said.report : undefined;
}

/** The warnings the core names by key (Q62, Q141): what every No sandbox chat is. */
const WARNINGS: Readonly<Record<string, string>> = {
  "setup.no-sandbox.runs-as-you":
    "A chat runs as you: it can change these settings and the local record of what it did. purlis guards then catch accidents only.",
};

/** A count and its noun, agreeing. */
const vaults = (count: number) => (count === 1 ? "1 vault" : `${count} vaults`);

/** The names in a list, joined as a sentence says them. */
function listed(names: readonly string[]): string {
  if (names.length < 2) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** What the row says of the move (R-066). */
export function moveSaid(moves: readonly string[]): string {
  const plain = moves.length === 1 ? "plain-file vault" : "plain-file vaults";
  return `With your press, purlis first moves ${moves.length} ${plain}, ${listed(moves)}, into the system keychain. Each secret is written and read back before its file goes.`;
}

/**
 * **The page's main act at No sandbox** (R-293): its words with the move named and counted
 * ("Create, and move 2 vaults to the keychain"), where there is one; else its own words.
 */
export function withTheMove(
  label: string,
  protection: ProtectionWord,
  noSandbox: NoSandboxAtSetup | undefined,
): string {
  const moves = noSandbox?.moves.length ?? 0;
  if (protection !== "off" || moves === 0 || noSandbox?.refused) return label;
  return `${label}, and move ${vaults(moves)} to the keychain`;
}

/** Whether the core refuses the row's choice (R-298): the page's act waits for another. */
export function refusedHere(
  protection: ProtectionWord,
  noSandbox: NoSandboxAtSetup | undefined,
): boolean {
  return protection === "off" && Boolean(noSandbox?.refused);
}

/** The row's answer: the person's pick, else the bound's default, else Basic sandbox. */
export function useProtection(
  setup: SetupProtection | undefined,
): [ProtectionWord, (to: ProtectionWord) => void] {
  const [picked, setPicked] = useState<ProtectionWord>();
  const chosen = picked ?? setup?.default ?? PROTECTION_PRESET;
  return [offered(setup, chosen) ? chosen : (setup?.default ?? PROTECTION_PRESET), setPicked];
}

/**
 * **The Protection row** of First run and New project (T1-24; Q115, Q159): Basic sandbox
 * preselected, a line of help under each option, and "Change any time in Settings". The answer
 * goes with the page's own main button, so setup still takes one press at every choice, and the
 * core keeps it as the person's own level for a project it made, the value Settings › Project ›
 * Sandbox › Your level reads (Q236). Options below what this machine's policy requires are out
 * of reach, with its reason and never "(recommended)", and the row starts at the policy's level
 * (Q241). No sandbox's own lines are T1-25's.
 */
export function ProtectionRow({
  value,
  onChange,
  setup,
  noSandbox,
  disabled,
}: {
  value: ProtectionWord;
  onChange: (to: ProtectionWord) => void;
  /** The core's bound ({@link useSetupProtection}). */
  setup?: SetupProtection;
  /** What No sandbox would do here ({@link useNoSandbox}). */
  noSandbox?: NoSandboxAtSetup;
  /** Held while the page's act is being done. */
  disabled?: boolean;
}) {
  const why = setup?.why;
  return (
    <>
      <SettingRow
        label="Protection"
        setting="setup-protection"
        help={why ? `${why} Change any time in Settings.` : "Change any time in Settings."}
        control={(ids) => (
          <Choice
            ids={ids}
            kind="radio"
            value={value}
            onValueChange={(to) => onChange(to as ProtectionWord)}
            disabled={disabled}
            options={ORDER.map((one) => ({
              value: one,
              label:
                one === PROTECTION_PRESET && offered(setup, one)
                  ? `${LEVEL_SAID[one]} (recommended)`
                  : LEVEL_SAID[one],
              says: PROTECTION_HELP[one],
              disabled: !offered(setup, one),
            }))}
          />
        )}
      />
      {value === "off" && (
        <NoSandboxLines
          noSandbox={noSandbox}
          onBasic={() => onChange("floor")}
          disabled={disabled}
        />
      )}
    </>
  );
}

/**
 * **No sandbox's own lines, in the row** (T1-25; R-066, R-298; Q156, Q163): the warning every No
 * sandbox chat carries and the vault move the page's one press makes, or, where the team's
 * committed file registers a plain-file vault, the core's refusal naming its owner and Use Basic
 * sandbox. Never a second dialog.
 */
function NoSandboxLines({
  noSandbox,
  onBasic,
  disabled,
}: {
  noSandbox?: NoSandboxAtSetup;
  onBasic: () => void;
  disabled?: boolean;
}) {
  const refused = noSandbox?.refused;
  if (refused)
    return (
      <div className="no-sandbox-lines" role="note">
        <p>{refused.said}</p>
        <Button isDisabled={disabled} onPress={onBasic}>
          Use Basic sandbox
        </Button>
      </div>
    );
  const warning = WARNINGS[noSandbox?.warning ?? "setup.no-sandbox.runs-as-you"];
  return (
    <div className="no-sandbox-lines" role="note">
      {warning && <p>{warning}</p>}
      {noSandbox && noSandbox.moves.length > 0 && <p>{moveSaid(noSandbox.moves)}</p>}
    </div>
  );
}

/**
 * **The local project's level, read-only** (Q241): where Open repo goes into the project this
 * machine has already, setup writes no level, and the page says the one it keeps.
 */
export function YourLevel({ level }: { level: ProtectionWord }) {
  return <p className="your-level">{`Your level: ${LEVEL_SAID[level]} · change it in Settings`}</p>;
}
