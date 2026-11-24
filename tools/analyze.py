import sys, subprocess
line = sys.argv[1]; secs = sys.argv[2] if len(sys.argv) > 2 else '00:00:00.5'; side = sys.argv[3] if len(sys.argv) > 3 else 'B'
g = line.split(';'); gt = g[0]; mv = g[1:]
p = subprocess.Popen(['./target/release/thoraxrex'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
def cmd(c):
    p.stdin.write(c + '\n'); p.stdin.flush(); out = []
    while True:
        l = p.stdout.readline().strip()
        if l == 'ok': return out
        out.append(l)
cmd_start = p.stdout.readline(); p.stdout.readline(); p.stdout.readline()
import threading, queue
errq = queue.Queue()
threading.Thread(target=lambda: [errq.put(l) for l in p.stderr], daemon=True).start()
cmd('options set Threads 2')
cmd('newgame ' + gt)
for k, m in enumerate(mv):
    s = 'W' if k % 2 == 0 else 'B'
    if s == side and k >= 4:
        best = cmd('bestmove time ' + secs)[-1]
        info = None
        import time; time.sleep(0.05)
        while not errq.empty():
            l = errq.get()
            if l.startswith('info'): info = l
        print(f'{k:3d} {s} played {m:14s} best {best:14s} {info}', flush=True)
    cmd('play ' + m)
