import sys,json,math,collections
games=[json.loads(l) for l in open(sys.argv[1])]
agg=collections.defaultdict(lambda:[0,0,0.0])
for f in sys.argv[2:]:
    for l in open(f):
        gi,ply,tot,sc=map(int,l.split())
        res=games[gi]['result']  # 1 white won
        stm_white = ply%2==0
        s = sc if stm_white else -sc     # white view
        w = s if res==1 else -s          # winner view
        left=tot-ply
        loser_to_move = (stm_white)!=(res==1)
        k=(min(16,(left+1)//2*2), 'L' if loser_to_move else 'W')
        a=agg[k]; a[0]+=1; a[1]+= w>0; a[2]+= 1/(1+math.exp(-w/333))
for k in sorted(agg): 
    a=agg[k]; print(k, a[0], "sign-ok %.0f%%"%(100*a[1]/a[0]), "p(win) %.2f"%(a[2]/a[0]))
t=[sum(a[i] for a in agg.values()) for i in range(3)]
print("ALL",t[0],"%.1f%%"%(100*t[1]/t[0]),"%.3f"%(t[2]/t[0]))
