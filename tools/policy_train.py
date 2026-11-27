"""Train the ThoraxRex move-policy transformer (mirrors src/policy.rs exactly).

usage: python3 tools/policy_train.py DUMP.txt OUT_WEIGHTS.txt [epochs] [d] [layers] [heads] [ff]
DUMP.txt comes from `thoraxrex policydump`. Positions are kept in file order; the last 8% (whole games at the end)
are held out for validation. Prints top-1 / top-3 accuracy per epoch and writes weights in the text format that
policy::load() reads.
"""
import sys, math, random, time, os
import numpy as np
import torch, torch.nn as nn, torch.nn.functional as F

TF, MF, NB, DB = 32, 12, 7, 9
path, out = sys.argv[1], sys.argv[2]
EPOCHS = int(sys.argv[3]) if len(sys.argv) > 3 else 12
D = int(sys.argv[4]) if len(sys.argv) > 4 else 64
L = int(sys.argv[5]) if len(sys.argv) > 5 else 3
H = int(sys.argv[6]) if len(sys.argv) > 6 else 4
FF = int(sys.argv[7]) if len(sys.argv) > 7 else 128
BS = 128
import os, copy
DROP = float(os.environ.get('POL_DROP', '0'))
LS = float(os.environ.get('POL_LS', '0'))
WD = float(os.environ.get('POL_WD', '1e-4'))
DEV = [None]
dev = 'mps' if torch.backends.mps.is_available() else ('cuda' if torch.cuda.is_available() else 'cpu')
torch.manual_seed(1); random.seed(1); np.random.seed(1)


def parse(line):
    p = line.split()
    i = 0
    assert p[i] == 'T'; n = int(p[i + 1]); i += 2
    tok = np.array(p[i:i + n * TF], dtype=np.float32).reshape(n, TF); i += n * TF
    cells = np.array(p[i:i + 2 * n], dtype=np.int16).reshape(n, 2); i += 2 * n
    assert p[i] == 'M'; k = int(p[i + 1]); i += 2
    per = 1 + NB + MF
    mv = np.array(p[i:i + k * per], dtype=np.float32).reshape(k, per); i += k * per
    assert p[i] == 'Y'; y = int(p[i + 1])
    return tok, cells, mv, y


def distmat(cells):
    n = len(cells)
    q = cells[:, 0].astype(np.int32); r = cells[:, 1].astype(np.int32)
    glob = (q == -99)
    dq = ((q[:, None] - q[None, :] + 16) & 31) - 16
    dr = ((r[:, None] - r[None, :] + 16) & 31) - 16
    d = (np.abs(dq) + np.abs(dr) + np.abs(dq + dr)) // 2
    d = np.minimum(d, 7)
    d[glob, :] = 8; d[:, glob] = 8
    return d.astype(np.int64)


t0 = time.time()
data = []
with open(path) as f:
    for line in f:
        try:
            tok, cells, mv, y = parse(line)
        except Exception:
            continue
        data.append((tok, distmat(cells), mv, y))
        if len(data) >= int(os.environ.get('POL_MAX', '10000000')):
            break
print(f'loaded {len(data)} positions in {time.time()-t0:.0f}s, device {dev}', flush=True)
nval = max(1, int(len(data) * 0.08))
train, val = data[:-nval], data[-nval:]


def collate(batch):
    B = len(batch)
    n = max(b[0].shape[0] for b in batch); k = max(b[2].shape[0] for b in batch)
    tok = np.zeros((B, n, TF), np.float32); dist = np.full((B, n, n), 8, np.int64); tmask = np.zeros((B, n), bool)
    piece = np.zeros((B, k), np.int64); nb = np.full((B, k, NB), -1, np.int64); mf = np.zeros((B, k, MF), np.float32)
    mmask = np.zeros((B, k), bool); y = np.zeros(B, np.int64)
    for i, (t, dm, mv, yy) in enumerate(batch):
        a = t.shape[0]; m = mv.shape[0]
        tok[i, :a] = t; dist[i, :a, :a] = dm; tmask[i, :a] = True
        piece[i, :m] = mv[:, 0].astype(np.int64); nb[i, :m] = mv[:, 1:1 + NB].astype(np.int64); mf[i, :m] = mv[:, 1 + NB:]
        mmask[i, :m] = True; y[i] = yy
    T = lambda x: torch.from_numpy(x).to(DEV[0])
    return T(tok), T(dist), T(tmask), T(piece), T(nb), T(mf), T(mmask), T(y)


