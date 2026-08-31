#!/usr/bin/env node
import { chmodSync, existsSync } from 'node:fs';
import { resolve, join } from 'node:path';

const binPath = resolve(join('bin', 'yfiles'));
if (existsSync(binPath)) {
  chmodSync(binPath, 0o755);
}
