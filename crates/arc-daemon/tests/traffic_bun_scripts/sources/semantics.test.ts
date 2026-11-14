import { expect, test } from 'bun:test';
test('literal arguments', async () => {
  const input = [...new Uint8Array(await Bun.stdin.arrayBuffer())];
  expect(input).toEqual([...new TextEncoder().encode('stdin\0bytes\n')]);
  console.log('NATIVE:' + JSON.stringify({event: process.env.npm_lifecycle_event, cwd:process.cwd(), args:process.argv.slice(2), input, value:process.env.TBS_VALUE}));
  console.error('NATIVE_STDERR');
});
