import { appendFileSync, existsSync } from 'node:fs';
export default {
  plugins: [{
    name: 'real-vite-entry-barrier',
    async buildStart() {
      appendFileSync(process.env.TBS_BODY, `${process.pid}\n`);
      while (!existsSync(process.env.TBS_RELEASE)) await new Promise(resolve => setTimeout(resolve, 10));
    },
  }],
};
