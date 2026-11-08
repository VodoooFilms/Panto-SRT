#!/usr/bin/env node
'use strict';

/**
 * Prepares the stable-diffusion.cpp engine (`sd-cli`) used by the Upscale
 * tab's RelayVSR AI models. Downloads the pinned official macOS release
 * (universal binary + libstable-diffusion.dylib) into build/sd/.
 * Models are NOT downloaded here; the app downloads them on demand.
 */

const fs = require('fs');
const path = require('path');
const { execFile } = require('child_process');

const ROOT = path.join(__dirname, '..');
const SD_DIR = path.join(ROOT, 'build', 'sd');
const TAG = 'master-1063-3d315c4';
const ASSET = `sd-${TAG}-bin-Darwin-macOS-26.7.2-arm64.zip`;
const URL = `https://github.com/leejet/stable-diffusion.cpp/releases/download/${TAG}/${ASSET}`;

const log = (m) => process.stdout.write(`  ${m}\n`);

function run(command, args) {
  return new Promise((resolve, reject) => {
    execFile(command, args, (err) => (err ? reject(err) : resolve()));
  });
}

async function main() {
  fs.mkdirSync(SD_DIR, { recursive: true });
  const binary = path.join(SD_DIR, 'sd-cli');
  const lib = path.join(SD_DIR, 'libstable-diffusion.dylib');
  if (fs.existsSync(binary) && fs.existsSync(lib)) {
    log('✓ Motor ya presente: build/sd/sd-cli');
    return;
  }
  if (process.platform !== 'darwin') {
    log('⚠ RelayVSR/sd-cli: por ahora solo se prepara en macOS; se omite.');
    return;
  }
  const zip = path.join(SD_DIR, 'sd-mac.zip');
  log(`Descargando ${ASSET}…`);
  await run('curl', ['-L', '--fail', '-sS', '-o', zip, URL]);
  const tmp = path.join(SD_DIR, 'extract-tmp');
  fs.rmSync(tmp, { recursive: true, force: true });
  fs.mkdirSync(tmp, { recursive: true });
  const { default: extract } = await import('extract-zip');
  await extract(zip, { dir: tmp });
  for (const name of ['sd-cli', 'libstable-diffusion.dylib']) {
    const found = path.join(tmp, name);
    if (!fs.existsSync(found)) throw new Error(`${name} no está dentro del zip.`);
    fs.copyFileSync(found, path.join(SD_DIR, name));
  }
  fs.chmodSync(binary, 0o755);
  fs.rmSync(tmp, { recursive: true, force: true });
  fs.rmSync(zip, { force: true });
  log('✓ Motor listo: build/sd/sd-cli');
}

main().catch((cause) => {
  process.stderr.write(`✖ ${cause.message}\n`);
  process.exitCode = 1;
});
