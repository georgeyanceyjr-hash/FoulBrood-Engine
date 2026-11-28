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
- Confirmation (v4polconf, new seeds): 112.0/200 (+104 =16 -80). Combined 222/400 = 55.5% - meets the install rule.
  Not installed yet: waiting for the user's go-ahead (install = v219 binary + policy_v4 as thoraxrex-policy.txt).

## Installed 27 Nov: v219 + policy v4 (move ordering only)
- Matches v v218 at 1 s: 110.0/200 then 112.0/200 (new seeds) = 222/400, 55.5%. Meets the install rule.
- Installed in FoulBroodData/Engines/ThoraxRex (bot, "FoulBrood Book") and ThoraxRex-NoBook: h2h/v219 binary,
  net q19 (unchanged), tools/policy_v4.txt as thoraxrex-policy.txt (found beside the binary). TR_POLROOT stays off.
- Previous engine kept in FoulBroodData/Engines/backup-v218 (binary, net, book).

## 27 Nov: tuning the installed engine (ZZm-poltune), all v installed v219 + policy v4, 200 games at 1 s
- climb (TR_CLIMB=1, what the bot used): 86.5/200 (+77 =19 -104). Harmful. Removed from the bot launcher
  (the viewer no longer writes it) with the user's OK.
- polk5 (TR_POLK=5): 92.0/200 (+86 =12 -102). pold3 (TR_POLD=3): 89.5/200 (+80 =19 -101). Defaults stay (K 3, D 4).
- Policy v5 (d128 L4, 18 epochs): validation top-1 36.8%, top-3 64.4% - same as v4 (36.7%). Its match and pold6
  were cancelled (results/skip.txt) to free the Mac. Nothing to confirm or install from this job.
- Next: ZZo-resources (v220: hash sized from RAM, 512 MB on the Mac; then 12 threads v 8), ZZp-qtorch
  (eval nets trained on the GPU: q21 64x32, q21w 128x32, q21x 256x64). ZZn stopped at a typo in the new
  trainer before training; fixed and re-queued as ZZp.

## 27 Nov afternoon: machine resources (ZZo)
- v220 (hash sized from RAM: 512 MB on the 24 GB Mac, was 64 MB; UHP option AnalysisMode for whole-time analysis)
  v v219, both 1 thread, 2 s: 102.5/200 (+96 =13 -91). Even at this speed, no harm.
- Installed 27 Nov 14:30 with the user's OK: h2h/v220 binary into FoulBroodData/Engines/ThoraxRex and
  ThoraxRex-NoBook (net q19, policy v4, book v8 unchanged). Previous engine in Engines/backup-v219.
- 12 threads v 8 threads (v220, one game at a time, 1 s): 38-40 after 78 games, running.

## 27 Nov evening
- 12 threads v 8 threads (v220, 1 s, one game at a time): 98.5/200. No gain: the 8-thread cap stays.
- GPU-trained eval nets (tools/train_qn_torch.py, same data as q19): q21 64x32 104.5/200 (best val 0.05044 v q19's
  0.05003 with the numpy trainer); q21w 128x32 about 67.5/150 when checked (val 0.05062); q21x 256x64 val 0.0511, its
  match skipped (results/skip.txt) to save an hour. The GPU trainer works but does not beat q19 on this data; more data
  is the next lever (after the release).
- Book: v8 covers 78/56/47/27% of common strong-game positions at plies 5-6/7-8/9-10/11-12 (book v4 84/69/57/32,
  Formi's book 11/4/0/0). Book v9 (ZZs) gives the engine's own 30 s move for all 707 such positions, over v8.
- FoulBrood Engine Formi Book added as a third built-in engine (Formi's published book, 1,782 positions).

## Night of 27-28 Nov results
- FoulBrood (installed v220, no book) v Nokamute, 400 games at 2 s: **393/400** (+393 =0 −7); 198 wins as White, 195 as Black.
- Contempt (v221 builds v v220): −25 scored 94/192, 60 scored 97.5/192. Neither is better; the −25 check fell under the 95 bar, so v221 was not installed (v220 stays). Default contempt 25 unchanged.
- GPU nets: q21 104.5/200 and q21w 88.5/200 v q19; below the 110 bar, not installed.
- Book v9 did not run: tools/thoraxrex-book-v8.txt was missing on the Mac. Copied it there and held the job as queue/ZZu-book9.job.hold for the next night.
- Self-play round 15 (q19, depth 6, 12 at a time) running until 05:45; 3,600 games by 23:32.
