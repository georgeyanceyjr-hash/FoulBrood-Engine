"""Texel-style logistic tuning of ThoraxRex evaluation weights."""
import sys, numpy as np
from scipy.optimize import minimize
rows = [l.strip().split(';') for f in sys.argv[1:] if not f.startswith('--') for l in open(f)]
X = np.array([[float(x) for x in r[0].split(',')] for r in rows])
side = np.array([int(r[1]) for r in rows])
y = np.array([float(r[2]) for r in rows])
X[:, 51] = np.where(side == 0, 1.0, -1.0)  # tempo, white perspective
DEF = np.array([-20,-55,-110,-220,-450,14,6,12,8,16,12,10,8,0,0,0,0,0,0,0,0,80,-18,22,-10,3,12,3,6,-220,-130,-70,-35,-12,-10,-40,-25,-20,-20,-20,-25,-40,12,18,24,30,25,8,20,30,-12,6], float)
print('positions', len(y), 'white score', y.mean())
def loss(w, K):
    z = K * (X @ w) / 100.0
    p = 1 / (1 + np.exp(-np.clip(z, -30, 30)))
    return -np.mean(y * np.log(p + 1e-9) + (1 - y) * np.log(1 - p + 1e-9))
Ks = [0.05, 0.1, 0.2, 0.3, 0.5, 0.8, 1.2]
lk = [(loss(DEF, K), K) for K in Ks]
L0, K = min(lk)
print('default loss', round(L0, 5), 'K', K)
lam = float(next((a.split("=")[1] for a in sys.argv if a.startswith("--lam=")), "1e-6"))
def obj(w):
    return loss(w, K) + lam * np.sum((w - DEF) ** 2)
res = minimize(obj, DEF, method='L-BFGS-B', options={'maxiter': 500})
w = res.x
print('tuned loss', round(loss(w, K), 5))
names = ['d1','d2','d3','d4','d5','qfree','mS','mB','mG','mA','mM','mL','mP','hQ','hS','hB','hG','hA','hM','hL','hP','beetleQ','covered','guard','attPinned','escape','onboard','prox','spots','fr0','fr1','fr2','fr3','fr4','noQueen','beetleNear','queenStuck','na1','na2','na3','na4','na5','enemyPinned','pbRemove','pbEscape','pbPlace','strength','jumpers','ddrop','pbCover','truePin','tempo']
for n, a, b in zip(names, DEF, w): print(f'{n:10s} {a:7.0f} -> {b:8.1f}')
print(','.join(str(int(round(x))) for x in w))
