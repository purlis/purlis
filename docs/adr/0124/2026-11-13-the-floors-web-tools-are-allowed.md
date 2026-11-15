# Amendment, 2026-11-13: at Basic sandbox, Claude Code's web tools are allowed (Q254)

The harness matrix's first row, "Sandbox: files and egress", said of Claude Code "web tools
denied" for both levels the table covers. That is the Full sandbox's claim only. Q254, from
T1-10's review, corrects the cell: **at Basic sandbox (the floor), Claude Code's web tools are
allowed, and the chat's proxy carries every host.** The floor still holds every denial class, for
Bash by Claude Code's own sandbox and for its file tools by deny rules, as the Full sandbox does.
This was a wrong cell, not a missing test.

The row is confirmed by its tests in `crates/purlis-core/src/sandbox/enforced_tests.rs`: one
for the Full sandbox, and a floor pin
(`at_basic_sandbox_claude_codes_denial_classes_are_held_and_its_web_tools_allowed`), each naming
the mutations it turns red.

The same review narrowed what the Full sandbox's test claims for Claude Code's file tools: a deny
rule holds a write where a denial class holds the path (the vaults, among them). A file tool's
write elsewhere outside the chat's folders is Claude Code's own permission check's, not a deny
rule purlis writes, and the cell says so.
