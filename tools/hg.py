"""Hive geometry helper for entering moves on hivegame.com.
usage: hg.py "<moves;...>" "<next move>"  -> JSON {anchor:{c,bug}, src:[dq,dr]|null, dst:[dq,dr], first:bool}"""
import sys, json
DIRS = {('', '-'): (1, 0), ('-', ''): (-1, 0), ('', '/'): (1, -1), ('\\', ''): (0, -1), ('', '\\'): (0, 1), ('/', ''): (-1, 1)}
BUG = {'Q': 'Queen', 'A': 'Ant', 'B': 'Beetle', 'G': 'Grasshopper', 'S': 'Spider', 'M': 'Mosquito', 'L': 'Ladybug', 'P': 'Pillbug'}
def target(ref, pos):
    if ref == '':
        return None
    pre = ref[0] if ref[0] in '-/\\' else ''
    suf = ref[-1] if ref[-1] in '-/\\' else ''
    name = ref.strip('-/\\')
    q, r = pos[name][:2]
    if not pre and not suf:
        return (q, r)
    dq, dr = DIRS[(pre, suf)]
    return (q + dq, r + dr)
def replay(moves):
    pos = {}; stack = {}
    for i, mv in enumerate(moves):
        if mv == 'pass':
            continue
        parts = mv.split(' ')
        p = parts[0]; ref = parts[1] if len(parts) > 1 else ''
        t = (0, 0) if i == 0 else target(ref, pos)
        if p in pos:
            stack[pos[p][:2]].remove(p)
        stack.setdefault(t, []).append(p)
        pos[p] = (t[0], t[1], len(stack[t]) - 1)
    return pos, stack
moves = [m for m in sys.argv[1].split(';') if m]
nxt = sys.argv[2]
pos, stack = replay(moves)
out = {'first': len(moves) == 0}
if moves:
    # anchor: a piece alone on its cell whose colour+bug is unique among top pieces
    tops = {}
    for cell, st in stack.items():
        if st:
            p = st[-1]; key = (p[0], p[1]); tops.setdefault(key, []).append((p, cell, len(st)))
    cand = [v[0] for k, v in tops.items() if len(v) == 1 and v[0][2] == 1]
    a = cand[0]
    ap = a[1]
    out['anchor'] = {'c': 'white' if a[0][0] == 'w' else 'black', 'bug': BUG[a[0][1]]}
    parts = nxt.split(' '); p = parts[0]; ref = parts[1] if len(parts) > 1 else ''
    dst = target(ref, pos) if len(moves) > 0 else (0, 0)
    if len(moves) == 1:
        dst = target(ref, pos)
    out['dst'] = [dst[0] - ap[0], dst[1] - ap[1]]
    out['dst_h'] = len(stack.get(dst, []))
    if p in pos:
        s = pos[p]; out['src'] = [s[0] - ap[0], s[1] - ap[1]]; out['src_h'] = s[2]
    else:
        out['src'] = None
    out['piece'] = {'c': 'white' if p[0] == 'w' else 'black', 'bug': BUG[p[1]]}
print(json.dumps(out))
