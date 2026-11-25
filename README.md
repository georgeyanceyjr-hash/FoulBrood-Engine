# FoulBrood engine

A Hive engine (base game plus Mosquito, Ladybug and Pillbug) written from scratch in Rust. It speaks the
Universal Hive Protocol, so it works in the [FoulBrood viewer](https://github.com/georgeyanceyjr-hash/Foulbrood)
and other UHP programs. It plays on hivegame.com as the bot **HotFrass**.

This is **version 1.0**. During development the engine was called ThoraxRex (and its working folder still is);
the program file is still named `thoraxrex`.

## Build and run

    cargo build --release

The engine is `target/release/thoraxrex`. Put a net next to it as `thoraxrex-net.txt` and, for the opening book,
`thoraxrex-book.txt` (`h2h/v215/` is the build currently installed in the viewer — the v214 program with net q15, installed 25 Nov 2026 after 225.5/400 against net q12 over two matches; `h2h/v214/` is version 1.0 with a faster net evaluation, same net and same scores as v213; `h2h/v213/` is version 1.0 as first built for Apple Silicon, with its net; `tools/thoraxrex-book-v5.txt` is the book). Then add the program to
the viewer on its Engine tab. It is listed as "FoulBrood Book" when it finds a book and "FoulBrood" without one.

## What it is

- Rules core checked against reference move counts (perft depth 6 = 192,353,904) and random positions.
- Search: iterative-deepening alpha-beta with a transposition table, threads, null move, late-move reductions
  and a queen-pressure quiescence search.
- Evaluation: a small neural net (64x32) over hand-built features plus queen-neighbourhood and pillbug inputs,
  trained on the engine's own games.
- Opening book looked up in all 12 orientations of the board.
- Analyses positions that have no move history (the viewer's "Set up position").
- Reports its score to the viewer (`FoulBroodScore` lines) and supports `Threads`, `HashMB`.

## What is in this repository (it is also the project's backup)

| Folder | Contents |
|---|---|
| `src/`, `Cargo.toml` | the engine |
| `h2h/` | every tested version as built for Apple Silicon, each with its net (`v213` = version 1.0; `v214` = the same with a faster net evaluation, installed 24 Nov 2026 after 54.5/96 against v213; `v215` = v214 with net q15, trained with 4,000 added depth-6 self-play games, installed 25 Nov 2026 after 117.5/200 and 108/200 against q12) |
| `tools/` | match runner, training script (`train_qn.py`), nets (`net_*.txt`), weight sets, test and book tools |
| `queue/` | the job files that produced each result, in order: they are the record of how every net was made |
| `results/datagen/` | the self-play games each net was trained on (`sp2` to `sp11`) |
| `results/summary-*.txt` | the score of every test match |
| `*.rs` in the top folder | earlier variants of the evaluation and search that were tested and set aside |
| `cloud-tools/` | scripts used off the Mac: book building and checking, loss analysis, symmetry and set-up tests |
| `run_queue.command`, `tools/par.command` | the queue runner and the parallel match script |

To retrain a net from the games here: build the engine, turn each `results/datagen/spN_train.jsonl` into
features with `thoraxrex dumpfeat <games> <out.csv> extra`, then run `tools/train_qn.py` as the newest
`queue/*loop*.job` shows.

## Game records and opening sources

- `tools/bots_arch*.jsonl`, `tools/arch_mv.jsonl`, `tools/top_mv.jsonl`, `tools/hg_*.jsonl` and
  `game-records/hivegame-top1900-7289.jsonl.gz`: public game records from hivegame.com, which the opening book and
  some training starts were built from.
- `hivegame-bot/games.jsonl`: HotFrass's own games played through the viewer, with the engine's score at each move.
- `cloud-tools/fbook/formi-opening-reference.json` and `formi-check.in`: opening lines taken from the published
  opening book of the bot Formi (hive-bugs.fly.dev/book), used as a reference for the next book. They are Formi's
  author's work, included here only so the book can be rebuilt.

## What is not here

- The feature tables in `data/` (about 2 GB, with single files too large for GitHub). They are regenerated from the
  self-play games as above.
- Nokamute, used as a test opponent (`ref/`), which has its own home.
- Match logs move by move (`results/par-*`, `results/games-*`); the summaries are kept.

## Licence

MIT. See `LICENSE`.
