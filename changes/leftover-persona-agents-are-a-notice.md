### Changed

- **Leftover persona sub-agent files are a Notice with Fix, not a line in every briefing.** When
  the project still holds sub-agent files purlis generated for a persona, the Inbox shows one
  Notice whose Fix runs `purlis doctor --fix persona-agents`, and it goes once the files are
  gone. A chat's briefing no longer mentions them (#1990).
