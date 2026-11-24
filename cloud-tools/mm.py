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
net, depth, N = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]); seed = int(sys.argv[4]) if len(sys.argv) > 4 else 1
rng = random.Random(seed)
games = [json.loads(l) for l in open('/home/claude/nn/hg_all.jsonl')][3800:]
samples = []
while len(samples) < N:
    g = rng.choice(games); mv = g['moves']; res = g['result']
    if res not in (0, 1) or len(mv) < 14: continue
    k = rng.randrange(8, min(len(mv) - 1, 50))
    if (k % 2 == 0) != (res == 1): continue  # winner to move
    samples.append((mv[:k], mv[k]))
cmds = []
for pre, _ in samples:
    n = len(pre); t = n // 2 + 1; c = 'White' if n % 2 == 0 else 'Black'
    cmds.append(f"newgame Base+MLP;InProgress;{c}[{t}];{';'.join(pre)}\nbestmove depth {depth}\n")
env = dict(os.environ, TR_BOOK='none', TR_NET=net)
out = subprocess.run(['/home/claude/propolis/target/release/thoraxrex'], input=''.join(cmds) + 'exit\n', capture_output=True, text=True, env=env).stdout
blocks = out.split('\nok\n'); lines = [b.strip().split('\n')[-1] for b in blocks[2::2]]
hit = 0; tot = 0
for (pre, played), eng in zip(samples, lines):
    try:
        tot += 1; hit += canon(pre, eng.strip()) == canon(pre, played)
    except Exception as e:
        pass
print(f"{net} depth {depth}: {hit}/{tot} = {100*hit/max(tot,1):.1f}% agreement")
