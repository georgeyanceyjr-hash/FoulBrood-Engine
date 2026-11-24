"""Convert a Foulbrood v3 book.json into a ThoraxRex book (key weight move)."""
import sys, json, subprocess, collections
src, dst = sys.argv[1], sys.argv[2]
eng = sys.argv[3] if len(sys.argv) > 3 else './target/release/thoraxrex'
p = subprocess.Popen([eng], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
def read():
    out = []
    while True:
        l = p.stdout.readline().strip()
        if l == 'ok': return out
        out.append(l)
read()
def cmd(c):
    p.stdin.write(c + '\n'); p.stdin.flush(); return read()
cmd('options set UseBook False')
d = json.load(open(src))
agg = collections.defaultdict(lambda: collections.Counter())
bad = 0; n = 0
for k, v in d['entries'].items():
    for ch in v['choices']:
        ex, mv, w = ch.get('example'), ch.get('example_move'), ch.get('weight', 1)
        if not ex or not mv: continue
        r = cmd('newgame ' + ex)
        if not r or r[0].startswith('err'): bad += 1; continue
        key = cmd('key')[0]
        g = cmd('play ' + mv)
        if not g or g[0].startswith(('err', 'invalid')): bad += 1; continue
        canon = g[0].split(';')[-1]
        agg[key][canon] += w; n += 1
with open(dst, 'w') as f:
    for key, c in agg.items():
        for m, w in c.items():
            f.write(f'{key} {w} {m}\n')
print('choices', n, 'positions', len(agg), 'bad', bad)
