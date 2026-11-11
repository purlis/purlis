// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The code family** (#4481, spec #4469, ADR 0104): text to read and copy, a command or YAML, in
 * a box with its Copy button. A screen imports {@link CodeBlock} and never writes
 * `data-scope="code"` (`purlis/code-family`).
 *
 * The text can be reached by the keyboard, so a person who needs to scroll it can. Copy says it
 * copied, or that it couldn't, and to a reader as well.
 *
 * **Drawn by its parts** (ADR 0103): the `root`, its `head` with the `label`, and the `body`
 * holding the text. `code.css` draws the family default and a theme's parts reach the same
 * attributes.
 */
import { useEffect, useId, useState } from "react";
import { Check, Copy } from "lucide-react";
import { Button } from "../button/Button";
import { Reserved } from "../dialog/Reserved";

/** How long Copy says it copied. */
const COPIED_MS = 1800;

function CopyButton({ text, what }: { text: string; what: string }) {
  const [said, setSaid] = useState<"Copied" | "Couldn't copy">();
  useEffect(() => {
    if (said === undefined) return;
    const later = setTimeout(() => setSaid(undefined), COPIED_MS);
    return () => clearTimeout(later);
  }, [said]);
  const copy = () => {
    const put = navigator.clipboard?.writeText(text);
    if (!put) return setSaid("Couldn't copy");
    void put.then(
      () => setSaid("Copied"),
      () => setSaid("Couldn't copy"),
    );
  };
  return (
    <>
      <Button
        size="sm"
        variant="ghost"
        icon={said === "Copied" ? <Check aria-hidden size={14} /> : <Copy aria-hidden size={14} />}
        // Named by the word it shows, Copied too, so its name holds what is seen (WCAG 2.5.3).
        aria-label={`${said ?? what}: ${text.split("\n")[0]}`}
        onPress={copy}
      >
        <Reserved texts={[what, "Copied", "Couldn't copy"]}>{said ?? what}</Reserved>
      </Button>
      <span role="status" data-scope="code" data-part="said">
        {said}
      </span>
    </>
  );
}

/** **Text to copy**: `label` is its heading and what a reader hears it by. */
export function CodeBlock({
  text,
  label,
  copy = "Copy",
  className,
}: {
  text: string;
  label: string;
  /** The Copy button's word. */
  copy?: string;
  /** A place for it in its view: a margin, a width. Never a look. */
  className?: string;
}) {
  const id = useId();
  return (
    <figure aria-labelledby={id} className={className} data-scope="code" data-part="root">
      <figcaption data-scope="code" data-part="head">
        <span id={id} data-scope="code" data-part="label">
          {label}
        </span>
        <CopyButton text={text} what={copy} />
      </figcaption>
      <pre tabIndex={0} aria-labelledby={id} data-scope="code" data-part="body">
        <code>{text}</code>
      </pre>
    </figure>
  );
}
