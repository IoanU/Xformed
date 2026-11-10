// Run with the Studio running: node scripts/smoke.mjs [http://127.0.0.1:8080]
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
const root=process.argv[2]||'http://127.0.0.1:8080';
const call=body=>fetch(`${root}/convert`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
assert.equal((await fetch(`${root}/health`)).status,200);
assert.match(await(await fetch(root)).text(),/Transduction instrument/);
const image=(await readFile(new URL('../inputs/images.png',import.meta.url))).toString('base64');
const text={kind:'Text',text:'The morning light spills over the hills, full of hope and music.'};
let wav;
for(const from of ['text','image']) {
  const payload=from==='text'?text:{kind:'ImageBase64',data_b64:image};
  const result=await call({from,to:'audio',options:{target_seconds:12},payload});assert.equal(result.status,200);
  const {artifacts}=await result.json();
  const midi=Buffer.from(artifacts.find(a=>a.type==='MidiBase64').data_b64,'base64');
  assert.equal(midi.toString('ascii',0,4),'MThd');assert.ok(midi.readUInt16BE(10)>=4);
  const score=JSON.parse(Buffer.from(artifacts.find(a=>a.type==='MidiJsonBase64').data_b64,'base64').toString());
  assert.ok(score.tracks.length>=3&&score.tracks.length<=7);assert.ok(score.tracks.every(t=>t.notes.length>0));assert.ok(score.identity&&score.meter);
  assert.ok(score.sections.length>=2);assert.ok(score.tracks.every(t=>t.instrument!=='Drums'||t.channel===9));
  wav=artifacts.find(a=>a.type==='WavBase64').data_b64;
  const bytes=Buffer.from(wav,'base64');assert.equal(bytes.toString('ascii',0,4),'RIFF');assert.equal(bytes.readUInt16LE(22),2);
  assert.ok(bytes.length>12*44100*4);
}
const cases=[
 ['text','image',text,'PngBase64'],
 ['image','text',{kind:'ImageBase64',data_b64:image},'Text'],
 ['audio','image',{kind:'AudioBase64',data_b64:wav},'PngBase64'],
 ['audio','text',{kind:'AudioBase64',data_b64:wav},'Text'],
 ...[['text',text],['image',{kind:'ImageBase64',data_b64:image}],['audio',{kind:'AudioBase64',data_b64:wav}]].map(([from,payload])=>[from,'json',payload,'Json'])
];
for(const [from,to,payload,type] of cases){
 const response=await call({from,to,payload});assert.equal(response.status,200,`${from}->${to}`);
 const result=await response.json(),artifact=result.artifacts.find(a=>a.type===type);assert.ok(artifact,`${from}->${to}`);
 if(type==='PngBase64')assert.equal(Buffer.from(artifact.data_b64,'base64').subarray(1,4).toString(),'PNG');
 if(type==='Text')assert.ok(artifact.text.length>100);
}
for(const body of [
 {from:'text',to:'audio',payload:{kind:'Text',text:''}},
 {from:'text',to:'audio',options:{target_seconds:999},payload:text},
 {from:'image',to:'text',payload:{kind:'ImageBase64',data_b64:'broken'}},
 {from:'audio',to:'image',payload:{kind:'AudioBase64',data_b64:'broken'}},
 {from:'text',to:'video',payload:text},
 {from:'audio',to:'text',payload:text}
]){const response=await call(body);assert.equal(response.status,400);assert.ok((await response.json()).error);}
assert.equal((await fetch(`${root}/health`)).status,200);
console.log('PASS: all six cross-modal directions, three analysis routes, stereo WAV, multi-track MIDI, sections, and invalid-input recovery.');
