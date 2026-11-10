# Amended 2026-11-09: artifact templates, and protocol 8 (ADR 0122 §9, #5973)

The vocabulary gains one word:

| Word        | What purlis does                                                                                                                                         | Its shape, under `contributes`                    | Change |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- | ------ |
| `artifacts` | offers the extension's page templates to a workspace; a person's pin copies one into the workspace's `artifacts/` and pins it as any page is (protocol 8) | `artifacts`: `name`, `title`, `folder`, `icon`? | #5973  |

**The shape.** At most 8 templates, the most artifact icons the bar draws. `name` is one id
segment, the template's id after `<extension-id>.`. `title` is a label purlis draws. `folder` is
a relative path inside the extension holding the template's `index.html`, held to the rule a
theme's `file` is held to, so the extension's fingerprint covers every byte of every template and
a changed template asks the operator again. `icon` is one of purlis's icons, else the pin's
default. A template needs no program. The approval prompt names each template, its title and its
folder.

**Protocol 8 holds no new message.** purlis reads a template from the extension's folder and
asks its program nothing about it, so no request or answer changes; 8 is the version a manifest
names to declare `artifacts`, and a manifest naming an earlier `version` is refused. An extension
that declares it is asked every other question in protocol 7's words.

**A pin copies, and then it is the workspace's page.** Only an extension installed here,
approved, unchanged since, and on in the project and workspace offers a template. Pinning one
(`purlis artifact pin --template <extension-id>.<name>`) reads its folder within the size cap,
refuses a folder that is a link out of the extension or holds something shaped like a secret,
and copies its files, never its data files, into `artifacts/<extension-id>.<name>/`, never over a
folder already there. It is then pinned through the one pin path, under every refusal a pin
meets (#5950, #5954). From then on the extension's approval says nothing about it: **the copy's
script is approved by each person on their own machine, over the copy's fingerprint**
(ADR 0122 §4), and a teammate without the extension sees the page with `needs-extension` where
its queries' data would be. A template carries no producing command, so it has no Regenerate; its
live data comes from queries.

Copying rather than pinning the extension's folder in place keeps ADR 0122 §1's rule that an
artifact is a folder in the workspace's `artifacts/`, shared with the workspace: a page served
from a machine-local extension folder would be a pin no teammate could open.

**On Windows, where purlis starts no extension's program** (ADR 0031), no extension offers a
template and a pin from one is refused with a sentence, never degraded.

It is proven end to end by the extension probe as a third manifest on the same folder,
`artifact-probe` (`crates/extension-probe/purlis-extension.artifacts.json`), in protocol 8, for
the reason `query-probe` is a manifest of its own.
