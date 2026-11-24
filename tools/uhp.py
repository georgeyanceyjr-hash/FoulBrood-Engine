import subprocess, os, itertools
_n=itertools.count()
class Engine:
    def __init__(self, path, args=()):
        self.path = path
        d = os.environ.get('MATCH_ERRDIR')
        self.errf = open(os.path.join(d, 'err-%d-%d-%s.txt' % (os.getpid(), next(_n), os.path.basename(os.path.dirname(path)) + '_' + os.path.basename(path))), 'w') if d else subprocess.DEVNULL
        self.p = subprocess.Popen([path, *args], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.errf, text=True, bufsize=1)
        self.read()
    def read(self):
        out = []
        while True:
            l = self.p.stdout.readline()
            if l == '':
                raise EOFError('engine died: ' + self.path + ' rc=' + str(self.p.poll()))
            l = l.strip()
            if l == 'ok':
                return out
            out.append(l)
    def cmd(self, c):
        self.p.stdin.write(c + '\n'); self.p.stdin.flush()
        return self.read()
    def close(self):
        try:
            self.p.stdin.write('exit\n'); self.p.stdin.flush()
        except Exception: pass
        self.p.kill()
