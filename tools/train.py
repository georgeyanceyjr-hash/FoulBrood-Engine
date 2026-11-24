"""Train a small MLP evaluation (numpy, Adam). Input rows from `thoraxrex dumpfeat`:
NF own features, NF opponent features, search score (stm, may be empty), result (stm).
Target = lam * sigmoid(score/K) + (1-lam) * result  (result only when no score)."""
import sys, numpy as np
NF = int(sys.argv[1]); files = sys.argv[2].split(','); out = sys.argv[3]
VAL = [f for f in files if f.startswith('val:')]; files = [f[4:] if f.startswith('val:') else f for f in files]
H1 = int(sys.argv[4]) if len(sys.argv) > 4 else 32; H2 = int(sys.argv[5]) if len(sys.argv) > 5 else 32
LAM = float(sys.argv[6]) if len(sys.argv) > 6 else 0.5; EPOCHS = int(sys.argv[7]) if len(sys.argv) > 7 else 20
K = 333.0
WD = float(__import__('os').environ.get('WD', '0'))
EXTRA = int(__import__('os').environ.get('EXTRA', '0'))
ncols = 2 * (NF - 1) + EXTRA
total = sum(sum(1 for _ in open(f)) for f in files)
X = np.zeros((total, ncols), np.float32); Y = np.zeros(total, np.float32); isval = np.zeros(total, bool)
r = 0
for f in files:
    v = ('val:' + f) in VAL
    for line in open(f):
        p = line.rstrip('\n').split(',')
        base = p[:2 * NF]
        X[r, :NF - 1] = [float(t) for t in base[:NF - 1]]
        X[r, NF - 1:2 * (NF - 1)] = [float(t) for t in base[NF:2 * NF - 1]]
        if EXTRA:
            X[r, 2 * (NF - 1):] = [float(t) for t in p[2 * NF:2 * NF + EXTRA]]
        sc, res = p[2 * NF + EXTRA], float(p[2 * NF + EXTRA + 1])
        Y[r] = LAM / (1 + np.exp(-float(sc) / K)) + (1 - LAM) * res if sc != '' else res
        isval[r] = v; r += 1
rng = np.random.default_rng(1)
if isval.any():
    vi = np.where(isval)[0]; ti = np.where(~isval)[0]
else:
    idx = rng.permutation(len(X)); nv = len(X) // 10; vi, ti = idx[:nv], idx[nv:]
mean = X[ti].mean(0); std = X[ti].std(0) + 1e-6
Xn = (X - mean) / std
n_in = X.shape[1]
def init(a, b): return (rng.standard_normal((a, b)) * np.sqrt(2 / b)).astype(np.float32)
P = {'w1': init(H1, n_in), 'b1': np.zeros(H1, np.float32), 'w2': init(H2, H1), 'b2': np.zeros(H2, np.float32),
     'w3': (rng.standard_normal(H2) * 0.1).astype(np.float32), 'b3': np.zeros(1, np.float32)}
M = {k: np.zeros_like(v) for k, v in P.items()}; V = {k: np.zeros_like(v) for k, v in P.items()}
def fwd(x):
    z1 = x @ P['w1'].T + P['b1']; a1 = np.clip(z1, 0, 8)
    z2 = a1 @ P['w2'].T + P['b2']; a2 = np.clip(z2, 0, 8)
    o = a2 @ P['w3'] + P['b3'][0]
    return z1, a1, z2, a2, o
def loss(x, y):
    o = fwd(x)[4]; p = 1 / (1 + np.exp(-o)); return float(np.mean((p - y) ** 2))
lr = 1e-3; step = 0; B = 512
print('rows', len(X), 'inputs', n_in, 'val loss init', loss(Xn[vi], Y[vi]), flush=True)
for ep in range(EPOCHS):
    perm = rng.permutation(ti)
    for s in range(0, len(perm), B):
        b = perm[s:s + B]; x = Xn[b]; y = Y[b]
        z1, a1, z2, a2, o = fwd(x); p = 1 / (1 + np.exp(-o))
        g = 2 * (p - y) * p * (1 - p) / len(b)
        G = {'w3': a2.T @ g, 'b3': np.array([g.sum()], np.float32)}
        d2 = np.outer(g, P['w3']) * ((z2 > 0) & (z2 < 8))
        G['w2'] = d2.T @ a1; G['b2'] = d2.sum(0)
        d1 = (d2 @ P['w2']) * ((z1 > 0) & (z1 < 8))
        G['w1'] = d1.T @ x; G['b1'] = d1.sum(0)
        step += 1
        for k in P:
            M[k] = 0.9 * M[k] + 0.1 * G[k]; V[k] = 0.999 * V[k] + 0.001 * G[k] ** 2
            mh = M[k] / (1 - 0.9 ** step); vh = V[k] / (1 - 0.999 ** step)
            P[k] -= (lr * mh / (np.sqrt(vh) + 1e-8)).astype(np.float32)
            if k[0] == 'w' and WD: P[k] -= (lr * WD * P[k]).astype(np.float32)
    print('epoch', ep, 'train', round(loss(Xn[ti[:50000]], Y[ti[:50000]]), 5), 'val', round(loss(Xn[vi], Y[vi]), 5), flush=True)
    if ep == EPOCHS * 2 // 3: lr *= 0.3
with open(out, 'w') as f:
    f.write(f'{n_in} {H1} {H2}\n')
    for arr in (mean, std, P['w1'].ravel(), P['b1'], P['w2'].ravel(), P['b2'], P['w3'], P['b3']):
        f.write(' '.join(f'{v:.6g}' for v in arr) + '\n')
    f.write(f'{K}\n')
print('wrote', out)
