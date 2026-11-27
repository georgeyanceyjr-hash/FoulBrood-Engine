"""Play a UHP engine match. usage: match.py engA engB games secs [gametype] [seed]"""
import random, sys, time, os
sys.path.insert(0, os.path.dirname(__file__))
from uhp import Engine

DEPTH = int(os.environ.get('MATCH_DEPTH', '0'))
def play_game(white, black, gt, opening, secs, maxply=400):
    for e in (white, black):
        e.cmd('newgame ' + gt)
    moves = []
    for m in opening:
        for e in (white, black):
            r = e.cmd('play ' + m)
            if r and r[0].startswith(('err','invalid')): raise RuntimeError(f'{r} {m}')
        moves.append(m)
    t = time.strftime('%H:%M:%S', time.gmtime(int(secs))) + ('.%03d' % round((secs % 1) * 1000) if secs % 1 else '')
    while True:
        st = white.cmd('validmoves')  # state check through game string
        gs = white.cmd('undo 0') if False else None
        side = len(moves) % 2
        e = white if side == 0 else black
        try:
            if DEPTH: r = e.cmd('bestmove depth %d' % DEPTH)
            elif t: r = e.cmd('bestmove time ' + t)
            else: r = e.cmd('bestmove time 00:00:01')
        except EOFError as ex:
            raise EOFError(str(ex) + ' moves=' + ';'.join(moves))
        mv = r[-1]
        if mv.startswith('err') and 'game is over' in mv.lower():
            return 'Draw', moves  # engine applied the repetition-draw rule
        if mv.startswith(('err','invalid')): raise RuntimeError(f'bestmove {mv} after {moves}')
        res = None
        for x in (white, black):
            g = x.cmd('play ' + mv)
            if g and g[0].startswith(('err','invalid')) and 'game is over' in g[0].lower():
                return 'Draw', moves + [mv]  # one engine applied the repetition-draw rule
            if g[0].startswith(('err','invalid')): raise RuntimeError(f'{g} for {mv} after {moves}')
            res = g[0].split(';')[1]
        moves.append(mv)
        if os.environ.get('MATCH_VERBOSE'): print(len(moves), mv, res, flush=True)
        if res in ('WhiteWins', 'BlackWins', 'Draw'):
            return res, moves
        if len(moves) >= maxply:
            return 'Draw', moves

def main():
    # A queued match can be cancelled: put part of its games-file name on a line of results/skip.txt.
    try:
        tags = [t.strip() for t in open('results/skip.txt') if t.strip()]
        if any(t in os.environ.get('MATCH_GAMES', '') for t in tags):
            print('skipped (results/skip.txt)'); return
    except OSError:
        pass
    a, b, n, secs = sys.argv[1], sys.argv[2], int(sys.argv[3]), float(sys.argv[4])
    gt = sys.argv[5] if len(sys.argv) > 5 else 'Base+MLP'
    rng = random.Random(int(sys.argv[6]) if len(sys.argv) > 6 else 1)
    A = Engine(a)
    # The opponent must not inherit the variant's TR_* settings (net, book, eval terms); give it its own via MATCH_OPPENV="K=V,K=V".
    saved = {k: v for k, v in os.environ.items() if k.startswith('TR_')}
    for k in saved: del os.environ[k]
    for kv in filter(None, os.environ.get('MATCH_OPPENV', '').split(',')):
        k, v = kv.split('=', 1); os.environ[k] = v
    B = Engine(b)
    for kv in filter(None, os.environ.get('MATCH_OPPENV', '').split(',')):
        os.environ.pop(kv.split('=', 1)[0], None)
    os.environ.update(saved)
    for E, tag in ((A, 'A'), (B, 'B')):
        threads = os.environ.get('MATCH_THREADS_' + tag) or os.environ.get('MATCH_THREADS')
        if threads:
            E.cmd('options set Threads ' + threads)
            E.cmd('options set NumThreads ' + threads)
        # keep reference engines from pondering on the opponent's time
        E.cmd('options set BackgroundPondering False')
        E.cmd('options set PonderDuringIdle Disabled')
        E.cmd('options set RandomOpening False')
    names = (os.path.basename(a), os.path.basename(b))
    score = [0.0, 0.0]; w = d = l = 0
    firsts = ['wG1','wS1','wB1','wA1','wL','wP','wM']
    seconds = ['bG1','bS1','bB1','bA1','bL','bP','bM']
    dirs = ['-wX', 'wX-', '/wX', 'wX/', '\\wX', 'wX\\']
    for i in range(n // 2):
        f = rng.choice(firsts); s = rng.choice(seconds); dr = rng.choice(dirs).replace('wX', f)
        opening = [f, f'{s} {dr}']
        extra = int(os.environ.get('MATCH_RANDPLIES', '0'))
        if extra:
            A.cmd('newgame ' + gt)
            for mv0 in opening:
                A.cmd('play ' + mv0)
            for _ in range(extra):
                vm = [x for x in A.cmd('validmoves')[-1].split(';') if x and x != 'pass' and not x.startswith(('wQ', 'bQ'))]
                if not vm:
                    break
                mv0 = rng.choice(sorted(vm))
                A.cmd('play ' + mv0)
                opening.append(mv0)
        if os.environ.get('MATCH_NOOPEN'):
            opening = []
        for swap in (0, 1):
            white, black = (A, B) if swap == 0 else (B, A)
            try:
                res, moves = play_game(white, black, gt, opening, secs)
            except EOFError as ex:
                print('CRASH', ex, 'opening', opening, flush=True); raise
            if res == 'Draw': r = 0.5
            elif (res == 'WhiteWins') == (swap == 0): r = 1.0
            else: r = 0.0
            score[0] += r; score[1] += 1 - r
            w += r == 1; d += r == 0.5; l += r == 0
            open(os.environ.get('MATCH_GAMES','/tmp/games.txt'),'a').write(f'{res} {names[0]}={"W" if swap==0 else "B"} {gt};'+';'.join(moves)+'\n')
            print(f'game {2*i+swap+1}: {names[0]} as {"White" if swap==0 else "Black"}: {res} in {len(moves)} plies  | total {names[0]} {score[0]}-{score[1]} (+{w} ={d} -{l})', flush=True)
    A.close(); B.close()

main()
