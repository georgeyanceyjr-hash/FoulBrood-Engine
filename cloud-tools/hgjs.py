import json,sys
s=json.loads(sys.stdin.read())
s=s.replace("const s=1/0.78125; const X=x*s, Y=y*s;","const s=innerWidth/800; const X=x*s, Y=y*s;")
open('/home/claude/hghelpers.js','w').write(s)
print(len(s))
