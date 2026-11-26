import subprocess,sys,os,json,math,random
from concurrent.futures import ThreadPoolExecutor
ENG='/home/claude/propolis/target/release/thoraxrex'
def idx(raw): return round(100*math.tanh(raw/300)) if abs(raw)<99000 else (100 if raw>0 else -100)
def probe(moves,cmd,env):
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
        p.stdin.write('play '+m+'\n');p.stdin.flush();rd()
    p.stdin.write(cmd+'\n');p.stdin.flush();rd();p.stdin.close();err=p.stderr.read();p.wait()
    fin=[l for l in err.splitlines() if 'kind=final' in l]
    if not fin:return None
    d=dict(x.split('=',1) for x in fin[-1].split(';')[1:])
    return idx(int(d['score'])),int(d['depth'])
C=[c for c in json.load(open('calset.json')) if c['kind']=='loss']
random.seed(7);S=random.sample(C,16)
env=dict(os.environ,TR_BOOK='none',TR_CLIMB='1',TR_NET='/home/claude/foulbrood-engine/tools/net_q19.txt')
with ThreadPoolExecutor(2) as ex:
    shallow=list(ex.map(lambda c:probe(c['moves'],'bestmove depth 6',env),S))
    deep=list(ex.map(lambda c:probe(c['moves'],'bestmove time 00:00:30',env),S))
json.dump([S,shallow,deep],open('deep.json','w'))
ok=[(a,b) for a,b in zip(shallow,deep) if a and b]
print('n=%d  depth 6: mean %+.0f   30 s (mean depth %.1f): mean %+.0f'%(len(ok),sum(a[0] for a,_ in ok)/len(ok),sum(b[1] for _,b in ok)/len(ok),sum(b[0] for _,b in ok)/len(ok)),flush=True)
