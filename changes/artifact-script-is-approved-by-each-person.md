### Added

- **A pinned artifact with script runs it only once you approve it, on your machine.** A page
  with no script opens as before. One with script (a `<script>` that runs, an `onclick=`, a
  `javascript:` link) is shown without it, and its tab's bar says so and offers Approve script;
  the frame's edge shows whether its script is asked about or running. Your approval covers the
  whole template folder but its data, so a page regenerated with new data never asks again,
  while any other change to the folder, or a change to the pin's folder or command, shows it
  without its script and asks again, naming who changed the pin. Approvals are yours alone,
  kept on this machine; every window hears one at once. `purlis artifact approve-status` says
  where each pin stands. On Windows, scripted artifacts render without their script for now.
