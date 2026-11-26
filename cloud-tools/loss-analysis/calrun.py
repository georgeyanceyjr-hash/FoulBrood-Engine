import subprocess,sys,os,json,math
from concurrent.futures import ThreadPoolExecutor
ENG='/home/claude/propolis/target/release/thoraxrex'
def idx(raw): return round(100*math.tanh(raw/300)) if abs(raw)<99000 else (100 if raw>0 else -100)
def probe(moves,depth,env):
    p=subprocess.Popen([ENG],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)
    def rd():
        o=[]
        while True:
            l=p.stdout.readline()
            if not l or l.strip()=='ok':break
            o.append(l.rstrip())
        return o
    rd();p.stdin.write('newgame Base+MLP\n');p.stdin.flush();rd()
    for m in moves:
        p.stdin.write('play '+m+'\n');p.stdin.flush();r=rd()
        if any(x.startswith(('err','invalid')) for x in r): p.kill();return None
    p.stdin.write('bestmove depth %d\n'%depth);p.stdin.flush();rd();p.stdin.close();err=p.stderr.read();p.wait()
    fin=[l for l in err.splitlines() if 'kind=final' in l]
    if not fin:return None
    d=dict(x.split('=',1) for x in fin[-1].split(';')[1:])
    return idx(int(d['score']))
depth=int(sys.argv[1]);name=sys.argv[2];C=json.load(open(sys.argv[4] if len(sys.argv)>4 else 'calset.json'))
env=dict(os.environ,TR_BOOK='none',TR_CLIMB='1',TR_NET=sys.argv[3])
with ThreadPoolExecutor(4) as ex:res=list(ex.map(lambda c:probe(c['moves'],depth,env),C))
def mean(k):
    xs=[r for c,r in zip(C,res) if c['kind']==k and r is not None];return sum(xs)/max(1,len(xs)),len(xs)
l,nl=mean('loss');w,nw=mean('win')
print('%s depth %d | positions from losses to Antsy/Formi: mean %+.0f (n=%d) | from wins: mean %+.0f (n=%d) | gap %.0f'%(name,depth,l,nl,w,nw,w-l),flush=True)
json.dump(res,open('cal-%s-d%d.json'%(name,depth),'w'))
