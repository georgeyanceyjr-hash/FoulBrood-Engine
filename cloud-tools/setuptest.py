import subprocess, json, random, sys
E='/home/claude/propolis/target/release/thoraxrex'
D={'E':(1,0),'NE':(1,-1),'NW':(0,-1),'W':(-1,0),'SW':(-1,1),'SE':(0,1)}
def dest(mv,pos):
    parts=mv.split()
    if len(parts)==1: return (0,0)
    t=parts[1]
    if t[0]=='-': d,ref='W',t[1:]
    elif t[0]=='/': d,ref='SW',t[1:]
    elif t[0]=='\\': d,ref='NW',t[1:]
    elif t[-1]=='-': d,ref='E',t[:-1]
    elif t[-1]=='/': d,ref='NE',t[:-1]
    elif t[-1]=='\\': d,ref='SE',t[:-1]
    else: d,ref=None,t
    q,r=pos[ref]
    return (q,r) if d is None else (q+D[d][0],r+D[d][1])
def run(cmds):
    out=subprocess.run([E],input='\n'.join(cmds)+'\nexit\n',capture_output=True,text=True,env={'TR_BOOK':'none'}).stdout
    return [b.strip() for b in out.split('\nok\n')[1:]]
games=[]
for l in open('/home/claude/loss/losses.txt'):
    l=l.strip()
    if ';' in l:
        ms=[m for m in l.split('|')[-1].split(';') if m]
        if len(ms)>10: games.append(ms)
d=json.load(open('/mnt/user-data/uploads/georgeyancey/Documents/Codex/FoulBroodData/hivegame-bot/opening-reference.json'))
random.seed(3)
games+= [r[0].split(';') for r in random.sample(d['1']['recs'],25) if r[0]]
print(len(games),'games')
bad=0;n=0
for g in games[:40]:
    pos={};stack={};  # piece->(q,r); (q,r)->list
    cmdsA=['newgame Base+MLP']; roots=[]
    last=''
    for i,m in enumerate(g):
        if m=='pass': 
            last=''
        else:
            piece=m.split()[0]; to=dest(m,pos)
            if piece in pos:
                frm=pos[piece]; stack[frm].pop(); last='%s:%d:%d'%(piece,frm[0],frm[1])
            else: last=''
            pos[piece]=to; stack.setdefault(to,[]).append(piece)
        cmdsA+=['play '+m,'validmoves','key','evaluate']
        ply=i+1; side=ply%2; turn=ply//2+1
        enc=','.join('%s:%d:%d:%d'%(pc,q,r,lv) for (q,r),st in stack.items() for lv,pc in enumerate(st))
        roots.append('Base+MLP~%d~%d~%s~%s'%(side,turn,enc,last))
    ra=run(cmdsA)[1:]
    cmdsB=[]
    for rt in roots: cmdsB+=['newgame '+rt,'validmoves','key','evaluate']
    rb=run(cmdsB)
    for i in range(len(g)):
        if 'err' in ra[4*i].lower() or 'invalid' in ra[4*i].lower(): break
        a=(sorted(ra[4*i+1].split(';')),ra[4*i+3]); kA=ra[4*i+2]
        b=(sorted(rb[4*i+1].split(';')),rb[4*i+3]); gs=rb[4*i]
        n+=1
        if a!=b or not gs.startswith('Base+MLP~'):
            bad+=1
            if bad<4: print('BAD ply',i+1,g[i],'|',roots[i][:120],'|',gs[:80],'|',len(a[0]),len(b[0]),a[1],b[1], set(a[0])^set(b[0]))
print(n,'positions',bad,'bad')
