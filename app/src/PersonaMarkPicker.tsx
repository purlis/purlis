// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useCallback,
  useContext,
  useMemo,
  useState,
  useSyncExternalStore,
  type CSSProperties,
} from "react";
import * as RadioGroup from "@radix-ui/react-radio-group";
import { commands, type PersonaImage, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { OffWhenReadOnly } from "./ReadOnlyLine";
import {
  MARK_TOKENS,
  NOT_DRAWN,
  PERSONA_COLOURS,
  PERSONA_ICONS,
  PersonaMark,
  PersonaMarks,
  ReloadPersonaMarks,
  followTheme,
  initialsOf,
} from "./PersonaMark";
import { inForce, tintVariables } from "./theme/theme";
import { Form, TextField } from "./ui/field/Field";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **Pick a persona's icon and colour**, in its view. A pick is written to the persona's
 * definition at once (`icon:` and `color:`), so it is on every surface and every teammate's
 * machine after a save, and there is no second step to forget.
 *
 * Two radio groups (Radix, ADR 0037). The first choice of each takes the key out of the
 * definition: the persona goes back to its initials, or to the colour of its name. Where its
 * folder holds a custom image, that is what is drawn, and the icon picked here is what it
 * falls back to.
 *
 * Under the colours, one text box takes any other hue as `#rrggbb`, written at Enter; the
 * core checks it as it checks a line written by hand, and its refusal is said as a Notice.
 *
 * What the persona asked for and cannot have is said above the groups, each as a Notice.
 *
 * On a project purlis opens read-only (#826) the definition is a committed file, so the groups
 * and the box are off, with the project's sentence as their reason; the Notices stay.
 */
export function PersonaMarkPicker({ plane, persona }: { plane: PlaneId; persona: string }) {
  const mark = useContext(PersonaMarks).get(persona);
  const reload = useContext(ReloadPersonaMarks);
  const [refused, setRefused] = useState<string>();
  // Which image this window could not decode: a new one is tried, and may draw.
  const [undrawn, setUndrawn] = useState<PersonaImage | null>(null);
  const image = mark?.image ?? null;
  const notDrawn = image !== null && undrawn === image;
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  const imageTrouble = useCallback(() => setUndrawn(image), [image]);
  const trouble = useMemo(
    () => [
      ...(mark?.trouble ?? []),
      ...(notDrawn ? [NOT_DRAWN] : []),
      ...(refused ? [refused] : []),
    ],
    [mark, notDrawn, refused],
  );
  const pick = (icon: string | null, colour: string | null) => {
    setRefused(undefined);
    void commands
      .personaMarkSet(plane, persona, icon, colour)
      .then((said) => {
        if (said.status === "error") setRefused(said.error);
        else reload();
      })
      .catch((err: unknown) =>
        setRefused(`purlis could not write ${persona}'s icon and colour: ${String(err)}`),
      );
  };
  // A persona the plane does not have (any more) has no definition to write a pick into.
  if (mark === undefined) return null;
  const icon = mark.icon;
  const colour = mark.colour;
  const custom = colour !== null && !PERSONA_COLOURS.includes(colour);
  return (
    <section className="persona-mark-picker" aria-label="Icon and colour">
      <div className="persona-mark-shown">
        <PersonaMark persona={persona} onImageTrouble={imageTrouble} />
        {image !== null && !notDrawn && (
          <p>
            The custom image in its folder is drawn. The icon picked here is what it falls back to.
          </p>
        )}
      </div>
      {trouble
        .filter((why) => !dismissed.has(why))
        .map((why) => (
          <Notice
            key={why}
            cause={`persona-mark:${persona}`}
            tone="trouble"
            at="pane"
            onDismiss={() => setDismissed((was) => new Set(was).add(why))}
          >
            {why}
          </Notice>
        ))}
      <OffWhenReadOnly>
        <RadioGroup.Root
          className="persona-mark-choices"
          aria-label="Icon"
          value={icon ?? ""}
          onValueChange={(to) => pick(to === "" ? null : to, colour)}
        >
          <Tooltip tip="Initials">
            <RadioGroup.Item className="persona-mark-choice" value="" aria-label="Initials">
              {initialsOf(persona)}
            </RadioGroup.Item>
          </Tooltip>
          {Object.entries(PERSONA_ICONS).map(([name, Icon]) => (
            <Tooltip key={name} tip={name}>
              <RadioGroup.Item className="persona-mark-choice" value={name} aria-label={name}>
                <Icon aria-hidden="true" />
              </RadioGroup.Item>
            </Tooltip>
          ))}
        </RadioGroup.Root>
        <RadioGroup.Root
          className="persona-mark-choices"
          aria-label="Colour"
          value={custom ? "custom" : (colour ?? "")}
          onValueChange={(to) => {
            if (to !== "custom") pick(icon, to === "" ? null : to);
          }}
        >
          <Tooltip tip="From its name">
            <RadioGroup.Item
              className="persona-mark-choice persona-mark-swatch"
              value=""
              aria-label="From its name"
            >
              <PersonaMark persona={persona} mark={{ icon: null, colour: null, image: null }} />
            </RadioGroup.Item>
          </Tooltip>
          {PERSONA_COLOURS.map((name) => (
            <Tooltip key={name} tip={name}>
              <RadioGroup.Item
                className="persona-mark-choice persona-mark-swatch"
                value={name}
                aria-label={name}
              >
                <Swatch colour={name} />
              </RadioGroup.Item>
            </Tooltip>
          ))}
          {/* A `#rrggbb` written by hand in the definition: shown as what is picked, and kept
            until another is. The window offers the palette; the file takes any hue. */}
          {custom && colour !== null && (
            <Tooltip tip={colour}>
              <RadioGroup.Item
                className="persona-mark-choice persona-mark-swatch"
                value="custom"
                aria-label={colour}
              >
                <Swatch colour={colour} />
              </RadioGroup.Item>
            </Tooltip>
          )}
        </RadioGroup.Root>
        <HexColour
          key={custom ? colour : ""}
          held={custom ? colour : ""}
          onPick={(to) => pick(icon, to)}
        />
      </OffWhenReadOnly>
    </section>
  );
}

/** What a colour typed by hand must look like before it is sent: the core reads the same. */
const HEX = /^#[0-9a-fA-F]{6}$/;

/** What a colour typed by hand that is not one says, when the box is left or Enter is pressed. */
export const NOT_A_COLOUR = "A colour is # and six hex digits, such as #7c5cff.";

/**
 * Any colour, typed as `#rrggbb` and written at Enter. Something else is said under the box, as
 * the field family says a value it refuses (N26), and nothing is sent. `held` is a colour the
 * definition already has outside the palette.
 */
function HexColour({ held, onPick }: { held: string; onPick: (colour: string) => void }) {
  const [typed, setTyped] = useState(held);
  return (
    <Form
      className="persona-mark-hex"
      onSubmit={(event) => {
        event.preventDefault();
        onPick(typed.trim());
      }}
    >
      <TextField
        aria-label="Any colour, as #rrggbb"
        placeholder="#rrggbb"
        maxLength={7}
        columns={7}
        value={typed}
        validate={(value) => (HEX.test(value.trim()) ? undefined : NOT_A_COLOUR)}
        onChange={setTyped}
      />
    </Form>
  );
}

/** One colour of the picker, drawn as a mark's ground is. */
function Swatch({ colour }: { colour: string }) {
  const theme = useSyncExternalStore(followTheme, inForce);
  return (
    <span
      className="persona-mark persona-mark-blank"
      data-colour={colour}
      style={tintVariables(theme, colour, MARK_TOKENS) as CSSProperties}
      aria-hidden="true"
    />
  );
}
