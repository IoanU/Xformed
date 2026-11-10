// A static-only server at the same subpath as GitHub Pages. No conversion endpoint.
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { resolve, sep, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../dist/', import.meta.url));
const mount = '/Xformed/';
const port = Number(process.argv[2] || 8082);
const mime = { '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8', '.wasm': 'application/wasm', '.json': 'application/json' };
createServer(async (request, response) => {
  try {
    const path = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    if (path === '/' || path === mount.slice(0, -1)) {
      response.writeHead(302, { Location: mount }); response.end(); return;
    }
    if (request.method !== 'GET' && request.method !== 'HEAD') throw new Error('Static server');
    if (!path.startsWith(mount)) throw new Error('Unknown path');
    const file = resolve(root, path.slice(mount.length) || 'index.html');
    if (!file.startsWith(resolve(root) + sep)) throw new Error('Invalid path');
    if (!(await stat(file)).isFile()) throw new Error('Not a file');
    const body = await readFile(file);
    response.writeHead(200, { 'Content-Type': mime[extname(file)] || 'application/octet-stream',
      'Content-Length': body.length, 'Cache-Control': 'no-store' });
    response.end(request.method === 'HEAD' ? undefined : body);
    console.log(`${request.method} ${path} 200`);
  } catch {
    response.writeHead(404); response.end('Not found');
    console.log(`${request.method} ${request.url} 404`);
  }
}).listen(port, '127.0.0.1', () => console.log(`Static-only studio: http://127.0.0.1:${port}${mount}`));
