# Amendment, 2026-11-09: a pin to workspace is a team act, not an arrangement

**Amends [ADR 0040](../0040-a-pin-is-an-arrangement-so-it-is-machine-state.md)**, by
[ADR 0120](../0120-an-artifact-is-a-workspaces-page-rendered-isolated-and-its-api-is-the-read-only-tool-catalogue.md) §1 and §2, accepted the same day (#5948, #5949).

This record holds that a pin is an arrangement, and so machine state. **Pin to workspace** is not
one. It makes an HTML page an artifact of the workspace for everyone in the plane: it is confirmed
as "visible to everyone in <plane>", written to `workspaces/<ws>/artifacts/pins.toml`, and saved
with the plane like any other change. Changing a pin's folder or command makes a new pin, which no
machine trusts until a person confirms it there.

The unqualified **Pin**, of a tab, a workspace or a chat, is unchanged and stays machine state.
An artifact's open tab is a tab like any other, and pinning that tab is the person's own.
