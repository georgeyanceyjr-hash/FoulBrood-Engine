#!/bin/bash
# Live move server (bash). Reads live/req-N.txt (line 1 = seconds, line 2 = UHP game string), writes live/resp-N.txt.
# Stops when live/stop exists or after ~4 idle hours.
shopt -s nullglob
mkdir -p live; rm -f live/stop live/req-*.txt live/resp-*.txt live/ready live/out-*.txt live/err-*.txt
BIN=h2h/pp/thoraxrex
TH=$(sysctl -n hw.perflevel0.physicalcpu 2>/dev/null || echo 4)
echo "bin=$BIN threads=$TH climb=1 net=q12 $(date)" > live/ready
idle=0
while [ ! -e live/stop ] && [ $idle -lt 48000 ]; do
  did=0
  for r in live/req-*.txt; do
    n=${r#live/req-}; n=${n%.txt}
    [ -e live/resp-$n.txt ] && continue
    S=$(sed -n 1p "$r"); G=$(sed -n 2p "$r")
    T=$(printf "00:%02d:%02d" $((S/60)) $((S%60)))
    printf "newgame %s\noptions set Threads %s\nbestmove time %s\nexit\n" "$G" "$TH" "$T" \
      | TR_CLIMB=1 TR_POLICY=none TR_BOOK=$PWD/tools/thoraxrex-book-v5.txt TR_NET=$PWD/tools/net_q12.txt $BIN > live/out-$n.txt 2> live/err-$n.txt
    { grep -v "^ok$" live/out-$n.txt | tail -1; grep "^info" live/err-$n.txt | tail -1; } > live/resp-$n.tmp
    mv live/resp-$n.tmp live/resp-$n.txt
    did=1; idle=0
  done
  if [ $did -eq 0 ]; then sleep 0.3; idle=$((idle+1)); fi
done
