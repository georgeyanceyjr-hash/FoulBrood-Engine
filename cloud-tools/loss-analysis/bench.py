# For each loss: at each of White's last several decisions, what does the engine think (score from White's side) and play?
import probe,sys
secs=int(sys.argv[1]);cfgs=[{},{'TR_QMAX':'2'}]
games={'antsy':(open('antsy.txt').read().strip().split(';'),range(22,36,2)),'formi2':(open('formi2.txt').read().strip().split(';'),range(24,36,2)),'formi3':(open('formi3.txt').read().strip().split(';'),range(26,40,2))}
for name,(m,plies) in games.items():
    for n in plies:
        row=[]
        for c in cfgs:
            for k in ['TR_QMAX']: probe.env.pop(k,None)
            probe.env.update(c)
            r=probe.probe(m,n,secs)
            s=r[0][0] if r and r[0] else str(r)
            d=dict(x.split('=',1) for x in s.split(';') if '=' in x)
            row.append('%s d%s %s'%(d.get('score'),d.get('depth'),d.get('move')))
        print(name,n,'played',m[n] if n<len(m) else '','| default:',row[0],'| qmax2:',row[1],flush=True)