class Layer(nn.Module):
    def __init__(s):
        super().__init__()
        s.ln1 = nn.LayerNorm(D); s.q = nn.Linear(D, D); s.k = nn.Linear(D, D); s.v = nn.Linear(D, D); s.o = nn.Linear(D, D)
        s.dist_bias = nn.Parameter(torch.zeros(H, DB))
        s.ln2 = nn.LayerNorm(D); s.f1 = nn.Linear(D, FF); s.f2 = nn.Linear(FF, D)

    def forward(s, x, dist, tmask):
        B, n, _ = x.shape; hd = D // H
        h = s.ln1(x)
        q = s.q(h).view(B, n, H, hd).transpose(1, 2); k = s.k(h).view(B, n, H, hd).transpose(1, 2); v = s.v(h).view(B, n, H, hd).transpose(1, 2)
        sc = q @ k.transpose(-1, -2) / math.sqrt(hd)
        sc = sc + s.dist_bias[:, dist].permute(1, 0, 2, 3)  # (B,H,n,n)
        sc = sc.masked_fill(~tmask[:, None, None, :], -1e9)
        a = torch.softmax(sc, -1) @ v
        x = x + F.dropout(s.o(a.transpose(1, 2).reshape(B, n, D)), DROP, s.training)
        return x + F.dropout(s.f2(F.gelu(s.f1(s.ln2(x)), approximate='tanh')), DROP, s.training)


class Policy(nn.Module):
    def __init__(s):
        super().__init__()
        s.inp = nn.Linear(TF, D); s.res = nn.Embedding(8, D); s.layers = nn.ModuleList([Layer() for _ in range(L)])
        s.lnf = nn.LayerNorm(D); s.m1 = nn.Linear(2 * D + MF, D); s.m2 = nn.Linear(D, 1)

    def forward(s, tok, dist, tmask, piece, nb, mf, mmask):
        x = s.inp(tok)
        for l in s.layers:
            x = l(x, dist, tmask)
        x = s.lnf(x)
        B, k = piece.shape
        on_board = piece >= 0
        idx = piece.clamp(min=0)
        hp = torch.gather(x, 1, idx[..., None].expand(B, k, D))
        hres = s.res((-piece - 1).clamp(0, 7))
        hp = torch.where(on_board[..., None], hp, hres)
        nbm = nb >= 0
        nbx = torch.gather(x, 1, nb.clamp(min=0).view(B, k * NB, 1).expand(B, k * NB, D)).view(B, k, NB, D)
        cnt = nbm.sum(-1, keepdim=True).clamp(min=1)
        hn = (nbx * nbm[..., None]).sum(2) / cnt
        z = s.m2(F.gelu(s.m1(torch.cat([hp, hn, mf], -1)), approximate='tanh')).squeeze(-1)
        return z.masked_fill(~mmask, -1e9)


DEV[0] = dev
model = Policy().to(dev)
opt = torch.optim.AdamW(model.parameters(), lr=1e-3, weight_decay=WD)
steps = EPOCHS * math.ceil(len(train) / BS)
sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=1e-3, total_steps=steps, pct_start=0.05)
print(f'params {sum(p.numel() for p in model.parameters())}, train {len(train)}, val {len(val)}', flush=True)


def evaluate():
    model.eval(); c1 = c3 = tot = 0; loss = 0.0
    with torch.no_grad():
        for i in range(0, len(val), 256):
            b = collate(val[i:i + 256])
            z = model(*b[:7]); y = b[7]
            loss += F.cross_entropy(z, y, reduction='sum').item()
            top = z.topk(min(3, z.shape[1]), -1).indices
            c1 += (top[:, 0] == y).sum().item(); c3 += (top == y[:, None]).any(-1).sum().item(); tot += len(y)
    model.train()
    return c1 / tot, c3 / tot, loss / tot


