// Executed in a separate Python process. This bounded fixture implements only
// the old unsupported dispatch and the old lifecycle shapes used here; it is
// not an accepted daemon binary. Its source boundary is accepted commit
// 16772fe2aa350413225f3fb2bb9227d14ae5a6c4: arc-ipc/src/lib.rs SHA-256
// 17ff544a163bf7c831b8f3c84092c03047eb01498a9696c2004fe888229bdf46,
// arc-ipc/src/jobs.rs c75464c6cd8239c862ba100a501975b2189ae37aa6202a0931f7875432619a3e,
// arc-detector/src/types.rs 9a882ebafc075ed3057387fc8856715acf6ca18d4ccda2698f0cafdee64264bd.
const FROZEN_OLD_PEER: &str = r#"
import json, socket, struct, sys
path, log, mode = sys.argv[1:]
listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
listener.bind(path); listener.listen(8)
job = None
def readn(conn, n):
    out = b''
    while len(out) < n:
        data = conn.recv(n-len(out))
        if not data: raise EOFError()
        out += data
    return out
while True:
    conn, _ = listener.accept()
    with conn:
        try:
            n = struct.unpack('>I', readn(conn, 4))[0]
            req = json.loads(readn(conn, n)); op = req['operation']; p = req['payload']
            with open(log, 'a') as f: f.write(json.dumps(req) + '\n')
            out = {'protocol_version': 1, 'request_id': req['request_id']}
            def ok(value): out.update(status='ok', result=value)
            def error(code): out.update(status='error', error={'code': code})
            if op == 'traffic.bun.capabilities' and mode == 'replace':
                ok({'capability': 'traffic.bun.v1'})
            elif op == 'job.start':
                if p['action'] == 'REGISTER':
                    expected = {'classification':'UNKNOWN','locality':'UNKNOWN','reason':'UNRESOLVED_COMMAND','uncertainty':'OBSERVE'}
                    if p['classification'] != expected: error('INVALID_REQUEST')
                    else:
                        job = dict(job_id=p['job_id'], phase='REGISTERED', classification=expected,
                            observer=p['observer'], observer_verified_at_ms=1, child=None,
                            child_verified_at_ms=None, observation={'target':'OBSERVER','status':'VERIFIED_MATCH','observed_at_ms':1,'reason':'REGISTERED_NOT_CLAIMED'},
                            reason='REGISTERED_NOT_CLAIMED', outcome=None, registered_at_ms=1,
                            launch_claimed_at_ms=None, completed_at_ms=None, updated_at_ms=1, revision=1)
                        ok({'disposition':'CREATED','job':job})
                elif p['action'] == 'CLAIM':
                    job.update(phase='LAUNCHING', launch_claimed_at_ms=1, reason='CLAIMED_NOT_ATTACHED',
                        observation={'target':'NONE','status':'NOT_ATTACHED','observed_at_ms':1,'reason':'CLAIMED_NOT_ATTACHED'}, revision=2)
                    ok({'disposition':'APPLIED','job':job})
                elif p['action'] == 'ATTACH':
                    job.update(phase='RUNNING', child=p['child'], child_verified_at_ms=1, reason='CHILD_VERIFIED',
                        observation={'target':'CHILD','status':'VERIFIED_MATCH','observed_at_ms':1,'reason':'CHILD_VERIFIED'}, revision=3)
                    ok({'disposition':'APPLIED','job':job})
                else: error('INVALID_REQUEST')
            elif op == 'job.complete':
                job.update(phase='COMPLETED', outcome=p['outcome'], completed_at_ms=1, reason='OBSERVED_EXIT',
                    observation={'target':'CHILD','status':'VERIFIED_MATCH','observed_at_ms':1,'reason':'OBSERVED_EXIT'}, revision=4)
                ok({'disposition':'APPLIED','job':job})
            elif op == 'job.status':
                ok({'kind':'FOUND','job':job})
            else:
                # Accepted dispatch rejects unknown operations before typed payload decode.
                error('UNSUPPORTED_CAPABILITY')
            data = json.dumps(out).encode(); conn.sendall(struct.pack('>I', len(data)) + data)
        except (EOFError, BrokenPipeError, ConnectionResetError): pass
"#;
