# Amended 2026-10-18: three claims about denial sources, checked (#1907)

The denial-sources discussion rested on three claims nobody had checked against a primary
source. They were checked on 2026-10-18 (verdicts and sources:
https://github.com/purlis/purlis/issues/1907#issuecomment-6246438543). Nothing built changes.

- **Other processes' macOS log entries, in-process, need an entitlement: confirmed.**
  `OSLogStore.local()` needs both an administrator's account and the
  `com.apple.logging.local-store` entitlement. The ruling "no new entitlement" stands, so
  `log stream` on an administrator's account stays Proposal A's only route, and it still waits
  on the operator.
- **Landlock denials reach the Linux audit log: confirmed, from Linux 6.15 (Landlock ABI 7).**
  Reading them needs `CAP_AUDIT_READ`. purlis's wrap needs ABI 6 (Linux 6.12, ADR 0067 amended
  2026-10-16), so on 6.12–6.14 there is no audit record at all. A Linux denial source must not
  count on the audit log; the wrap's supervisor's own refusals stay the record.
- **A seccomp listener that reports a call without answering it: false.** A listener must
  answer every call it receives (a closed listener makes the kernel answer `ENOSYS`), a call the
  filter refuses itself never reaches one, and a process's filter chain takes one listener only.
  The built wrap is unaffected, since its supervisor is the listener. What this rules out is
  purlis watching a sandbox it does not own, such as a Claude Code chat's own Linux sandbox: the
  Linux side of Proposal B would need the audit log (privileged) or Claude Code's own report
  (Proposal C). Proposals A and B as written are macOS-only and unchanged.
