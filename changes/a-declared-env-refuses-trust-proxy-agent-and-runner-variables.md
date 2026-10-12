### Security

- **A harness declaration's `env` namespace refuses more.** Beside the variables that change
  what a process loads or runs, a namespace that reaches a TLS-trust, proxy, SSH-agent or
  program-runner variable (a pager, an editor, a browser, a compiler or its wrapper) is now
  refused, with one sentence that says so. A harness's own look-alikes, such as
  `AIDER_PROXY_MODE`, still pass (#1013).
