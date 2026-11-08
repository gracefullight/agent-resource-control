# Native CLI privacy verification. Synthetic sentinels never enter its receipt.
import ctypes, os, signal, subprocess, tempfile, select, sqlite3, uuid, json
from pathlib import Path
BASE=Path.cwd()
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

identities={}
def own(name,process):
    identity=native(process.pid)
    assert identity and 'start_token' in identity
    identities[name]=identity
def send_owned(name,process,sig):
    current=native(process.pid)
    if current and current.get('start_token')==identities[name]['start_token']:os.kill(process.pid,sig)
daemon=cli=None
result={'scope':'native CLI sentinel audit with pinned live SQLite WAL','pass':False}
with tempfile.TemporaryDirectory(prefix='arc-priv-',dir='/tmp') as tmp:
    root=Path(tmp);os.chmod(root,0o700)
    env=dict(os.environ,ARC_DB_PATH=str(root/'db'),ARC_SOCKET_PATH=str(root/'daemon.sock'))
    env.pop('ARC_RUNTIME_REPORT',None)
    with (root/'daemon-out').open('wb') as out,(root/'daemon-err').open('wb') as err:
        try:
            daemon=subprocess.Popen([str(BASE/'target/debug/arc-daemon')],env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=err)
            own('daemon',daemon)
            assert select.select([daemon.stdout],[],[],10)[0]
            ready=daemon.stdout.readline();assert b'arc-daemon ready.' in ready, {'startupStdout':ready.decode(errors='replace'),'startupStderr':(root/'daemon-err').read_text()};out.write(ready);out.flush()
            reader=sqlite3.connect(root/'db');reader.execute('BEGIN');reader.execute('SELECT * FROM sqlite_master').fetchall()
            arg='synthetic-arg-'+uuid.uuid4().hex;secret='synthetic-env-'+uuid.uuid4().hex
            childenv=dict(env,ARC_FIXTURE_MODE='echo',PRIVATE_ENV=secret)
            cli=subprocess.Popen([str(BASE/'target/debug/arcctl'),'exec','--',str(BASE/'target/debug/arc-exec-fixture'),arg],env=childenv,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            own('wrapper',cli)
            stdout,stderr=cli.communicate(timeout=30)
            assert cli.returncode==17 and stderr==b'fixture-stderr\n'
            observed=json.loads(stdout);assert observed['args']==[list(arg.encode())] and observed['environment']==list(secret.encode())
            fresh=sqlite3.connect(root/'db')
            assert fresh.execute('SELECT count(*) FROM job_lifecycle').fetchone()[0]==1
            assert fresh.execute('SELECT count(*) FROM permits').fetchone()[0]==0
            dump='\n'.join(fresh.iterdump()).encode();fresh.close()
            wal=root/'db-wal';assert wal.is_file() and wal.stat().st_size>0
            checked=[]
            for path in root.iterdir():
                if path.is_file():
                    data=path.read_bytes();assert arg.encode() not in data and secret.encode() not in data
                    checked.append(path.name)
            assert arg.encode() not in dump and secret.encode() not in dump
            result.update(pass_=True,exitCode=cli.returncode,oneLifecycleRecord=True,permits=0,liveWalChecked=True,checkedFiles=sorted(checked),databaseRowsSecretFree=True,arcDiagnosticsSecretFree=True)
            reader.rollback();reader.close()
            result['pass']=True
        finally:
            if cli and cli.poll() is None:
                send_owned('wrapper',cli,signal.SIGKILL);cli.communicate(timeout=5)
            if daemon and daemon.poll() is None:send_owned('daemon',daemon,signal.SIGTERM)
            if daemon:daemon.wait(timeout=5);result['daemonExit']=daemon.returncode
result['finalIdentityObservations']={name:native(item['pid']) for name,item in identities.items()}
result['allRecordedIdentitiesAbsent']=all(item is None for item in result['finalIdentityObservations'].values())
assert result['pass'] and result['allRecordedIdentitiesAbsent'] and result['daemonExit']==0
print(json.dumps(result,sort_keys=True))
