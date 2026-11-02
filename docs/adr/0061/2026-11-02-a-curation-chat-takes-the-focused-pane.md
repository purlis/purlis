# Amended (2026-11-02): a curation chat takes the focused pane (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D1 and D4, ruled by
the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

A curation action is a chat the person starts with a press, so it **takes the focused pane**, as
New chat does; it opens no tab. Its **name** says `<label> · <subject>`, where this record said its
tab did. It is filed under the subject workspace when it runs in that workspace's directory, and
at the plane root otherwise, and it is listed in the sidebar there. The Curate ▸ rows on a
workspace's, a persona's and the plane root's menus are not a chat's tab menu, and they stay.
