import subprocess,sys,os,json,math,time
ENG='/home/claude/propolis/target/release/thoraxrex'
def idx(raw): return round(100*math.tanh(raw/300)) if abs(raw)<99000 else (100 if raw>0 else -100)
def probe(moves,secs,env):
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
    p.stdin.write('bestmove time 00:00:%02d\n'%secs);p.stdin.flush();o=rd();p.stdin.close();err=p.stderr.read();p.wait()
    fin=[l for l in err.splitlines() if 'kind=final' in l]
    if not fin:return None
    d=dict(x.split('=',1) for x in fin[-1].split(';')[1:])
    return int(d['score']),int(d['depth']),d['move']
T=json.load(open('turnset.json'));secs=int(sys.argv[1]);name=sys.argv[2]
env=dict(os.environ,TR_BOOK='none',TR_CLIMB='1',TR_NET='/home/claude/foulbrood-engine/tools/net_q17.txt')
for kv in sys.argv[3:]:
    k,v=kv.split('=',1);env[k]=v
res=[]
for t in T:
    r=probe(t['moves'],secs,env)
    res.append(None if r is None else (idx(r[0]),r[1],r[2]))
turn=[(t,r) for t,r in zip(T,res) if t['kind']=='turn' and r]
ctl=[(t,r) for t,r in zip(T,res) if t['kind']=='control' and r]
print(name,'| turning points: mean score %+.0f (truth %+.0f), different move %d/%d, mean depth %.1f | controls: mean score %+.0f (n=%d)'%(
 sum(r[0] for _,r in turn)/len(turn),sum(t['then'] for t,_ in turn)/len(turn),sum(1 for t,r in turn if r[2]!=t['played']),len(turn),sum(r[1] for _,r in turn)/len(turn),sum(r[0] for _,r in ctl)/max(1,len(ctl)),len(ctl)),flush=True)
json.dump(res,open('turn-%s.json'%name,'w'))
