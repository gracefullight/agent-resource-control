import { expect, test } from 'bun:test';
import { appendFileSync, existsSync } from 'node:fs';
test('root with a native nested ARC invocation', async () => {
  appendFileSync(process.env.TBS_BODY!, `${process.pid}\n`);
  const p=process.env.TBS_INNER!;
  const child=Bun.spawn([process.env.TBS_ARCCTL!, 'exec', '--', 'bun', 'run', 'unexpected'], {
    cwd:p,env:{...process.env,TBS_PROJECT:p,TBS_BODY:`${p}/bodies`,TBS_RELEASE:`${p}/release`},stdout:'inherit',stderr:'inherit',
  });
  expect(await child.exited).toBe(0);
  while(!existsSync(process.env.TBS_RELEASE!)) await Bun.sleep(10);
},120000);
