#!/bin/zsh
# ThoraxRex parallel test: builds, then plays each weight variant vs Foulbrood 4.1 in parallel shards.
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p results results/err
TAG=$(date +%m%d-%H%M)
echo "Building ThoraxRex..."
cargo build --release > results/build-$TAG.log 2>&1 || { echo BUILD FAILED; cat results/build-$TAG.log; exit 1; }
cp thoraxrex-book.txt target/release/thoraxrex-book.txt
E=$HOME/Documents/Codex/FoulBroodData/Engines
OPPS=(${=OPPS:-Foulbrood-4.1/wrapper.py Foulbrood-5.3/wrapper.py Nokamute/nokamute})
VARIANTS=(${=VARIANTS:-t6 t7})
SHARDS=${SHARDS:-2}; GAMES=${GAMES:-10}
PIDS=()
for OPP in $OPPS; do
oname=${OPP%%/*}
for v in $VARIANTS; do
  wb=${v%%_c*}; cau=0; [[ $v == *_c* ]] && cau=${v##*_c}
  for s in $(seq 1 $SHARDS); do
    ( [ -f tools/v_$v.env ] && source tools/v_$v.env; export TR_SOFT TR_PARTIAL TR_CAUTION; WB=${WB:-$wb}; \
    TR_BOOK=${TR_BOOK:-none} MATCH_ERRDIR=results/err TR_CAUTION=$cau TR_WEIGHTS=tools/w_$WB.txt MATCH_THREADS=${TH:-1} MATCH_GAMES=results/games-$v-$oname-$TAG-$s.txt \
      python3 -u tools/match.py target/release/thoraxrex "$E/$OPP" $GAMES ${SECS:-1} Base+MLP $((s*101+${SO:-0})) > results/par-$v-$oname-$TAG-$s.log 2>&1 ) &
    PIDS+=($!)
  done
done
done
echo "Run ${#VARIANTS} vars x $SHARDS shards x $GAMES games vs ${#OPPS} opps                             "
( while true; do sleep 120; for v in $VARIANTS; do echo "$v: $(cat results/par-$v-*-$TAG-*.log 2>/dev/null | grep -c '^game') games done"; done; done ) &
MON=$!
wait $PIDS
kill $MON 2>/dev/null
python3 tools/sumpar.py results "$TAG" | tee results/summary-$TAG.txt
echo "PAR DONE" > results/par-$TAG.done
