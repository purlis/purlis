### Added

- **A handoff is announced where you'll see it.** When another chat hands work to a workspace, a
  card hangs from that workspace's chip, "<chat> arrived from <asking chat>", with Go to it. The
  card never takes the keyboard, and hiding it leaves a dot on the chip. With no chip for that
  workspace, it's listed in the Notices centre. The Inbox gets an Update, which the ✋ doesn't
  count, and the chat's sidebar row says "new" until you've looked at it (#4759).
- **A task that can't report asks is announced the same way.** A task dispatched on a harness
  purlis has no hooks into can't say when it needs you, so it gets the same card and Update. Its
  row keeps the faint hand, and the chat that asked for it is told you must look at it yourself
  (#4759).
