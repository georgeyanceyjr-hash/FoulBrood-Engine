#!/usr/bin/env python3
# lsuite.py SECS [KEY=VAL env ...] : our lost games; our-turn positions 6..18 plies before the end.
# prints mean p(we win)=sigmoid(score/333) -- lower = sees the loss coming -- and mean score.
import sys,os,subprocess,math,concurrent.futures as cf
secs=float(sys.argv[1]); env=dict(os.environ, TR_BOOK='none', TR_POLICY='none'); env.setdefault('TR_NET','/home/claude/nn/net_q8.txt')
for a in sys.argv[2:]:
    k,v=a.split('=',1); env[k]=v
B='/home/claude/propolis/target/release/thoraxrex'
pos=[]
for l in open('/home/claude/loss/losses.txt'):
    gid,us,mv=l.rstrip('\n').split('|'); mv=mv.split(';'); N=len(mv); u=0 if us=='W' else 1
    for k in range(N-18,N-5):
        if k>=8 and k%2==u: pos.append((gid,N-k,k,mv[:k]))
def run(p):
    gid,left,k,mv=p
    c='White' if k%2==0 else 'Black'
    ms=int(secs*1000); t="00:00:%02d"%max(1,int(secs))
    inp="newgame Base+MLP;InProgress;%s[%d];%s\noptions set Threads 1\nbestmove time %s\nexit\n"%(c,k//2+1,';'.join(mv),t)
    out=subprocess.run([B],input=inp,capture_output=True,text=True,env=env)
    sc=None
    for ln in (out.stdout+out.stderr).splitlines():
        if ln.startswith('info depth'): sc=int(ln.split()[4])
        if ln.startswith('err') or 'invalid' in ln.lower(): return (gid,left,None)
    return (gid,left,sc)
with cf.ThreadPoolExecutor(2) as ex: res=list(ex.map(run,pos))
bad=[r for r in res if r[2] is None]; ok=[r for r in res if r[2] is not None]
import collections
by=collections.defaultdict(list)
for g,left,sc in ok: by['far' if left>=12 else 'near'].append(sc)
p=lambda xs: sum(1/(1+math.exp(-max(-20,min(20,x/333)))) for x in xs)/len(xs)
allx=[r[2] for r in ok]
print(' '.join(sys.argv[2:]) or 'base', '| n',len(ok),'bad',len(bad),'| p(we win) all %.3f far %.3f near %.3f | mean score %d | frac>0 %.2f'%(p(allx),p(by['far']),p(by['near']),sum(max(-2000,min(2000,x)) for x in allx)/len(allx), sum(x>0 for x in allx)/len(allx)))
if bad: print('bad games', sorted(set(r[0] for r in bad)))