best_vl = 1e9; best_sd = None; best_ep = 0
for ep in range(EPOCHS):
    # bucket by size for efficient padding
    order = list(range(len(train))); random.shuffle(order)
    chunks = [order[i:i + BS * 50] for i in range(0, len(order), BS * 50)]
    batches = []
    for ch in chunks:
        ch.sort(key=lambda j: (train[j][2].shape[0], train[j][0].shape[0]))
        batches += [ch[i:i + BS] for i in range(0, len(ch), BS)]
    random.shuffle(batches)
    t0 = time.time(); tl = 0.0
    for bi in batches:
        b = collate([train[j] for j in bi])
        z = model(*b[:7])
        loss = F.cross_entropy(z, b[7], label_smoothing=LS)
        opt.zero_grad(); loss.backward(); nn.utils.clip_grad_norm_(model.parameters(), 1.0); opt.step(); sched.step()
        tl += loss.item()
    a1, a3, vl = evaluate()
    if ep == 0 or vl < best_vl:
        best_vl = vl; best_sd = copy.deepcopy(model.state_dict()); best_ep = ep + 1
    print(f'epoch {ep+1}: train loss {tl/len(batches):.3f}  val loss {vl:.3f}  top1 {a1:.3f}  top3 {a3:.3f}  ({time.time()-t0:.0f}s)', flush=True)

model.load_state_dict(best_sd); print('keeping epoch', best_ep, 'val loss %.3f' % best_vl, flush=True)
# export (input-major matrices = Linear.weight.T)
model = model.cpu(); DEV[0] = 'cpu'
def W(lin): return lin.weight.detach().t().contiguous().view(-1).tolist()
def V(t): return t.detach().contiguous().view(-1).tolist()
with open(out, 'w') as f:
    f.write(f'{D} {H} {L} {FF}\n')
    def put(name, vals):
        f.write(name + ' ' + str(len(vals)) + ' ' + ' '.join(f'{v:.6g}' for v in vals) + '\n')
    put('w_in', W(model.inp)); put('b_in', V(model.inp.bias)); put('res_emb', V(model.res.weight))
    for i, l in enumerate(model.layers):
        put(f'l{i}.ln1_g', V(l.ln1.weight)); put(f'l{i}.ln1_b', V(l.ln1.bias))
        put(f'l{i}.wq', W(l.q)); put(f'l{i}.bq', V(l.q.bias)); put(f'l{i}.wk', W(l.k)); put(f'l{i}.bk', V(l.k.bias))
        put(f'l{i}.wv', W(l.v)); put(f'l{i}.bv', V(l.v.bias)); put(f'l{i}.wo', W(l.o)); put(f'l{i}.bo', V(l.o.bias))
        put(f'l{i}.dist_bias', V(l.dist_bias)); put(f'l{i}.ln2_g', V(l.ln2.weight)); put(f'l{i}.ln2_b', V(l.ln2.bias))
        put(f'l{i}.f1', W(l.f1)); put(f'l{i}.f1b', V(l.f1.bias)); put(f'l{i}.f2', W(l.f2)); put(f'l{i}.f2b', V(l.f2.bias))
    put('lnf_g', V(model.lnf.weight)); put('lnf_b', V(model.lnf.bias))
    put('m1', W(model.m1)); put('m1b', V(model.m1.bias)); put('m2', V(model.m2.weight)); put('m2b', V(model.m2.bias))
# reference logits for the first validation position, to check the Rust forward pass
model.eval()
b = collate(data[:1])
with torch.no_grad():
    z = model(*[t.cpu() for t in b[:7]])[0]
print('REF', ' '.join(f'{v:.4f}' for v in z[:min(8, z.shape[0])].tolist()))
print('saved', out)
