"""Eval-net trainer on the GPU (PyTorch: Apple MPS, CUDA, else CPU). Same inputs, network and
output file as train_qn.py, so nets it writes load in the engine unchanged:
  python3 train_qn_torch.py NF files out [H1 H2 LAM EPOCHS]
Rows from `thoraxrex dumpfeat`: NF own features, NF opponent features, CSVX extra cols,
search score (side to move, may be empty), result. Target = LAM*sigmoid(score/K)+(1-LAM)*result.
Env: WD weight decay, EXTRA extra inputs, CSVX, BATCH (4096), LR (2e-3).
Keeps the epoch with the best validation loss."""
import os, sys, time, numpy as np, torch, torch.nn as nn

NF = int(sys.argv[1]); files = sys.argv[2].split(','); out = sys.argv[3]
VAL = {f[4:] for f in files if f.startswith('val:')}; files = [f[4:] if f.startswith('val:') else f for f in files]
H1 = int(sys.argv[4]) if len(sys.argv) > 4 else 32; H2 = int(sys.argv[5]) if len(sys.argv) > 5 else 32
LAM = float(sys.argv[6]) if len(sys.argv) > 6 else 0.5; EPOCHS = int(sys.argv[7]) if len(sys.argv) > 7 else 20
K = 333.0
WD = float(os.environ.get('WD', '0')); EXTRA = int(os.environ.get('EXTRA', '0')); CSVX = int(os.environ.get('CSVX', '288'))
B = int(os.environ.get('BATCH', '4096')); LR = float(os.environ.get('LR', '2e-3'))
dev = 'mps' if torch.backends.mps.is_available() else ('cuda' if torch.cuda.is_available() else 'cpu')
ncols = 2 * (NF - 1) + EXTRA
t0 = time.time()

# Rows are written straight into one preallocated float32 array (4M rows x 408 inputs is about 6.6 GB).
counts = [sum(1 for _ in open(f)) for f in files]; total = sum(counts)
X = np.empty((total, ncols), np.float32); Y = np.empty(total, np.float32); isval = np.zeros(total, bool)
r = 0
for f, n in zip(files, counts):
    isval[r:r + n] = f in VAL
    for line in open(f):
        p = line.rstrip('\n').split(',')
        X[r, :NF - 1] = p[:NF - 1]; X[r, NF - 1:2 * (NF - 1)] = p[NF:2 * NF - 1]
        if EXTRA: X[r, 2 * (NF - 1):] = p[2 * NF:2 * NF + EXTRA]
        sc, res = p[2 * NF + CSVX], float(p[2 * NF + CSVX + 1])
        Y[r] = LAM / (1 + np.exp(-float(sc) / K)) + (1 - LAM) * res if sc != '' else res
        r += 1
rng = np.random.default_rng(1)
if not isval.any(): isval[rng.permutation(total)[:total // 10]] = True
vi = np.where(isval)[0]; ti = np.where(~isval)[0]
S1 = np.zeros(ncols); S2 = np.zeros(ncols)  # training-row mean and std, in chunks (no 6 GB copy)
for s0 in range(0, total, 200000):
    c = X[s0:s0 + 200000][~isval[s0:s0 + 200000]].astype(np.float64); S1 += c.sum(0); S2 += (c * c).sum(0)
mean = (S1 / len(ti)).astype(np.float32); std = (np.sqrt(np.maximum(S2 / len(ti) - (S1 / len(ti)) ** 2, 0)) + 1e-6).astype(np.float32)
X -= mean; X /= std  # in place: only one copy on the CPU side
Xt = torch.from_numpy(X).to(dev); Yt = torch.from_numpy(Y).to(dev)
if dev != 'cpu': del X
tit = torch.from_numpy(ti).to(dev); vit = torch.from_numpy(vi).to(dev)
print(f'loaded {len(X)} rows, {ncols} inputs in {time.time()-t0:.0f}s, device {dev}', flush=True)

torch.manual_seed(1)
class Net(nn.Module):
    def __init__(s):
        super().__init__(); s.l1 = nn.Linear(ncols, H1); s.l2 = nn.Linear(H1, H2); s.l3 = nn.Linear(H2, 1)
        for l in (s.l1, s.l2): nn.init.kaiming_normal_(l.weight, nonlinearity='relu'); nn.init.zeros_(l.bias)
        nn.init.normal_(s.l3.weight, std=0.1); nn.init.zeros_(s.l3.bias)
    def forward(s, x):  # clipped ReLU 0..8, as in the engine
        return s.l3(torch.clamp(s.l2(torch.clamp(s.l1(x), 0, 8)), 0, 8)).squeeze(-1)
net = Net().to(dev)
opt = torch.optim.AdamW(net.parameters(), lr=LR, weight_decay=WD)
sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=LR, total_steps=EPOCHS * ((len(ti) + B - 1) // B))

def val_loss(ix):
    net.eval(); tot = 0.0
    with torch.no_grad():
        for s in range(0, len(ix), 65536):
            b = ix[s:s + 65536]; tot += float(((torch.sigmoid(net(Xt[b])) - Yt[b]) ** 2).sum())
    net.train(); return tot / len(ix)

best = (val_loss(vit), None, -1); print('val loss init', round(best[0], 5), flush=True)
for ep in range(EPOCHS):
    perm = tit[torch.randperm(len(tit), device=dev)]
    for s in range(0, len(perm), B):
        b = perm[s:s + B]
        loss = ((torch.sigmoid(net(Xt[b])) - Yt[b]) ** 2).mean()
        opt.zero_grad(set_to_none=True); loss.backward(); opt.step(); sched.step()
    v = val_loss(vit)
    print('epoch', ep, 'train', round(val_loss(tit[:200000]), 5), 'val', round(v, 5), f'{time.time()-t0:.0f}s', flush=True)
    if v < best[0]: best = (v, {k: t.detach().cpu().clone() for k, t in net.state_dict().items()}, ep)
if best[1] is not None: net.load_state_dict(best[1])
print('best epoch', best[2], 'val', round(best[0], 5), flush=True)
sd = {k: t.detach().cpu().numpy().astype(np.float32) for k, t in net.state_dict().items()}
with open(out, 'w') as f:
    f.write(f'{ncols} {H1} {H2}\n')
    for arr in (mean, std, sd['l1.weight'].ravel(), sd['l1.bias'], sd['l2.weight'].ravel(), sd['l2.bias'],
                sd['l3.weight'].ravel(), sd['l3.bias']):
        f.write(' '.join(f'{v:.6g}' for v in arr) + '\n')
    f.write(f'{K}\n')
print('wrote', out)
