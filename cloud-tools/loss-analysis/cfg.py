import probe,os,sys
m=open('antsy.txt').read().strip().split(';')
for env in [{},{'TR_QMAX':'2'},{'TR_QMAX':'4'},{'TR_QMAX':'6'},{'TR_EXT4':'1','TR_QMAX':'4'},{'TR_LMP':'0','TR_QMAX':'4'},{'TR_ROOTLMR':'0'}]:
    probe.env.update({k:'' for k in ['TR_QMAX','TR_EXT4','TR_LMP','TR_ROOTLMR']})
    for k in ['TR_QMAX','TR_EXT4','TR_LMP','TR_ROOTLMR']: probe.env.pop(k,None)
    probe.env.update(env)
    print(env,'B@33:',probe.probe(m,33,int(sys.argv[1])),flush=True)
