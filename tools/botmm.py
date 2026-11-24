"""Top-player move agreement: mm.py NET DEPTH N [seed] -> % of sampled positions (winner to move) where engine plays the same move."""
import sys, json, random, subprocess, os
sys.argv_saved = sys.argv
DIRS = {('', '-'): (1, 0), ('-', ''): (-1, 0), ('', '/'): (1, -1), ('\\', ''): (0, -1), ('', '\\'): (0, 1), ('/', ''): (-1, 1)}
def target(ref, pos):
    pre = ref[0] if ref[0] in '-/\\' else ''; suf = ref[-1] if ref[-1] in '-/\\' else ''
    q, r = pos[ref.strip('-/\\')][:2]
    if not pre and not suf: return (q, r)
    dq, dr = DIRS[(pre, suf)]; return (q + dq, r + dr)
def replay(moves):
    pos = {}
    for i, mv in enumerate(moves):
        if mv == 'pass': continue
        p, *ref = mv.split(' ')
        pos[p] = (0, 0) if i == 0 else target(ref[0], pos)
    return pos
def canon(moves, mv):
    if mv == 'pass': return 'pass'
    pos = replay(moves); p, *ref = mv.split(' ')
    return (p, target(ref[0], pos) if ref else (0, 0))
path, N, depth = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
seed = int(sys.argv[4]) if len(sys.argv) > 4 else 1
maxply = int(sys.argv[5]) if len(sys.argv) > 5 else 40
BOTS = {'Formi', 'Antsy'}
rng = random.Random(seed)
games = [json.loads(l) for l in open(path)]
games = [g for g in games if g.get('gt') == 'MLP' and g.get('win') in ('White', 'Black') and len(g['hist']) >= 12
         and (g['w'] if g['win'] == 'White' else g['b']) in BOTS]
pool = []
for gi, g in enumerate(games):
    wpar = 0 if g['win'] == 'White' else 1
    for k in range(4, min(len(g['hist']) - 1, maxply)):
        if k % 2 == wpar:
            pool.append((gi, k))
rng.shuffle(pool)
samples = [(games[gi]['hist'][:k], games[gi]['hist'][k]) for gi, k in pool[:N]]
cmds = []
for pre, _ in samples:
    n = len(pre); t = n // 2 + 1; c = 'White' if n % 2 == 0 else 'Black'
    cmds.append(f"newgame Base+MLP;InProgress;{c}[{t}];{';'.join(pre)}\nbestmove depth {depth}\n")
env = dict(os.environ, TR_BOOK='none', TR_NET=os.environ.get('TR_NET', 'tools/net14.txt'))
out = subprocess.run([os.environ.get('TR_EXE','target/release/thoraxrex')], input=''.join(cmds) + 'exit\n', capture_output=True, text=True, env=env).stdout
blocks = out.split('\nok\n'); lines = [b.strip().split('\n')[-1] for b in blocks[2::2]]
hit = tot = 0
for (pre, played), eng in zip(samples, lines):
    try:
        tot += 1; hit += canon(pre, eng.strip()) == canon(pre, played)
    except Exception:
        pass
print(f"{len(games)} games, {tot} positions, depth {depth}: {hit}/{tot} = {100*hit/max(tot,1):.1f}%")
