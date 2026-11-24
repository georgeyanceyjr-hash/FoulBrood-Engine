"""Book v6 = Formi lines that pass the engine check, laid over book v5.
usage: build_v6.py <formicheck.out> <v5 book> <out>
A Formi line is kept when the engine's score after the book move is no more than MARGIN below its own choice.
A position is dropped whole when its best Formi move fails. v5 entries for a position Formi covers (in any of
the 12 orientations) are replaced."""
import subprocess, sys, collections
E='/home/claude/propolis/target/release/thoraxrex'; MARGIN=100
chk, v5, out = sys.argv[1:4]
pos=collections.OrderedDict()
# the check output drops the move prefix: take it from the check input (same key and move)
prefixes={}
for l in open('/home/claude/fbook/check.in'):
    q=l.rstrip('\n').split('|'); prefixes[(q[0],q[2])]=q[5]
for l in open(chk):
    p=l.rstrip('\n').split('|')
    if len(p)<8: continue
    key,w,mv,n,val=p[0],int(p[1]),p[2],int(p[3]),float(p[4]); prefix=prefixes[(key,mv)]
    own,book=int(p[-3]),int(p[-1])
    pos.setdefault(key,dict(prefix=prefix,moves=[]))['moves'].append((w,mv,book-own,val,n))
keep={}; dropped_pos=0; dropped_alt=0
for k,d in pos.items():
    best=max(d['moves'],key=lambda m:m[0])
    if best[2] < -MARGIN: dropped_pos+=1; continue
    ms=[m for m in d['moves'] if m[2] >= -MARGIN]
    dropped_alt+=len(d['moves'])-len(ms)
    keep[k]=dict(prefix=d['prefix'],moves=ms)
# all 12 orientation keys of every kept position
cmds=[]
for k,d in keep.items():
    cmds.append('newgame Base+MLP')
    for m in d['prefix'].split(';'):
        if m: cmds.append('play '+m)
    for t in range(12): cmds.append('symkey %d'%t)
res=subprocess.run([E],input='\n'.join(cmds)+'\nexit\n',capture_output=True,text=True,env={'TR_BOOK':'none'}).stdout
blocks=[b.strip().split('\n')[-1] for b in res.split('\nok\n')[1:]]
it=iter(blocks); covered=set()
for k,d in keep.items():
    next(it)
    for m in d['prefix'].split(';'):
        if m: next(it)
    ks=[next(it) for _ in range(12)]
    assert ks[0]==k,(ks[0],k)
    covered.update(ks)
lines=[]; replaced=0; kept5=0
for l in open(v5):
    k=l.split(' ',1)[0]
    if k in covered: replaced+=1; continue
    lines.append(l.rstrip('\n')); kept5+=1
for k,d in keep.items():
    for w,mv,_,_,_ in d['moves']: lines.append('%s %d %s'%(k,w,mv))
open(out,'w').write('\n'.join(lines)+'\n')
print('formi positions %d kept %d (dropped %d; alt moves dropped %d); v5 lines kept %d replaced %d; v6 lines %d'%(len(pos),len(keep),dropped_pos,dropped_alt,kept5,replaced,len(lines)))
