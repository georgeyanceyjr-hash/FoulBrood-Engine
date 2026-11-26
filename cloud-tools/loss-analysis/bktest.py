import subprocess,os,sys,collections
ENG='/home/claude/propolis/target/release/thoraxrex'
def best(book,moves):
    p=subprocess.Popen([ENG],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=dict(os.environ,TR_BOOK=book,TR_NET='/home/claude/foulbrood-engine/tools/net_q17.txt'))
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
    p.stdin.write('bestmove time 00:00:01\n');p.stdin.flush();o=rd();p.stdin.close();p.wait();return o[-1]
for pos in ['wP;bG1 -wP','wP;bG1 wP-','wP;bG1 wP/','wP;bB1 wP\\']:
    c=collections.Counter(best(sys.argv[1],pos.split(';')) for _ in range(12)); print(pos,dict(c))
