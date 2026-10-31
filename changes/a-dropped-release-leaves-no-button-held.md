### Fixed

- **A mouse button you let go of is let go in the browser too.** When the page moved on between
  your press and your release, the release was dropped and the engine kept the button held, so
  the next click could read as a drag. purlis now sends the release at the point of your press,
  and handing a browser back lets go of any button still held (#2989).
