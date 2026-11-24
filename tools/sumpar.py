import sys, glob, re, collections
d, tag = sys.argv[1], sys.argv[2]
tot = collections.defaultdict(lambda: [0, 0, 0, 0, 0])  # w d l, whiteW, blackW
for f in glob.glob(f'{d}/par-*-{tag}-*.log'):
    key = re.match(rf'.*/par-(.*)-{tag}-\d+\.log', f).group(1)
    for line in open(f):
        m = re.match(r'game \d+: \S+ as (White|Black): (WhiteWins|BlackWins|Draw)', line)
        if not m: continue
        col, res = m.groups(); t = tot[key]
        if res == 'Draw': t[1] += 1
        elif res[:5] == col: t[0] += 1; t[3 if col == 'White' else 4] += 1
        else: t[2] += 1
for k, (w, dr, l, ww, bw) in sorted(tot.items()):
    n = w + dr + l
    print(f'{k}: {w+dr/2:.1f}/{n} (+{w} ={dr} -{l})  wins as White {ww}, as Black {bw}')
