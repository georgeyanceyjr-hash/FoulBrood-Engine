"""Build the move-policy training games: every game once, movers to learn from are renamed K.
Keep = Antsy / Formi / SixfoldOptera moves always, and the winner's moves in every other game."""
import json, sys
TOP = {'Antsy', 'Formi', 'SixfoldOptera'}
import random
seen = set(); rows = []; n = k = 0
def put(moves, wk, bk):
    global n, k
    t = tuple(moves)
    if len(moves) < 12 or t in seen: return
    seen.add(t); n += 1; k += wk + bk
    rows.append(json.dumps({'w': 'K' if wk else 'x', 'b': 'K' if bk else 'x', 'hist': moves}))
for l in open('tools/bots_arch2.jsonl'):
    d = json.loads(l)
    put(d['hist'], d['w'] in TOP or d.get('win') == 'White', d['b'] in TOP or d.get('win') == 'Black')
for f in ('tools/arch_mv.jsonl', 'tools/top_mv.jsonl', 'tools/hg_all.jsonl'):
    for l in open(f):
        d = json.loads(l)
        put(d['moves'], d['result'] == 1, d['result'] == 0)
random.seed(4); random.shuffle(rows)
open(sys.argv[1], 'w').write('\n'.join(rows) + '\n')
print(n, 'games,', k, 'sides kept')
