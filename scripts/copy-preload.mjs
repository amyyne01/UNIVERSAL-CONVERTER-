import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

// preload.cjs is CommonJS and is NOT compiled by tsc — copy it verbatim next to main.js.
const dest = 'dist-electron/electron/preload.cjs';
mkdirSync(dirname(dest), { recursive: true });
copyFileSync('electron/preload.cjs', dest);
console.log('Copied preload.cjs ->', dest);
