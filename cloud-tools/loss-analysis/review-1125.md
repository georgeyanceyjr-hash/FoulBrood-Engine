# HotFrass games, 25 Nov 2026 (net q17, new clock use)

27 games after q17 was installed: 7 wins, 20 losses.
Every loss was to Antsy (9) or Formi (11); every other opponent was beaten (AlphaHive 3, Hammerhead-Lite 2, two humans).

What the losses have in common (losses-1125-q17.jsonl has the moves and the engine's own score for each of its moves):

- As White the engine rates itself well ahead (+80 to +97 on the viewer scale) until about move 25-40, then the score
  collapses within one or two of its own moves. In 16 of the 20 losses there is a single opponent move after which the
  score falls by 50 points or more (a beetle move in 8 of them, an ant in 4).
- The opponent's beetle or mosquito ends up on top of our pillbug in 17 of the 20 losses, but in most of them that
  happens AFTER the engine already rates itself lost: it is how they finish, not usually how they start. (My first
  reading, that covering the pillbug was the cause, was wrong.)
- The book repeats itself: as White, wP; bG1 -wP; wA1 wP/ was lost 7 times out of 7 (two continuations).

Turning-point test (turnset.json, turnrun.py): the 16 positions just before the score collapsed, searched for 2 s.
Mean score from HotFrass's side; the truth two moves later averaged -44. Lower is better; controls (12 positions from
won games) must stay near +100.

| setting | turning points | controls |
|---|---|---|
| q17 (installed) | +42 | +99 |
| q12 | +41 | +99 |
| q15 | +42 | +99 |
| q17, 8 s instead of 2 s | +29 | +99 |
| q17, quiescence 2 / 4 | +36 / +33 | +99 |
| q17, extensions (TR_EXT4) | +33 | +99 |
| q17, no null move / no RFP | +46 / +41 | +99 |
| q14a (self-play + 4,496 strong-player games) | +18 | +100 |

So neither more time nor search settings fix it, three generations of self-play nets are identical on it, and the one
net trained with strong-player games is clearly better. Job ZZe-q19hg trains q19/q19b: the current self-play data
plus those games.

Done on 25 Nov: book v7 (tools/thoraxrex-book-v7.txt) drops wA1 wP/ after wP; bG1 -wP in every orientation, leaving
wQ wP- (82%) and wM wP/ (18%); installed in the viewer's FoulBrood Book engine.

## 26 Nov: overnight nets on the same test

The timed (2 s) version of the test turned out noisy: q14a scored +18 in one run and +36 in another. At a fixed
depth of 6 (`turnrun.py -6 ...`) repeat runs agree within a couple of points, so these are the numbers to use:

| net | trained on | turning points (depth 6) | controls | vs q17, 200 games |
|---|---|---|---|---|
| q17 (installed) | self-play | +30, +32 (two runs) | +99 | - |
| q18 | self-play + one more round | +30 | +99 | 103.0 |
| q14a | older self-play + strong-player games | +19 | +100 | - |
| q19 | self-play + strong-player games once | +11 | +99 | 102.5 |
| q19b | self-play + strong-player games twice | +13 | +99 | 98.5 |

q18 did not reach the 110/200 bar and was not installed; another self-play round has stopped paying.
q19 is level with q17 in self-play and clearly less blind on the turning points. Not installed: waiting for the user.
