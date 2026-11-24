#!/bin/bash
# mv.sh N SECS "site moves ;-separated"  -> asks the Mac move server, prints info + go(...) call
D=$HOME/mnt/georgeyancey/Documents/Codex/ThoraxRex; cd $D
N=$1; S=$2; M="$3"
K=$(echo "$M" | tr ';' '\n' | grep -c .)
C=$([ $((K%2)) -eq 0 ] && echo White || echo Black)
printf '%s\nBase+MLP;InProgress;%s[%d];%s\n' "$S" "$C" $((K/2+1)) "$M" > live/req-$N.tmp; mv live/req-$N.tmp live/req-$N.txt
for i in $(seq 1 $((S*2+40))); do [ -e live/resp-$N.txt ] && break; sleep 0.5; done
MV=$(sed -n 1p live/resp-$N.txt); sed -n 2p live/resp-$N.txt | sed 's/nodes.*//'
echo "await go($(python3 tools/hg.py "$M" "$MV" | python3 -c "import sys,json; d=json.load(sys.stdin); d['move']=sys.argv[1]; print(json.dumps(d))" "$MV"), $((K+2)));"
