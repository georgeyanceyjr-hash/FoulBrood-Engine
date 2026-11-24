#!/bin/zsh
# ThoraxRex test queue. Runs test jobs dropped into queue/*.job one after another.
# Close this window to stop.
cd "$(dirname "$0")"
mkdir -p queue
echo "ThoraxRex test queue started $(date). Waiting for jobs in queue/ (close this window to stop)."
while true; do
  j=$(ls queue/*.job 2>/dev/null | sort | head -1)
  if [ -n "$j" ]; then
    echo "== $(date) running $j"; cat "$j"
    mv "$j" "$j.running"
    ( source "$j.running"; export VARIANTS OPPS SHARDS GAMES; ./tools/par.command )
    mv "$j.running" "$j.done"
    echo "== $(date) finished $j"
  else
    sleep 20
  fi
done
