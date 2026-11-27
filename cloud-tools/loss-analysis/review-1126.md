# Review of 26 Nov games (net q19, book v7)

Scores are on the viewer scale: -100 = forced Black win, +100 = forced White win. "Own rating" is from HotFrass's side.

## Results
- 26 Nov (q19): 20 games. 0 of 16 against Antsy and Formi; 4 of 4 against AlphaHive and tigrisbela.
- 25 Nov (q17): 0 of 22 against Antsy and Formi.
- Two days: 0 of 38 against the two top bots (Antsy 2800, Formi 2740; HotFrass about 2200).

## How the losses unfold (HotFrass's own rating during the games)
| Moves | 25 Nov (q17) | 26 Nov (q19) |
|---|---|---|
| 7-12 | +30 | +25 |
| 13-18 | +42 | +38 |
| 19-24 | +31 | +32 |
| 25-30 | -3 | +1 |
| 31-40 | -57 | -31 |

It stays at -20 or worse from about move 30 (25 Nov) / 33 (26 Nov). q19 did not change the picture.

## Calibration test (new): calset.json
231 positions at HotFrass's turn, moves 13-26: 155 from the 38 losses to Antsy/Formi, 76 from wins.
Fixed depth 6, no book (calrun.py):
| Net | Losses to Antsy/Formi | Wins | Gap |
|---|---|---|---|
| q17 | +35 | +79 | 44 |
| q19 | +26 | +77 | 51 |
| in the games | +34 | +85 | 50 |

A good net rates the first column at 0 or below while keeping the second high.

## Is it search depth?
16 of the lost-game positions, q19: depth 6 gives +47, 30 seconds (depth 9) gives +40 (deeprun.py, deep.json).
Deeper search barely moves it, so the optimism is in the evaluation, not the horizon.

## Unused data
tools/arch_mv.jsonl has 6,249 Antsy/Formi games. Only 2,833 of them are among the 4,496 strong-player games used
for q14a/q19. tools/arch_new.jsonl (3,003 games) is the unused part.

## Queued: ZZf-q20arch.job
- q20 = sp2..sp14 + strong-player games + the 3,003 unused Antsy/Formi games
- q20b = same, with the unused games twice and HotFrass's own 58 bot games three times
- each plays q19 for 200 games at 1 s; nothing is installed automatically
- afterwards: run calrun.py on both nets

## q20 (trained 26 Nov, 15:40 MST)
| Test | q17 | q19 | q20 |
|---|---|---|---|
| Calibration, positions from losses (lower is better) | +35 | +26 | +22 |
| Calibration, positions from wins | +79 | +77 | +78 |
| Picks the move Antsy/Formi played, 160 positions, depth 6 | - | 24% | 27% |

## Is the rating wrong, or is HotFrass outplayed afterwards?
300 positions from decisive Antsy-v-Formi games (moves 13-26), q19 at depth 4 (peerrun.py):
| q19 rating for the side to move | Positions | Side to move won |
|---|---|---|
| -100..-41 | 113 | 22% |
| -40..-16 | 23 | 48% |
| -15..+14 | 18 | 72% |
| +15..+39 | 18 | 44% |
| +40..+69 | 23 | 70% |
| +70..+100 | 105 | 78% |

The rating points the right way but is overconfident: positions it calls +70 or better are still lost 22% of the
time between equal top bots. So a +30 rating in HotFrass's own games is not proof of a blind spot; much of the
gap is that Antsy and Formi play the following moves better.

## Move-agreement benchmark (new): agreeset.json
160 positions from Antsy-v-Formi games with the move the bot played. agreerun.py reports how often the engine picks
that move at a fixed depth. A fast way to compare engine changes against top-bot play without playing rated games.

## Evening: q20 / q20b and the move-policy net

- q20 (adds 3,003 unused Antsy/Formi games) v q19: 100.5/200. Not installed.
- q20b (those games x2, HotFrass's own bot games x3): match v q19 in progress, 80.5/148 at the time of writing.
  Calibration at depth 6: positions from losses +7 (q19 +26), from wins +79. In-sample (q20b trained on these games).
- Move agreement with Antsy/Formi at depth 6, 160 archive positions: no policy 22-24%, policy_v3b for ordering 31%,
  new root prior (TR_POLROOT) 47-53%. But those positions are in the policy's training games.
- Unseen positions (holdset.json: 200 Antsy/Formi moves from HotFrass's games of 25-26 Nov): search 32%, raw policy 32%,
  search + root prior 32-33%. policy_v3b memorised its games (50% on seen, 32% on unseen).
- New: TR_POLROOT / TR_POLROOTCAP in search.rs (root handicap from the policy). Off by default.
- Queued on the Mac: ZZf2-policy4 (policy v4: 14,096 games, winner's + top-bot moves, dropout, best epoch kept),
  ZZg-polroot (root 50, root 100, policy-only, each 200 games v v218 at 1 s).
- q20b v q19 final: 100.0/200 (+92 =16 -92). Not installed. Neither extra-data net beats q19 at 1 s.
- v3b matches cancelled at the user's request. Policy v4 first run failed (no torch in ThoraxRex/.venv-training); the
  job now installs torch there. Dump: 14,096 games, 431,788 positions. Training restarted 00:50 UTC 27 Nov.
- Policy v4 (d96 L4, 431,788 positions, dropout .1, best epoch 13): validation top-1 36.7%, top-3 64.4%; v4s 35.5%.
  Unseen HotFrass-game positions: raw policy 34% (v3b 32%), on its own training games 39% (v3b 50%) - no longer memorising.
  Search + v4 for ordering 32%, + root prior 50: 34%. Search alone 32%.
- Match, v219 + policy v4 + TR_POLROOT=50 v v218 at 1 s: 54.0/200 (+23 =62 -115). The root prior is harmful; many draws.
  Leave TR_POLROOT off. Policy-for-ordering match (v4polonly) in progress.
- Match, v219 + policy v4 for move ordering only (TR_POLROOT off) v v218 at 1 s: 110.0/200 (+105 =10 -95).
  Meets the first bar. Confirmation (200 more games, new seeds) queued as ZZk-pol4conf; install only if combined >= 55%.
