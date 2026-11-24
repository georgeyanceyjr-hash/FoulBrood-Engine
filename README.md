# FoulBrood engine

A Hive engine (base game plus Mosquito, Ladybug and Pillbug) written from scratch in Rust. It speaks the
Universal Hive Protocol, so it works in the [FoulBrood viewer](https://github.com/georgeyanceyjr-hash/Foulbrood)
and other UHP programs. It plays on hivegame.com as the bot **HotFrass**.

This is **version 1.0**. During development the engine was called ThoraxRex (and its working folder still is);
the program file is still named `thoraxrex`.

## Build and run

    cargo build --release

The engine is `target/release/thoraxrex`. Put a net next to it as `thoraxrex-net.txt` and, for the opening book,
`thoraxrex-book.txt` (`h2h/v212/` has the net for 1.0; `thoraxrex-book.txt` is the book). Then add the program to
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
| `h2h/` | every tested version as built for Apple Silicon, each with its net (`v212` = 1.0's net) |
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

## What is not here

- The feature tables in `data/` (about 2 GB). They are regenerated from the self-play games as above.
- Game records of other hivegame.com players, which the opening book and some training starts were built from
  (`tools/bots_arch*.jsonl`, `tools/arch_mv.jsonl`, `tools/top_mv.jsonl`, `tools/hg_*.jsonl`). The finished
  book is included; rebuilding it from scratch needs those records downloaded again.
- Nokamute, used as a test opponent (`ref/`), which has its own home.
- Match logs move by move (`results/par-*`, `results/games-*`); the summaries are kept.

## Licence

MIT. See `LICENSE`.
