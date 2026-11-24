#!/bin/zsh
# Builds ThoraxRex and plays a test match against Foulbrood 5.3.
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p results
TAG=$(date +%m%d-%H%M)
echo "Building ThoraxRex..."
cargo build --release > results/build-$TAG.log 2>&1 || { echo BUILD FAILED; exit 1; }
E=$HOME/Documents/Codex/FoulBroodData/Engines
echo "ThoraxRex vs Foulbrood 5.3, 20 games, 1s/move"
MATCH_GAMES=results/games-fb53-$TAG.txt python3 -u tools/match.py target/release/thoraxrex "$E/Foulbrood-5.3/wrapper.py" 20 1 Base+MLP 1 2>&1 | tee results/match-fb53-$TAG.log
echo DONE
