import subprocess,sys,os,json
from concurrent.futures import ThreadPoolExecutor
ENG='/home/claude/propolis/target/release/thoraxrex'
def run(c,depth,env):
    p=subprocess.Popen([ENG],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=env)
    def rd():
        o=[]
        while True:
            l=p.stdout.readline()
            if not l or l.strip()=='ok':break
            o.append(l.rstrip())
        return o
    rd();p.stdin.write('newgame Base+MLP\n');p.stdin.flush();rd()
    for m in c['moves']:
        p.stdin.write('play '+m+'\n');p.stdin.flush();rd()
    p.stdin.write('bestmove depth %d\n'%depth);p.stdin.flush();mine=(rd() or [''])[-1].strip()
    # compare positions, not strings: play each move and compare the resulting position key
    def key(mv):
        p.stdin.write('play '+mv+'\n');p.stdin.flush();r=rd()
        if any(x.startswith(('err','invalid')) for x in r):return None
        p.stdin.write('key\n');p.stdin.flush();k=(rd() or [''])[0];p.stdin.write('undo\n');p.stdin.flush();rd();return k
    a,b=key(mine),key(c['played']);p.stdin.close();p.wait()
    return None if a is None or b is None else a==b
depth=int(sys.argv[1]);name=sys.argv[2];S=json.load(open('agreeset.json'))
env=dict(os.environ,TR_BOOK='none',TR_CLIMB='1',TR_NET=sys.argv[3])
with ThreadPoolExecutor(4) as ex:res=list(ex.map(lambda c:run(c,depth,env),S))
ok=[r for r in res if r is not None]
w=[r for c,r in zip(S,res) if r is not None and c['won']]
print('%s depth %d: picks the move Antsy/Formi played in %d of %d positions (%.0f%%); when the mover went on to win: %.0f%%'%(name,depth,sum(ok),len(ok),100*sum(ok)/len(ok),100*sum(w)/max(1,len(w))),flush=True)
