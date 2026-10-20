### Security

- **A login purlis performs runs its page scripts in purlis's own world.** The scripts that find,
  focus, check and clear a login's fields run apart from the page's own scripts, as every other
  script purlis runs on a page does, so a page that redefines them cannot steer which field gets
  a value. A login whose world is gone fills nothing more and says so (#2613).
