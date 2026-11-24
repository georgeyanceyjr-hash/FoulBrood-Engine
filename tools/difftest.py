import random, sys
sys.path.insert(0, 'tools')
from uhp import Engine
A = Engine('./target/release/propolis')
B = Engine('/home/claude/ref/hotfrass/target/release/hotfrass')
rng = random.Random(int(sys.argv[1]) if len(sys.argv) > 1 else 1)
games = int(sys.argv[2]) if len(sys.argv) > 2 else 50
types = ['Base+MLP', 'Base+MLP', 'Base', 'Base+M', 'Base+L', 'Base+P', 'Base+MP']
bad = 0; positions = 0
for g in range(games):
    gt = rng.choice(types)
    ga = A.cmd('newgame ' + gt)[0]; gb = B.cmd('newgame ' + gt)[0]
    for ply in range(rng.randint(10, 160)):
        ma = A.cmd('validmoves')[0]; mb = B.cmd('validmoves')[0]
        la = [m for m in ma.split(';') if m]; lb = [m for m in mb.split(';') if m]
        positions += 1
        if len(la) != len(lb):
            print('COUNT MISMATCH', gt, len(la), len(lb), A.cmd('newgame ' + gt)[0] if False else ga)
            bad += 1
        # every hotfrass move must be legal for propolis and every propolis move legal for hotfrass
        sample = lb if len(lb) <= 40 else rng.sample(lb, 40)
        for m in sample:
            r = A.cmd('play ' + m)[0]
            if r.startswith(('err', 'invalid')):
                print('A rejects', m, 'in', ga); bad += 1
            else:
                A.cmd('undo')
        sample = la if len(la) <= 40 else rng.sample(la, 40)
        for m in sample:
            r = B.cmd('play ' + m)[0]
            if r.startswith(('err', 'invalid')):
                print('B rejects', m, 'in', gb); bad += 1
            else:
                B.cmd('undo')
        if bad > 5: sys.exit(1)
        if not la: break
        m = rng.choice(la)
        ga = A.cmd('play ' + m)[0]; gb = B.cmd('play ' + m)[0]
        sa = ga.split(';')[1]; sb = gb.split(';')[1]
        if sa != sb:
            print('STATE MISMATCH', sa, sb, ga); bad += 1
        if sa != 'InProgress': break
print('positions', positions, 'bad', bad)
