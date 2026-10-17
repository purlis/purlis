### Fixed

- **A New tab opens faster: the picker compiles each harness's sandbox once, not once per
  profile** (#2179). With several profiles of one harness listed, the picker does one compile
  for them all, and still checks each profile's program for itself.
