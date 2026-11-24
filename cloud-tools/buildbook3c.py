import json, subprocess, sys
from collections import defaultdict
MAXPLY = int(sys.argv[1]) if len(sys.argv) > 1 else 20
BOTS = {'Formi', 'Antsy', 'SixfoldOptera'}
games = []; seen = set()
for l in open('bots/arch2.jsonl'):
    x = json.loads(l)
    if x.get('gt') != 'MLP' or x['id'] in seen: continue
    if not (x['w'] in BOTS or x['b'] in BOTS or min(x.get('wr') or 0, x.get('br') or 0) >= 1900): continue
    res = {'White': 1.0, 'Black': 0.0}.get(x['win'])
    if res is None:
        if x.get('conc') in ('Board', 'Repetition', 'Draw') or x.get('conc') is None: res = 0.5
        else: continue
    seen.add(x['id']); games.append((x['hist'], res))
for l in open('/mnt/user-data/uploads/georgeyancey/Downloads/hivegame-top1900-7289.jsonl'):
    x = json.loads(l)
    if x['id'] in seen or x.get('gt') != 'MLP': continue
    s = x['status']; f = s.get('Finished') if isinstance(s, dict) else None
    res = 1.0 if f == {'Winner': 'White'} else 0.0 if f == {'Winner': 'Black'} else 0.5 if f == 'Draw' else None
    if res is None: continue
    seen.add(x['id']); games.append(([(p + ' ' + d).strip() for p, d in x['hist']], res))
print(len(games), 'games', file=sys.stderr)
cmds = []
for hist, res in games:
    cmds.append('newgame Base+MLP')
    for m in hist[:MAXPLY]:
        cmds += ['key', 'play ' + m]
out = subprocess.run(['propolis/target/release/thoraxrex'], input='\n'.join(cmds) + '\nexit\n', capture_output=True, text=True, env={'TR_BOOK': 'none'}).stdout
blocks = out.split('\nok\n')[1:]  # skip id
it = iter(blocks)
stats = defaultdict(lambda: defaultdict(lambda: [0, 0.0]))
bad = 0
prefix = {}
for hist, res in games:
    next(it)  # newgame
    for i, m in enumerate(hist[:MAXPLY]):
        k = next(it).strip().split('\n')[-1]
        pl = next(it)
        if 'err' in pl.lower() or 'invalid' in pl.lower(): bad += 1; break
        sc = res if i % 2 == 0 else 1 - res
        e = stats[k][m]; e[0] += 1; e[1] += sc
        prefix.setdefault(k, hist[:i])
print('bad', bad, 'positions', len(stats), file=sys.stderr)
import math
lines = 0; npos = 0
chk = open('bookcheck.txt', 'w')
with open('thoraxrex-book-v4b.txt', 'w') as f:
    for k, mv in stats.items():
        tot = sum(v[0] for v in mv.values())
        if tot < 8: continue
        cand = []
        for m, (n, sc) in mv.items():
            if n < 5: continue
            p = sc / n; pt = (sc + 1) / (n + 2)
            lcb = pt - math.sqrt(pt * (1 - pt) / (n + 2))
            cand.append((m, n, p, lcb))
        cand = [c for c in cand if c[2] >= 0.45]
        if not cand: continue
        best = max(c[3] for c in cand)
        npos += 1
        for m, n, p, lcb in cand:
            if lcb >= best - 0.05:
                f.write(f"{k} {max(1, round(100 * lcb * math.sqrt(n)))} {m}\n"); lines += 1
                chk.write(f"{k}|{max(1, round(100 * lcb * math.sqrt(n)))}|{m}|{n}|{p:.3f}|{';'.join(prefix[k])}\n")
print('positions', npos, 'lines', lines, file=sys.stderr)
