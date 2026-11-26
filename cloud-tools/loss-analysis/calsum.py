import json,sys,glob
C=json.load(open('calset.json'))
for f in sorted(glob.glob('cal-*.json')):
    R=json.load(open(f))
    def m(k,day=None):
        xs=[r for c,r in zip(C,R) if c['kind']==k and r is not None and (day is None or c['day']==day)];return (sum(xs)/len(xs),len(xs)) if xs else (0,0)
    l,nl=m('loss');w,nw=m('win')
    print('%-16s losses to Antsy/Formi %+4.0f (n=%d)   wins %+4.0f (n=%d)   gap %3.0f'%(f[4:-5],l,nl,w,nw,w-l))
ig=lambda k:sum(c['was'] for c in C if c['kind']==k)/sum(c['kind']==k for c in C)
print('%-16s losses to Antsy/Formi %+4.0f           wins %+4.0f           gap %3.0f   (what the bot thought during the games)'%('in-game',ig('loss'),ig('win'),ig('win')-ig('loss')))
