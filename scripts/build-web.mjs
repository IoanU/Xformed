// Rust + Node only. The official wasm-bindgen release is checksum-verified and cached.
import { mkdir, cp, readFile, writeFile, access } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join, resolve } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const version = '0.2.130';
const platform = {
  'win32-x64': 'x86_64-pc-windows-msvc',
  'linux-x64': 'x86_64-unknown-linux-musl',
  'linux-arm64': 'aarch64-unknown-linux-musl',
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin'
}[`${process.platform}-${process.arch}`];
const executable = process.platform === 'win32' ? 'wasm-bindgen.exe' : 'wasm-bindgen';
const tools = join(root, 'target', 'web-tools');
let bindgen = process.env.WASM_BINDGEN;

async function download(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${response.status}: ${url}`);
  return Buffer.from(await response.arrayBuffer());
}
if (!bindgen) {
  if (!platform) throw new Error('Set WASM_BINDGEN to a compatible wasm-bindgen executable.');
  const name = `wasm-bindgen-${version}-${platform}`;
  bindgen = join(tools, name, executable);
  try { await access(bindgen); } catch {
    await mkdir(tools, { recursive: true });
    const release = `https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${version}/${name}.tar.gz`;
    const archive = await download(release);
    const checksum = (await download(`${release}.sha256sum`)).toString().trim().split(/\s+/)[0];
    if (createHash('sha256').update(archive).digest('hex') !== checksum) throw new Error('wasm-bindgen checksum mismatch.');
    const archivePath = join(tools, `${name}.tar.gz`);
    await writeFile(archivePath, archive);
    execFileSync('tar', ['-xzf', archivePath, '-C', tools], { stdio: 'inherit' });
  }
}
if (!execFileSync(bindgen, ['--version'], { encoding: 'utf8' }).includes(version)) {
  throw new Error(`wasm-bindgen ${version} is required.`);
}
if (!process.argv.includes('--assets-only')) {
  execFileSync(process.env.CARGO || 'cargo', ['build', '--locked', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'xformed-web'], { cwd: root, stdio: 'inherit' });
}
const output = resolve(root, 'dist');
await mkdir(output, { recursive: true });
await cp(join(root, 'services', 'api', 'static'), output, { recursive: true });
const html = await readFile(join(output, 'index.html'), 'utf8');
await writeFile(join(output, 'index.html'), html.replace('<html lang="en">', '<html lang="en" data-engine="wasm">'));
await writeFile(join(output, '.nojekyll'), '');
await writeFile(join(output, 'package.json'), '{"type":"module"}\n');
execFileSync(bindgen, [join(root, 'target', 'wasm32-unknown-unknown', 'release', 'xformed_web.wasm'), '--target', 'web', '--out-dir', join(output, 'wasm'), '--out-name', 'xformed_web'], { stdio: 'inherit' });
console.log(`Static WebAssembly studio: ${output}`);
