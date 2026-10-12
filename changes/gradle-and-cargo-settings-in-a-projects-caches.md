### Changed

- **Gradle and cargo keep your settings in a project's own caches.** A sandboxed chat's Gradle
  now gets the settings from your `gradle.properties` that say how it builds and caches, and an
  init script that sends its downloads through the chat's proxy, which Java does not find on
  its own. cargo keeps your certificate file, revocation check, `git-fetch-with-cli` and
  `offline`. Your proxy, JVM options, project properties and init scripts are not copied, and
  the written files say what was left out (#1364).
