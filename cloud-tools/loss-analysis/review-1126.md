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
