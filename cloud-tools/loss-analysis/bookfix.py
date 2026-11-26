import subprocess,sys,os,collections
ENG='/home/claude/propolis/target/release/thoraxrex'
SRC,DST=sys.argv[1],sys.argv[2]
lines=[l.rstrip('\n') for l in open(SRC)]
book=collections.OrderedDict()
for l in lines:
    p=l.split(' ',2)
    if len(p)==3: book.setdefault(p[0],[]).append((int(p[1]),p[2]))
class E:
    def __init__(s):
        s.p=subprocess.Popen([ENG],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=dict(os.environ,TR_BOOK='none'))
        s.read()
    def read(s):
        out=[]
        while True:
            l=s.p.stdout.readline()
            if not l or l.strip()=='ok':break
            out.append(l.rstrip())
        return out
    def cmd(s,c):
        s.p.stdin.write(c+'\n');s.p.stdin.flush();return s.read()
    def setup(s,moves):
        s.cmd('newgame Base+MLP')
        for m in moves: s.cmd('play '+m)
    def keys(s): return [s.cmd('key')[0]]+[s.cmd('symkey %d'%t)[0] for t in range(1,12)]
e=E()
def fix(pos,ban):
    mv=pos.split(';') if pos else []
    e.setup(mv); ks=e.keys()
    legal=e.cmd('validmoves')[0].split(';')
    tm={t:{(m if t==0 else e.cmd('symmove %d %s'%(t,m))[0]):m for m in legal} for t in range(12)}
    # class of each legal move = set of keys after playing it
    cls={}
    for m in legal:
        e.setup(mv+[m]); cls[m]=frozenset(e.keys())
    banned={cls[b] for b in ban}
    agg=collections.Counter(); seen=set()
    for t,k in enumerate(ks):
        if k in seen or k not in book: continue
        seen.add(k)
        for w,s in book[k]:
            m=tm[t].get(s)
            if m is None: print('  unmapped',t,s); continue
            agg[cls[m]]+=w
    rep={}
    for m in legal: rep.setdefault(cls[m],m)
    keep=[(w,rep[c]) for c,w in agg.items() if c not in banned]
    drop=[(w,rep[c]) for c,w in agg.items() if c in banned]
    print('position',pos,'\n  kept',sorted(keep,reverse=True),'\n  dropped',drop)
    for k in seen: del book[k]
    if keep: book[ks[0]]=sorted(keep,reverse=True)
    return keep
fix('wP;bG1 -wP',['wA1 wP/'])
with open(DST,'w') as f:
    for k,es in book.items():
        for w,s in es: f.write('%s %d %s\n'%(k,w,s))
print(len(lines),'->',sum(len(v) for v in book.values()))
