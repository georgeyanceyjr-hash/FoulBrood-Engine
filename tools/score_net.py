"""How well does a net's static evaluation predict who won, on positions it was not trained on?
Usage: score_net.py NF CSV net1.txt [net2.txt ...]   (CSV rows from `thoraxrex dumpfeat ... extra`; env CSVX as in train_qn.py)
Prints, per net: mean squared error against the result, how often the favoured side won, and a calibration table:
of the positions the net calls p(win) >= 90 %, 80-90 %, ... how many were actually won. An overconfident net shows
real win rates well under its own numbers in the top rows."""
import sys, os, numpy as np
NF = int(sys.argv[1]); csv = sys.argv[2]; nets = sys.argv[3:]
CSVX = int(os.environ.get('CSVX', '294'))
rows = [l.rstrip('\n').split(',') for l in open(csv)]
res = np.array([float(p[2 * NF + CSVX + 1]) for p in rows], np.float32)
base = np.array([[float(t) for t in p[:NF - 1]] + [float(t) for t in p[NF:2 * NF - 1]] for p in rows], np.float32)
extra = np.array([[float(t) for t in p[2 * NF:2 * NF + CSVX]] for p in rows], np.float32)
print('positions', len(rows), 'from', csv)
for path in nets:
    with open(path) as f:
        n_in, h1, h2 = map(int, f.readline().split())
        mean, std, w1, b1, w2, b2, w3, b3 = [np.array(f.readline().split(), np.float32) for _ in range(8)]
    x = base if n_in == base.shape[1] else np.hstack([base, extra[:, :n_in - base.shape[1]]])
    a1 = np.clip(((x - mean) / std) @ w1.reshape(h1, n_in).T + b1, 0, 8)
    a2 = np.clip(a1 @ w2.reshape(h2, h1).T + b2, 0, 8)
    p = 1 / (1 + np.exp(-(a2 @ w3 + b3[0])))
    dec = res != 0.5
    print('\n' + os.path.basename(path))
    print('  mse %.4f   favoured side won %.1f%% of decided positions' % (float(np.mean((p - res) ** 2)), 100 * float(np.mean((p[dec] > 0.5) == (res[dec] > 0.5)))))
    for lo, hi in [(0.9, 1.01), (0.8, 0.9), (0.7, 0.8), (0.6, 0.7), (0.5, 0.6)]:
        m = ((p >= lo) & (p < hi)) | ((1 - p >= lo) & (1 - p < hi))
        if m.sum():
            won = np.where(p[m] >= 0.5, res[m], 1 - res[m])
            conf = np.where(p[m] >= 0.5, p[m], 1 - p[m])
            print('  net says %2d-%3d%%: %6d positions, net average %.1f%%, actually won %.1f%%' % (lo * 100, min(hi, 1) * 100, int(m.sum()), 100 * float(conf.mean()), 100 * float(won.mean())))
