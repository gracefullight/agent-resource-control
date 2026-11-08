import os, socket, subprocess, tempfile, time, signal, pty, struct, select, json, ctypes, threading, traceback, hashlib
from pathlib import Path
BASE=Path.cwd(); OUT=BASE/'target/task7-review-ctrlz.json'; LOG=BASE/'target/task7-review-ctrlz.log'
libc=ctypes.CDLL(None,use_errno=True)
class Bsd(ctypes.Structure):
    _fields_=[('head',ctypes.c_uint32*12),('comm',ctypes.c_char*16),('name',ctypes.c_char*32),('tail',ctypes.c_uint32*6),('sec',ctypes.c_uint64),('usec',ctypes.c_uint64)]
libc.proc_pidinfo.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_uint64,ctypes.c_void_p,ctypes.c_int]
libc.proc_pidinfo.restype=ctypes.c_int
class Timeval(ctypes.Structure):
    _fields_=[('sec',ctypes.c_long),('usec',ctypes.c_int)]
class Prefix(ctypes.Structure):
    _fields_=[('start',Timeval),('vm',ctypes.c_void_p),('sa',ctypes.c_void_p),('flags',ctypes.c_int),('status',ctypes.c_char),('pid',ctypes.c_int)]
def native(pid):
    info=Bsd(); n=libc.proc_pidinfo(pid,3,0,ctypes.byref(info),ctypes.sizeof(info))
    if n==ctypes.sizeof(info):return {'pid':pid,'start_token':str(info.sec*1000000+info.usec),'kind':'live','pgid':info.tail[1]}
    mib=(ctypes.c_int*4)(1,14,1,pid);buf=ctypes.create_string_buffer(4096);length=ctypes.c_size_t(len(buf))
    rc=libc.sysctl(mib,4,buf,ctypes.byref(length),None,0)
    if rc==0 and length.value==0:return None
    if rc==0 and length.value>=ctypes.sizeof(Prefix):
        p=Prefix.from_buffer_copy(buf)
        if p.pid==pid:return {'pid':pid,'start_token':str(p.start.sec*1000000+p.start.usec),'kind':'retained'}
    return {'pid':pid,'kind':'indeterminate','errno':ctypes.get_errno()}
