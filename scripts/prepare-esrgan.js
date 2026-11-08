#!/usr/bin/env node
'use strict';

/**
 * Prepares the Real-ESRGAN engine for the Upscale tab: the official
 * `realesrgan-ncnn-vulkan` macOS runner (MIT, from xinntao/Real-ESRGAN-ncnn-vulkan)
 * plus the tiny `realesr-animevideov3` x2/x4 ncnn weights (BSD-3-Clause,
 * creator: Xintao Wang; ncnn copy mirrored at huggingface.co/edgetools/realesrgan).
 * Both live in build/esrgan/ and ship inside the app bundle — nothing is
 * downloaded at runtime.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { execFile } = require('child_process');

const ROOT = path.join(__dirname, '..');
const ESRGAN_DIR = path.join(ROOT, 'build', 'esrgan');
const MODELS_DIR = path.join(ESRGAN_DIR, 'models');
const VERSION = 'v0.2.0';
const RUNNER_URL =
  `https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/releases/download/` +
  `${VERSION}/realesrgan-ncnn-vulkan-${VERSION}-macos.zip`;
const WEIGHTS_URL = 'https://huggingface.co/edgetools/realesrgan/resolve/main';
const SHA256 = {
  'realesr-animevideov3-x2.bin': '548a36f9c3f4ab8da56cd3b13badf23968bee207b396dad14d04b830e5f2ab2d',
  'realesr-animevideov3-x2.param': 'b88ff4f00ebf019a7fdac17fdd45a7fd3665d37509efc5baf2e4da2e24420a04',
  'realesr-animevideov3-x4.bin': '548a36f9c3f4ab8da56cd3b13badf23968bee207b396dad14d04b830e5f2ab2d',
  'realesr-animevideov3-x4.param': '850a248e7c14c27e5bd8cf7265113a9441036a7db63963bb8aa5169d788a435e',
};

const log = (m) => process.stdout.write(`  ${m}\n`);

function run(command, args) {
  return new Promise((resolve, reject) => {
    execFile(command, args, (err) => (err ? reject(err) : resolve()));
  });
}

async function main() {
  fs.mkdirSync(MODELS_DIR, { recursive: true });

  const runner = path.join(ESRGAN_DIR, 'realesrgan-ncnn-vulkan');
  if (!fs.existsSync(runner)) {
    if (process.platform !== 'darwin') {
      log('⚠ Real-ESRGAN: runner solo se prepara en macOS; se omite.');
      return;
    }
    const zip = path.join(ESRGAN_DIR, 'runner.zip');
    log(`Descargando realesrgan-ncnn-vulkan ${VERSION} (macOS)…`);
    await run('curl', ['-L', '--fail', '-sS', '-o', zip, RUNNER_URL]);
    const tmp = path.join(ESRGAN_DIR, 'extract-tmp');
    fs.rmSync(tmp, { recursive: true, force: true });
    fs.mkdirSync(tmp, { recursive: true });
    await run('unzip', ['-q', '-d', tmp, zip]);
    const bin = path.join(tmp, `realesrgan-ncnn-vulkan-${VERSION}-macos`, 'realesrgan-ncnn-vulkan');
    if (!fs.existsSync(bin)) throw new Error('El runner no está dentro del zip.');
    fs.copyFileSync(bin, runner);
    fs.chmodSync(runner, 0o755);
    fs.rmSync(tmp, { recursive: true, force: true });
    fs.rmSync(zip, { force: true });
    log('✓ Runner listo: build/esrgan/realesrgan-ncnn-vulkan');
  }

  for (const [name, expected] of Object.entries(SHA256)) {
    const target = path.join(MODELS_DIR, name);
    if (fs.existsSync(target)) {
      const actual = crypto.createHash('sha256').update(fs.readFileSync(target)).digest('hex');
      if (actual === expected) continue;
      fs.rmSync(target, { force: true });
    }
    log(`Descargando peso ${name} (1,2 MB)…`);
    await run('curl', ['-L', '--fail', '-sS', '-o', target, `${WEIGHTS_URL}/${name}`]);
    const actual = crypto.createHash('sha256').update(fs.readFileSync(target)).digest('hex');
    if (actual !== expected) {
      fs.rmSync(target, { force: true });
      throw new Error(`SHA-256 no coincide para ${name}.`);
    }
  }
  log('✓ Pesos listos: build/esrgan/models/');
}

main().catch((cause) => {
  process.stderr.write(`✖ ${cause.message}\n`);
  process.exitCode = 1;
});
