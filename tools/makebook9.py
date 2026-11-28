"""Book v9: the engine's own move, from a long search, for every common early position (tools/book9-positions.txt),
laid over book v8 for the deeper lines it has. Positions v9 covers (in any of the 12 orientations) drop their v8 lines.
usage: makebook9.py ENGINE POSITIONS V8BOOK OUT [SECONDS] [WORKERS]
Run with TR_BOOK=none and TR_POLICY set; each worker searches with 2 threads."""
import subprocess, sys, threading, queue, time
eng, posf, v8, out = sys.argv[1:5]
secs = int(sys.argv[5]) if len(sys.argv) > 5 else 30
workers = int(sys.argv[6]) if len(sys.argv) > 6 else 4

class UHP:
    def __init__(s):
        s.p = subprocess.Popen([eng], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        s.read()
        s.cmd('options set Threads 2')
    def read(s):
        lines = []
        while True:
            l = s.p.stdout.readline()
            if not l: raise RuntimeError('engine closed')
            l = l.rstrip('\n')
            if l == 'ok': return lines
            lines.append(l)
    def cmd(s, c):
        s.p.stdin.write(c + '\n'); s.p.stdin.flush(); return s.read()

rows = [l.rstrip('\n').split('|') for l in open(posf) if l.strip() and not l.startswith('#')]
todo = queue.Queue()
for r in rows: todo.put(r)
result = {}; lock = threading.Lock(); t0 = time.time()

def work():
    e = UHP()
    while True:
        try: key, n, ply, pre = todo.get_nowait()
        except queue.Empty: return
        try:
            e.cmd('newgame Base+MLP')
            for m in [m for m in pre.split(';') if m]: e.cmd('play ' + m)
            k = e.cmd('key')[-1]; ks = [e.cmd('symkey %d' % t)[-1] for t in range(12)]
            mv = e.cmd('bestmove time 00:%02d:%02d' % (secs // 60, secs % 60))[-1]
            if mv and not mv.lower().startswith('err'):
                with lock:
                    result[key] = (k, ks, mv)
                    print('%d/%d %s %s -> %s (%.0fs)' % (len(result), len(rows), ply, pre[-40:], mv, time.time() - t0), flush=True)
        except Exception as x:
            print('skip', key, x, flush=True)

ts = [threading.Thread(target=work) for _ in range(workers)]
for t in ts: t.start()
for t in ts: t.join()
covered = set()
for k, ks, mv in result.values(): covered.update(ks)
lines = ['#name Book']
kept = replaced = 0
for l in open(v8):
    if l.startswith('#') or not l.strip(): continue
    if l.split(' ', 1)[0] in covered: replaced += 1; continue
    lines.append(l.rstrip('\n')); kept += 1
for k, ks, mv in result.values(): lines.append('%s 1000 %s' % (k, mv))
open(out, 'w').write('\n'.join(lines) + '\n')
print('book v9: %d engine positions, %d v8 lines kept, %d replaced, %d lines' % (len(result), kept, replaced, len(lines) - 1))
