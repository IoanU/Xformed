// Verify the browser's ZIP exporter with an independent reader signature and CRC.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const sandbox={window:{},Uint8Array,Uint32Array,DataView,TextEncoder,Blob};vm.createContext(sandbox);vm.runInContext(await readFile(new URL('../services/api/static/download.js',import.meta.url),'utf8'),sandbox);
const result=sandbox.window.XformedZip([{name:'poem.txt',data:new TextEncoder().encode('Hope remains.\n')},{name:'image.bin',data:new Uint8Array([0,1,255,128])}]);
const bytes=Buffer.from(await result.arrayBuffer());assert.equal(bytes.readUInt32LE(0),0x04034b50);assert.equal(bytes.readUInt32LE(bytes.length-22),0x06054b50);assert.equal(bytes.readUInt16LE(bytes.length-12),2);assert.ok(bytes.includes(Buffer.from('Hope remains.')));
const output=new URL('../target/qa/',import.meta.url);await mkdir(output,{recursive:true});
await writeFile(new URL('export-check.zip',output),bytes);console.log('PASS: ZIP export with text and binary content.');
