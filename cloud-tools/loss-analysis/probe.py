import subprocess,sys,os,json,time
ENG='/home/claude/propolis/target/release/thoraxrex'
env=dict(os.environ,TR_NET='/home/claude/foulbrood-engine/tools/net_q12.txt',TR_CLIMB='1')
def probe(moves,n,secs):
    p=subprocess.Popen([ENG],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)
    def cmd(c):
        p.stdin.write(c+'\n');p.stdin.flush();out=[]
        while True:
            l=p.stdout.readline()
            if not l or l.strip()=='ok':break
            out.append(l.rstrip())
        return out
    cmd('info');cmd('newgame Base+MLP')
    for m in moves[:n]:
        r=cmd('play '+m)
        if any(x.startswith(('err','invalid')) for x in r):return 'ERR '+m+str(r)
    t=time.time();r=cmd('bestmove time 00:00:%02d'%secs)
    p.stdin.close();err=p.stderr.read();p.wait()
    sc=[l for l in err.splitlines() if 'Score' in l or 'depth' in l.lower()]
    return sc[-1:], round(time.time()-t,1)
if __name__=='__main__':
    moves=sys.argv[1].split(';');secs=int(sys.argv[3])
    for n in map(int,sys.argv[2].split(',')):
        print(n,'played:',moves[n] if n<len(moves) else None,'->',probe(moves,n,secs),flush=True)
