### Changed

- **A chat's pane draws its asks from the Inbox's own list.** A sandbox block, a folder write, a
  dispatch grant and a prompt held off screen are drawn on the chat's pane while the Inbox lists
  them, at most two per chat, the longest waiting first, so an answer in either place clears
  both. Their answers say the same words in both places (#1695, #1700).
- **The Inbox lists what waits longest first,** by when each ask began, where its source knows
  it (#1700).
- **A report with nowhere to go and a refused commit are updates in the Inbox,** not asks: they
  raise no notification, and Dismiss puts the chat's needs-you item away with them (#1694).
- **A folder write is asked as a folder write,** in the Inbox and its notification, apart from a
  host (#1700).

### Fixed

- **A connection whose hold ran out says so.** A sandbox block whose connection purlis held for
  your answer, and gave up on after a minute, now says that nobody answered in time and that an
  Allow tells the chat to run it again (#1709).
- **What an ask names is drawn safely.** A folder's or a chat's name in an ask is drawn with
  every character that draws as nothing written out, in the Inbox and on the pane (#1688).
- **An Inbox Notice that grants something waits to settle,** as an ask's Allow does: a press on
  it just after it was drawn or moved does nothing, and says why (#1695).
