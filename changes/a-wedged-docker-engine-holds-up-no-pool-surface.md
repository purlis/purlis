### Fixed

- **A Docker engine that stops answering holds up no pool surface.** Every `docker` run that only
  reads (an inspect, a version, a list, the check before each link to a runner) is ended after 20
  seconds. The Pools settings, the drain and a link to a runner then say Docker did not answer.
  A pull or a make still takes as long as it takes (#3546).
