import hashlib,json,subprocess,sys,time
from pathlib import Path
root=Path.cwd()
out=root/'.arc-verify/schema-compatibility/storage'
label=sys.argv[1]
command=sys.argv[2:]
inputs=[root/'Cargo.toml',root/'Cargo.lock',root/'crates/arc-storage/Cargo.toml']
inputs+=sorted((root/'crates/arc-storage/src').rglob('*.rs'))
inputs+=sorted((root/'crates/arc-storage/tests').rglob('*'))
inputs+=sorted((root/'crates/arc-storage/migrations').glob('*.sql'))
def manifest():
    return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs if p.is_file()}
before=manifest()
started=time.time()
with (out/(label+'.txt')).open('w') as log:
    try:
        result=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=120)
        code=result.returncode
    except subprocess.TimeoutExpired:
        log.write('\nCommand exceeded 120-second check deadline.\n')
        code=124
receipt={'command':command,'cwd':str(root),'exitCode':code,'elapsedSeconds':time.time()-started,'sourceBound':before==manifest(),'inputs':before}
(out/(label+'.json')).write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({k:v for k,v in receipt.items() if k!='inputs'}))
print((out/(label+'.txt')).read_text()[-10000:])
sys.exit(code)
