import sys; sys.path.insert(0,'tools')
from uhp import Engine
line=sys.argv[1]; eng=sys.argv[2] if len(sys.argv)>2 else './target/release/propolis'; arg=sys.argv[3] if len(sys.argv)>3 else 'depth 6'
g=line.split(';'); gt=g[0]; mv=g[1:]
A=Engine(eng)
for k in range(2, len(mv)):
    gs=gt+';InProgress;White[1];'+';'.join(mv[:k])
    A.cmd('newgame '+gs)
    a=A.cmd('bestmove '+arg)
    err=[x for x in A.p.stderr.readline() and [] ]
    print(k, 'played', mv[k], '| best', a[-1], flush=True)
