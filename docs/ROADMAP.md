# FoulBrood engine roadmap

## After the FoulBrood II release: pondering (thinking on the opponent's time)

Agreed 27 Nov 2026.

- The bot keeps one engine process running for the whole game (today hive-hydra starts a fresh engine for
  every move), so the engine can keep searching while the opponent thinks and reuse that work.
- The Online page's "Engine insight on the opponent" then shows the **bot's own pondering thoughts**
  (rating, depth, top move) instead of starting a second search. Two searches would split the computer and
  make the pondering shallower.
- When the bot is not pondering (playing by hand, or an engine that cannot ponder), the setting works as
  today: a separate search in the viewer.
- Any engine must still work: engines without pondering simply play as now.

## Names and versions (27 Nov 2026)

- The first published engine is **FoulBrood Engine v1.0** (Cargo version 1.0.0). With an opening book it shows as
  "FoulBrood Engine v1.0 Book". Builds before it are pre-releases, 0.<build> (0.221 = build v221), and show no number.
- The viewer released with it is **FoulBrood II v1.0**.