result={'probe':'native terminal VSUSP/Ctrl-Z','started':time.time(),'scriptSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'identities':{},'events':[],'pass':False}
def save():
    with OUT.open('w') as f:json.dump(result,f,indent=2);f.flush();os.fsync(f.fileno())
def record(name,pid):
    item=native(pid);assert item and item['kind']=='live',(name,item);result['identities'][name]=item;save()
def event(s):result['events'].append(s);save();print(s,flush=True)
def owned_signal(name,sig):
    prior=result['identities'].get(name)
    if not prior:return
    current=native(prior['pid'])
    if current and current.get('start_token')==prior['start_token']:os.kill(prior['pid'],sig)
def fixture_inventory():
    proc=ctypes.CDLL('/usr/lib/libproc.dylib',use_errno=True);proc.proc_listallpids.argtypes=[ctypes.c_void_p,ctypes.c_int]
    proc.proc_pidpath.argtypes=[ctypes.c_int,ctypes.c_void_p,ctypes.c_uint32]
    pids=(ctypes.c_int*16384)();count=proc.proc_listallpids(pids,ctypes.sizeof(pids));found=[];unreadable=0
    target=str((BASE/'target/debug/arc-exec-fixture').resolve()).encode()
    for pid in list(pids)[:max(0,count)]:
        if pid<=0:continue
        buf=ctypes.create_string_buffer(4096);n=proc.proc_pidpath(pid,buf,len(buf))
        if n<=0:unreadable+=1
        elif buf.value==target:found.append(native(pid))
    return {'enumerated':count,'exact_workspace_fixture_matches':found,'unreadable_paths':unreadable}
result['prior_probe_fixture_inventory']=fixture_inventory();save()
daemon=supervisor=None;master=slave=None;reader=None;reader_stop=threading.Event();captured=bytearray();conn=listener=None
with tempfile.TemporaryDirectory(prefix='az-review-',dir='/tmp') as tmp, LOG.open('wb') as log:
    root=Path(tmp);result['isolatedRoot']=str(root);save()
    env=dict(os.environ,ARC_DB_PATH=str(root/'db'),ARC_SOCKET_PATH=str(root/'daemon.sock'));env.pop('ARC_RUNTIME_REPORT',None)
    try:
        daemon=subprocess.Popen([str(BASE/'target/debug/arc-daemon')],env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=log)
        record('daemon',daemon.pid)
        assert select.select([daemon.stdout],[],[],8)[0],'daemon readiness timeout'
        assert b'arc-daemon ready.' in daemon.stdout.readline();event('isolated daemon ready')
        listener=socket.socket(socket.AF_UNIX);control=str(root/'child.sock');listener.bind(control);listener.listen(1);listener.settimeout(8)
        master,slave=pty.openpty();os.set_blocking(master,False)
        def drain():
            while not reader_stop.is_set():
                try:
                    data=os.read(master,4096)
                    if data:captured.extend(data);log.write(data);log.flush()
                except (BlockingIOError,OSError):pass
                time.sleep(.001)
            for _ in range(20):
                try:
                    data=os.read(master,4096)
                    if data:captured.extend(data);log.write(data);log.flush()
                except (BlockingIOError,OSError):pass
                time.sleep(.001)
        reader=threading.Thread(target=drain,daemon=True);reader.start()
        restored=root/'restored';childenv=dict(env,ARC_FIXTURE_LAUNCH='tty',ARC_FIXTURE_CONTROL=control,ARC_FIXTURE_RESTORED=str(restored),PRIVATE_ENV='qa-native')
        supervisor=subprocess.Popen([str(BASE/'target/debug/arc-exec-fixture'),str(BASE/'target/debug/arcctl'),'exec','--',str(BASE/'target/debug/arc-exec-fixture')],env=childenv,stdin=slave,stdout=slave,stderr=slave,close_fds=True)
        record('supervisor',supervisor.pid)
        conn,_=listener.accept();conn.settimeout(8)
        def exact(n):
            data=b''
            while len(data)<n:
                chunk=conn.recv(n-len(data));assert chunk,'fixture EOF';data+=chunk
            return data
        msg=json.loads(exact(struct.unpack('>I',exact(4))[0]));record('child',msg['pid']);record('wrapper',msg['ppid'])
        assert msg['tty'] and msg['foreground'] and msg['pgid']==msg['pid'];event('native tuples persisted before VSUSP')
        conn.sendall(b'x');os.write(master,b'\x1a');event('terminal VSUSP byte sent')
        deadline=time.monotonic()+8;stopped=False
        while time.monotonic()<deadline:
            pid,status=os.waitpid(supervisor.pid,os.WUNTRACED|os.WNOHANG)
            if pid:
                assert os.WIFSTOPPED(status),(pid,status)
                assert os.WSTOPSIG(status)==signal.SIGSTOP
                stopped=True;break
            time.sleep(.005)
        assert stopped,'supervisor stop timeout';event('supervisor stop observed')
        owned_signal('supervisor',signal.SIGCONT);event('supervisor SIGCONT sent')
        os.write(master,b'hello\n');event('terminal input sent')
        code=supervisor.wait(timeout=8);result['supervisorExit']=code;event('supervisor exited')
        assert code==17,code;assert restored.read_bytes()==b'foreground-restored';assert b'input=hello' in captured
        result['pass']=True;event('exit17 input and foreground restoration verified')
    except BaseException as e:
        result['error']=type(e).__name__+': '+str(e);traceback.print_exc(file=log);log.flush();save()
    finally:
        if supervisor and supervisor.poll() is None:
            for name in ['child','wrapper','supervisor']:owned_signal(name,signal.SIGKILL)
            try:supervisor.wait(timeout=5)
            except subprocess.TimeoutExpired:result['supervisorReapTimeout']=True
        if conn:conn.close()
        if listener:listener.close()
        if daemon and daemon.poll() is None:owned_signal('daemon',signal.SIGTERM)
        if daemon:
            try:daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:owned_signal('daemon',signal.SIGKILL);daemon.wait(timeout=5)
            result['daemonExit']=daemon.returncode
        deadline=time.monotonic()+4
        while time.monotonic()<deadline:
            checks={name:native(item['pid']) for name,item in result['identities'].items()}
            if all(item is None or item.get('start_token')!=result['identities'][name]['start_token'] for name,item in checks.items()):break
            time.sleep(.01)
        result['finalIdentityObservations']=checks
        result['allRecordedIdentitiesAbsent']=all(item is None for item in checks.values())
        reader_stop.set()
        if reader:reader.join(timeout=2)
        for fd in [master,slave]:
            if fd is not None:os.close(fd)
        result['outputSha256']=hashlib.sha256(captured).hexdigest();result['ended']=time.time();result['finalFixtureInventory']=fixture_inventory();save()
print(json.dumps({k:result.get(k) for k in ['pass','error','supervisorExit','daemonExit','allRecordedIdentitiesAbsent','finalIdentityObservations','finalFixtureInventory']}),flush=True)
raise SystemExit(0 if result['pass'] and result['allRecordedIdentitiesAbsent'] else 1)
