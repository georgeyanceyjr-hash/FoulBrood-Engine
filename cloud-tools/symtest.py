import json, subprocess, sys, random
E='/home/claude/propolis/target/release/thoraxrex'
def run(cmds):
    out=subprocess.run([E],input='\n'.join(cmds)+'\nexit\n',capture_output=True,text=True,env={'TR_BOOK':'none'}).stdout
    return [b.strip().split('\n')[-1] if b.strip() else '' for b in out.split('\nok\n')[1:]]
d=json.load(open('/mnt/user-data/uploads/georgeyancey/Documents/Codex/FoulBroodData/hivegame-bot/opening-reference.json'))
lines=[r[0].split(';')+[r[1][0][0]] for us in '01' for r in d[us]['recs'] if r[0]]
random.seed(1); lines=random.sample(lines,60)
bad=0;n=0
for L in lines:
    for t in range(12):
        c=['newgame Base+MLP']
        for m in L: c+=['symkey %d'%t,'symmove %d %s'%(t,m),'play '+m]
        r=run(c)[1:]
        skeys=r[0::3]; smoves=r[1::3]
        c2=['newgame Base+MLP']
        for m in smoves: c2+=['key','play '+m]
        r2=run(c2)[1:]
        keys=r2[0::2]; plays=r2[1::2]
        n+=1
        if skeys!=keys or any('err' in x.lower() or 'invalid' in x.lower() for x in plays):
            bad+=1
            if bad<4: print('BAD',t,L,smoves,skeys[:3],keys[:3],plays[-2:])
print(n,'checks',bad,'bad')
