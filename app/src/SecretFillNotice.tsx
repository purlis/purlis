// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useState } from "react";
import type { FillShown, PlaneId, Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { AlwaysLine } from "./LoginSite";
import { Notice, NoticeOf, type NoticeAction } from "./Notice";

/**
 * **Where a secret a chat asks for would go** (#5115, the person's rulings SF-1, SF-2): the key
 * and its vault by name, never a value, then the exact site large, never cut short, since the
 * part a look-alike hides in is the part that would go. The page and the field's label are the
 * page's own words, said as such. A card's key says it is allowed once each time (SF-4), and
 * the line Always allow writes is shown before it's pressed (#5116). A multi-line box or an
 * editable area says text in one is often posted or published, since it asks each time (#5120).
 * A 2FA code says it is one, made from the seed under the key
 * (#5118, SF-4): the seed is what the person approves for the site, and never goes there.
 */
export function SecretFillSite({ fill }: { fill: FillShown }) {
  // The label is the page's own words: set apart, so they never read as purlis's.
  const field =
    fill.label.trim() === "" ? (
      "a field"
    ) : (
      <>
        the field the page labels <code>{fill.label}</code>
      </>
    );
  return (
    <div className="login-site" data-testid="fill-site">
      <p className="ui-text-meta">
        {fill.code ? (
          <>
            Put a 2FA code from <code>{fill.key}</code> in vault <code>{fill.vault}</code> into{" "}
            {field} on
          </>
        ) : (
          <>
            Put <code>{fill.key}</code> from vault <code>{fill.vault}</code> into {field} on
          </>
        )}
      </p>
      <p className="login-site-domain ui-text-title" data-testid="fill-site-site">
        {fill.site}
      </p>
      {fill.page != null && (
        <p className="ui-text-meta">
          In browser {fill.browser}, on the page{" "}
          <span className="login-site-line">{fill.page}</span>
        </p>
      )}
      <p className="ui-text-meta">
        {fill.code
          ? "purlis works the code out from the seed as it fills, and the page and the chat never see the seed."
          : "purlis fills it there and nowhere else, and the chat never sees the value."}
      </p>
      {fill.warnings.map((warning) => (
        <p key={warning} className="login-site-warning">
          {warning}
        </p>
      ))}
      {fill.multi_line && (
        <p className="login-site-warning">
          This is a multi-line box or an editable area. Text in one is often posted or published, so
          you are asked each time.
        </p>
      )}
      {fill.multi_line ? null : fill.card ? (
        <p className="ui-text-meta">
          This is a card or payment value, so it is allowed once each time.
        </p>
      ) : fill.no_always != null ? (
        <p className="ui-text-meta">{fill.no_always}.</p>
      ) : (
        fill.line != null && (
          <AlwaysLine
            lead="Always allow adds this to "
            under={fill.under}
            vaultToo={fill.vault_too}
            line={fill.line}
          />
        )
      )}
    </div>
  );
}

/**
 * **A secret a chat asks purlis to fill, on its pane** (#5115): the sandbox's own Notice and
 * answers, held until it settles as every Notice that grants is. Its call waits for the press,
 * and only a press fills anything.
 */
function OneFill({
  plane,
  ask,
  onWay,
}: {
  plane: PlaneId;
  ask: Shown;
  onWay?: (answer: string, ask: Shown) => void;
}) {
  const offScreen = useContext(NoticeOf) !== null;
  const [refused, setRefused] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [first, ...rest] = ask.options.map((option): NoticeAction => ({
    label: option.label,
    busy,
    allows: option.allows,
    onPress: () => {
      if (busy) return;
      setBusy(true);
      setRefused(undefined);
      void answerTaken(plane, ask, option.id)
        .then((answer) => {
          if (answer.refused === undefined) onWay?.(option.label, ask);
          else setRefused(answer.refused);
        })
        .finally(() => {
          setBusy(false);
          asksMoved(plane);
        });
    },
  }));
  if (first === undefined || ask.fill == null) return null;
  return (
    <Notice
      // Drawn anew, it's a new Notice to settle and read.
      cause={`secret-fill:${ask.ask}:${ask.answer.via === "secret-fill" ? ask.answer.shown : ""}`}
      at="pane"
      tone={refused === undefined ? "news" : "trouble"}
      label="Secret fill"
      fixes={[first, ...rest]}
    >
      {offScreen ? "It asks" : "This chat asks"} to put a secret from a vault into its browser, and
      waits for your answer.
      <SecretFillSite fill={ask.fill} />
      {refused !== undefined && ` ${refused}`}
    </Notice>
  );
}

/** Every secret fill this chat's pane draws. */
export function SecretFillNotice({
  plane,
  session,
  asks,
  onWay,
}: {
  plane: PlaneId;
  session: number;
  asks: readonly Shown[];
  onWay?: (answer: string, ask: Shown) => void;
}) {
  return (
    <>
      {asks
        .filter((ask) => ask.session === session && ask.answer.via === "secret-fill")
        .map((ask) => (
          <OneFill key={ask.ask} plane={plane} ask={ask} onWay={onWay} />
        ))}
    </>
  );
}
