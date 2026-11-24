#!/bin/zsh
# Builds ThoraxRex (with opening book) and plays 20-game matches vs Foulbrood 4.1, Nokamute and Foulbrood 5.3.
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p results
TAG=$(date +%m%d-%H%M)
echo "Building ThoraxRex..."
cargo build --release > results/build-$TAG.log 2>&1 || { echo BUILD FAILED; exit 1; }
cp thoraxrex-book.txt target/release/thoraxrex-book.txt
E=$HOME/Documents/Codex/FoulBroodData/Engines
for opp in "Foulbrood-4.1/wrapper.py" "Nokamute/nokamute" "Foulbrood-5.3/wrapper.py"; do
  name=${opp%%/*}
  echo "ThoraxRex vs $name, 20 games, 1s/move"
  MATCH_GAMES=results/games-$name-$TAG.txt python3 -u tools/match.py target/release/thoraxrex "$E/$opp" 20 1 Base+MLP 11 2>&1 | tee results/match-$name-$TAG.log
done
echo "GAUNTLET DONE" | tee results/gauntlet-$TAG.done
