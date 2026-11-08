#!/usr/bin/env python3
"""Native NOTE_SIGNAL experiment using the unchanged pipeline fixture/controller."""
import importlib.util,json,os,pathlib,select,signal,sys
sys.dont_write_bytecode=True
source=pathlib.Path.cwd()/'crates/arc-daemon/tests/managed_exec/pipeline.py'
spec=importlib.util.spec_from_file_location('native_pipeline',source)
p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)
NOTE_SIGNAL=0x08000000
original_accept=p.accept
original_receive=p.receive
original_scenario=p.scenario
active=[]
observations=[]
coalesce=False

def event(e):
 return {'ident':e.ident,'filter':e.filter,'flags':e.flags,'fflags':e.fflags,'data':e.data,'udata':e.udata}

def accept(listener):
 stream,value=original_accept(listener)
 if listener.getsockname().endswith('producer.sock'):
  k=select.kqueue();pid=value['identity']['pid']
  receipts=k.control([select.kevent(pid,filter=select.KQ_FILTER_PROC,flags=select.KQ_EV_ADD|select.KQ_EV_ENABLE|0x0040,fflags=NOTE_SIGNAL)],1,0)
  active.append((k,pid,stream))
  observations.append({'registration':list(map(event,receipts)),'identity':value['identity']})
 return stream,value

def receive(stream):
 value=original_receive(stream)
 if 'signals' in value and active:
  k,pid,producer=active[-1]
  if stream is producer:
   events=[] if coalesce and value['signals'] < 2 else k.control(None,32,0)
   observations.append({'participantAcknowledgment':value,'childEvents':list(map(event,events))})
 return value

p.accept=accept;p.receive=receive
root=pathlib.Path(sys.argv[1]);root.mkdir()
fixture=sys.argv[2];arcctl=sys.argv[3]
results=[]
for mode in ['terminal-int','group-int','wrapper-term','provenance']:
 coalesce=mode=='provenance'
 before=len(observations)
 ok=original_scenario(root,False,mode,fixture,arcctl)
 # All delivery assertions are existing fixture assertions. Read event results
 # only after participant acknowledgments; absence is never a forwarding rule.
 rows=observations[before:]
 results.append({'mode':mode,'fixturePassed':ok,'observations':rows})
 for k,_,_ in active:k.close()
 active.clear()
print(json.dumps({'candidate':'EVFILT_PROC NOTE_SIGNAL child receipt','results':results,
 'allBaselineCasesPassed':all(r['fixturePassed'] for r in results),
 'hasSignalNumbers':any(e['data']!=0 or e['fflags']&0xfffff for r in results for o in r['observations'] for e in o.get('childEvents',[]))},sort_keys=True))
assert all(r['fixturePassed'] for r in results)
assert not any(e['data']!=0 or e['fflags']&0xfffff for r in results for o in r['observations'] for e in o.get('childEvents',[])), 'probe assumption changed; inspect native output'
