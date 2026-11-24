"""Bot-move agreement: botmm.py GAMES.jsonl N secs_or_depth [seed] [maxply] -> % positions where ThoraxRex picks the move a
winning Formi/Antsy played (winner to move, plies 4..maxply). Env vars (TR_*) pass through to the engine."""
import sys, json, random, subprocess, os
exec(open('/home/claude/mm.py').read().split('net, depth, N')[0])
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
env = dict(os.environ, TR_BOOK='none', TR_NET=os.environ.get('TR_NET', '/home/claude/nn/net14.txt'))
out = subprocess.run(['/home/claude/propolis/target/release/thoraxrex'], input=''.join(cmds) + 'exit\n', capture_output=True, text=True, env=env).stdout
blocks = out.split('\nok\n'); lines = [b.strip().split('\n')[-1] for b in blocks[2::2]]
hit = tot = 0
for (pre, played), eng in zip(samples, lines):
    try:
        tot += 1; hit += canon(pre, eng.strip()) == canon(pre, played)
    except Exception:
        pass
print(f"{len(games)} games, {tot} positions, depth {depth}: {hit}/{tot} = {100*hit/max(tot,1):.1f}%")
