#!/usr/bin/env node
'use strict';

/**
 * Prepares the NeMo-Speech.cpp runtime used by the Parakeet engine.
 *
 * The upstream release is macOS Apple Silicon only, so other platforms get an
 * empty build/nemo directory (the Tauri resource still needs to exist) and a
 * clear message instead of a broken build.
 */

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const DEST = path.join(ROOT, 'build', 'nemo');

fs.mkdirSync(DEST, { recursive: true });

if (process.platform !== 'darwin' || process.arch !== 'arm64') {
  process.stdout.write('  ⚠ NeMo/Parakeet solo está disponible en macOS Apple Silicon; se omite.\n');
  process.exit(0);
}

const result = spawnSync('sh', [path.join(__dirname, 'prepare-nemo.sh')], {
  stdio: 'inherit'
});
process.exit(result.status === null ? 1 : result.status);
