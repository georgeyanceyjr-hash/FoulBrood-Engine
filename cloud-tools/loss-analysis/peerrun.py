import json,sys,os
sys.argv=[sys.argv[0]]+sys.argv[1:]
exec(open('calrun.py').read().split("depth=int(sys.argv[1])")[0])
depth=int(sys.argv[1]);name=sys.argv[2];S=json.load(open('peerset.json'))
env=dict(os.environ,TR_BOOK='none',TR_CLIMB='1',TR_NET=sys.argv[3])
with ThreadPoolExecutor(4) as ex:res=list(ex.map(lambda c:probe(c['moves'],depth,env),S))
json.dump(res,open('peer-%s-d%d.json'%(name,depth),'w'))
B=[(-101,-40),(-40,-15),(-15,15),(15,40),(40,70),(70,101)]
print(name,'depth',depth,'- positions from Antsy v Formi games, moves 13-26, rating for the side to move:')
for lo,hi in B:
    xs=[s['won'] for s,r in zip(S,res) if r is not None and lo<=r<hi]
    if xs:print('   rated %+4d..%+4d: %3d positions, side to move won %3.0f%%'%(lo,hi-1,len(xs),100*sum(xs)/len(xs)))
