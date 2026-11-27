import subprocess,sys,os,json
ENG='/home/claude/propolis/target/release/thoraxrex'
S=json.load(open(sys.argv[1]));env=dict(os.environ,TR_BOOK='none',TR_POLICY=sys.argv[2])
ok=n=0;ps=[]
for c in S:
    p=subprocess.Popen([ENG],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=env)
    def rd():
        o=[]
        while True:
            l=p.stdout.readline()
            if not l or l.strip()=='ok':break
            o.append(l.rstrip())
        return o
    def cmd(x):
        p.stdin.write(x+'\n');p.stdin.flush();return rd()
    rd();cmd('newgame Base+MLP')
    for m in c['moves']:cmd('play '+m)
    r=cmd('policy')
    if not r or r[0].startswith('err'):p.kill();continue
    top=r[0].rsplit(' ',1)[0];ps.append(float(r[0].rsplit(' ',1)[1]))
    def key(mv):
        r=cmd('play '+mv)
        if any(x.startswith(('err','invalid')) for x in r):return None
        k=cmd('key')[0];cmd('undo');return k
    a,b=key(top),key(c['played']);p.stdin.close();p.wait()
    if a and b:n+=1;ok+=a==b
print(sys.argv[1],'raw policy top-1: %d of %d (%.0f%%), mean top prob %.2f'%(ok,n,100*ok/n,sum(ps)/len(ps)))
