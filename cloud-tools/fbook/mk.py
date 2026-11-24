import json, subprocess, math
E='/home/claude/propolis/target/release/thoraxrex'
d=json.load(open('/mnt/user-data/uploads/georgeyancey/Documents/Codex/FoulBroodData/hivegame-bot/opening-reference.json'))
recs=[(us,r[0],r[1]) for us in '01' for r in d[us]['recs']]
cmds=[]
for us,pre,c in recs:
    cmds.append('newgame Base+MLP')
    for m in pre.split(';'):
        if m: cmds.append('play '+m)
    cmds.append('key')
out=subprocess.run([E],input='\n'.join(cmds)+'\nexit\n',capture_output=True,text=True,env={'TR_BOOK':'none'}).stdout
blocks=[b.strip() for b in out.split('\nok\n')[1:]]
it=iter(blocks); n=0; seen=set(); bad=0
with open('fbook/check.in','w') as f:
    for us,pre,c in recs:
        next(it)
        ok=True
        for m in pre.split(';'):
            if m:
                b=next(it)
                if 'err' in b.lower() or 'invalid' in b.lower(): ok=False
        k=next(it).split('\n')[-1]
        if not ok: bad+=1; continue
        if k in seen: continue
        seen.add(k)
        best=c[0][1]
        for mv,v,sup in c:
            w=max(1,round(1000*math.exp(-(best-v)/0.02)))
            f.write('%s|%d|%s|%d|%.3f|%s\n'%(k,w,mv,sup,v,pre)); n+=1
print(len(recs),'positions',n,'lines',bad,'bad',len(seen),'keys')
