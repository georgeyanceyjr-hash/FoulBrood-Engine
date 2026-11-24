import json, subprocess, sys
from collections import defaultdict
MAXPLY = int(sys.argv[1]) if len(sys.argv) > 1 else 20
src = '/mnt/user-data/uploads/georgeyancey/Downloads/hivegame-games-all-4946.jsonl'
BOTS = {'Formi', 'Antsy', 'SixfoldOptera'}
games = []
for l in open(src):
    x = json.loads(l)
    st = x.get('st')
    try: s = json.loads(st) if isinstance(st, str) else st
    except Exception: s = None
    if not isinstance(s, dict) or 'Finished' not in s: continue
    f = s['Finished']
    res = 1.0 if f == {'Winner': 'White'} else 0.0 if f == {'Winner': 'Black'} else 0.5 if f == 'Draw' else None
    if res is None: continue
    strong = min(x.get('wr', 0), x.get('br', 0)) >= 1900 or x.get('wu') in BOTS or x.get('bu') in BOTS
    if not strong or x.get('gt') != 'MLP': continue
    games.append(([(p + ' ' + d).strip() for p, d in x['hist']], res))
for l in open('bots/b1.jsonl'):
    x = json.loads(l)
    res = {'White': 1.0, 'Black': 0.0}.get(x['win'], 0.5)
    games.append((x['hist'], res))
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
for hist, res in games:
    next(it)  # newgame
    for i, m in enumerate(hist[:MAXPLY]):
        k = next(it).strip().split('\n')[-1]
        pl = next(it)
        if 'err' in pl.lower() or 'invalid' in pl.lower(): bad += 1; break
        sc = res if i % 2 == 0 else 1 - res
        e = stats[k][m]; e[0] += 1; e[1] += sc
print('bad', bad, 'positions', len(stats), file=sys.stderr)
lines = 0
with open('thoraxrex-book-new.txt', 'w') as f:
    for k, mv in stats.items():
        tot = sum(v[0] for v in mv.values())
        if tot < 4: continue
        cand = [(m, n, s / n) for m, (n, s) in mv.items() if n >= 2]
        if not cand: continue
        best = max(c[2] for c in cand)
        for m, n, sc in cand:
            if sc >= best - 0.15 and sc >= 0.25:
                w = max(1, round(n * (sc + 0.05) * 10))
                f.write(f"{k} {w} {m}\n"); lines += 1
print('lines', lines, file=sys.stderr)
