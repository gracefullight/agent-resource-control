import { expect, test } from 'bun:test';
import { appendFileSync, existsSync } from 'node:fs';
test('independent native result', async () => {
  appendFileSync(process.env.TBS_BODY!, `${process.pid}\n`);
  while (!existsSync(process.env.TBS_RELEASE!)) await Bun.sleep(10);
  expect(6 * 7).toBe(42);
  console.log('TBS_TEST_OK');
});
