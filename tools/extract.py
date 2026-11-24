"""Replay games through ThoraxRex and dump (features, side, result) rows.
Input: JSON lines {"moves":[...], "gt":"Base+MLP", "result":1|0|0.5}"""
import sys, json, subprocess
inp, outp = sys.argv[1], sys.argv[2]
eng = sys.argv[3] if len(sys.argv) > 3 else './target/release/thoraxrex'
p = subprocess.Popen([eng], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
def read():
    out = []
    while True:
        l = p.stdout.readline()
        if not l: raise EOFError
        l = l.strip()
        if l == 'ok': return out
        out.append(l)
read()
def cmd(c):
    p.stdin.write(c + '\n'); p.stdin.flush(); return read()
rows = 0; bad = 0
with open(outp, 'w') as out:
    for line in open(inp):
        g = json.loads(line)
        r = cmd('newgame ' + g['gt'])
        ok = True
        n = len(g['moves'])
        for k, m in enumerate(g['moves']):
            if k >= 8 and k < n - 1:
                f = cmd('features')[0]
                out.write(f"{f};{k % 2};{g['result']}\n"); rows += 1
            r = cmd('play ' + m)
            if not r or r[0].startswith(('err', 'invalid')):
                bad += 1; ok = False; break
print('rows', rows, 'bad games', bad)
