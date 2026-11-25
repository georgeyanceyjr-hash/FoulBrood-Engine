# Overnight plan, 24-25 Nov 2026 (user approved: install what passes, keep testing until 6 am Denver)

Queue on the Mac, in order: ZW-q14hg (nets with strong-player games), ZX-speed (v214, faster net evaluation),
ZY-loop12 (self-play at depth 6, net q15).

## What counts as passing

- **A new net** (q14a, q14b, q15): first match at least 55 of 100 (or 110 of 200) against the installed net, then a
  confirmation match; install only if the combined score over at least 300 games is 55 % or better.
- **A faster build with identical evaluations** (v214): at least 45 of 100 against the build it replaces, and no new
  files in results/err.
- Anything else is reported, not installed.

## Installing

Copy the passed binary and net into FoulBroodData/Engines/ThoraxRex (keeps its book) and
FoulBroodData/Engines/ThoraxRex-NoBook, with no bot game in progress. Keep the previous version in h2h/. Record it in
this repository (h2h/, README) and push.
