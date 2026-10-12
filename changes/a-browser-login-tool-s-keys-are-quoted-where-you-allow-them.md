### Security

- **A browser-login tool's keys are quoted where you allow them.** The line an Allow of a
  persona's tools is bound to now quotes a browser login's user and password keys, so two
  different logins never read as the same line. A persona with a browser-login tool asks for
  its Allow once more after this update; a persona whose tools are all programs keeps its
  Allow (#1828).
