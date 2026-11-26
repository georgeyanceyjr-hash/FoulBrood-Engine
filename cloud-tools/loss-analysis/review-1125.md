# HotFrass games, 25 Nov 2026 (net q17, new clock use)

27 games after q17 was installed: 7 wins, 20 losses.
Every loss was to Antsy (9) or Formi (11); every other opponent was beaten (AlphaHive 3, Hammerhead-Lite 2, two humans).

What the losses have in common (losses-1125-q17.jsonl has the moves and the engine's own score for each of its moves):

- As White the engine rates itself well ahead (+80 to +97 on the viewer scale) until about move 25-40, then the score
  collapses within a few moves. It does not see the trouble until the search reaches it.
- In 15 of 20 losses an enemy beetle or mosquito had climbed before the score turned; in 17 the opponent's beetle or mosquito
  ends up on top of OUR pillbug, which switches off the queen's defender.
- The book repeats itself: as White, the line wP; bG1 -wP; wA1 wP/; bA1 \bG1; wQ wA1\; bB1 /bA1 was lost 4 times out of 4,
  and wP; bG1 -wP; wA1 wP/; bB1 -bG1; wQ wA1\; bQ -bB1 3 times out of 3.

Next steps suggested: stop repeating the two lost White lines; build a test set from these games of positions where
an enemy beetle is about to cover our pillbug, and measure candidate fixes against it.
