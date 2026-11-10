// Cross-source diversity audit. Excludes pitch transposition and microtiming from novelty checks.
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
const root=process.argv[2]||'http://127.0.0.1:8081';
const out=new URL('../../outputs/diversity/',import.meta.url);await mkdir(out,{recursive:true});
const corpus=[
 ['rain','Rain gathers on the windows. Somewhere, a city is learning to be quiet.'],
 ['joy','We dance in bright sunlight! Love, joy and hope burst through every open door!'],
 ['clock','Copper gears measure another interval. Voltage rises; the mechanism hesitates, then accelerates.'],
 ['absence','I miss you. The empty room is cold, and the sadness will not let me sleep.'],
 ['garden','Every leaf holds a little rain. Gentle light, soft branches, a peaceful afternoon.'],
 ['train','Trains race past the market, brakes shout, wheels stutter, then a sudden quiet.'],
 ['night','Mi-e frică de întuneric, sunt singur. Durere, tristețe și o speranță mică.'],
 ['star','One star remains above an ocean without a shore. There is no hurry here.']
];
const post=async(text,to,options={})=>{const response=await fetch(root+'/convert',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({from:'text',to,options,payload:{kind:'Text',text}})});const result=await response.json();assert.equal(response.status,200,JSON.stringify(result));return result;};
const artifact=(r,type)=>r.artifacts.find(a=>a.type===type);
const decode=a=>Buffer.from(a.data_b64,'base64');
const interval=s=>{const ns=s.tracks.find(t=>t.name==='Lead').notes;return ns.slice(0,33).slice(1).map((n,i)=>n.pitch-ns[i].pitch).join(',');};
const rhythm=s=>{const ns=s.tracks.find(t=>t.name==='Lead').notes;return ns.slice(0,33).slice(1).map((n,i)=>Math.round((n.start-ns[i].start)*4)).join(',');};
const report={art:[],artTakes:[],music:[],checks:{}};
for(const style of ['nebula','bloom','contour'])for(const[id,text]of corpus){const start=performance.now();const r=await post(text,'image',{image_style:style});const info=artifact(r,'CreativeInfo');const name=`${style}-${id}.png`;await writeFile(new URL(name,out),decode(artifact(r,'PngBase64')));report.art.push({style,source:id,file:name,composition:info.title,seed:info.seed,milliseconds:Math.round(performance.now()-start)});console.log(`${style}/${id}: ${info.title}`);}
for(const style of ['nebula','bloom','contour']){
 const names=new Set();
 for(let variation=0;variation<4;variation++){
  const r=await post(corpus[0][1],'image',{image_style:style,variation});
  const name=`${style}-take${variation}.png`, title=artifact(r,'CreativeInfo').title;
  names.add(title);await writeFile(new URL(name,out),decode(artifact(r,'PngBase64')));
  report.artTakes.push({style,variation,file:name,composition:title});
 }
 assert.equal(names.size,4,`${style}: takes must explore all composition families`);
}
for(const[id,text]of corpus){const r=await post(text,'audio',{target_seconds:16});const score=JSON.parse(decode(artifact(r,'MidiJsonBase64')));const wave=decode(artifact(r,'WavBase64'));assert.equal(wave.readUInt16LE(22),2);const midi=decode(artifact(r,'MidiBase64'));assert.equal(midi.readUInt16BE(10),score.tracks.length+1);assert.ok(midi.includes(Buffer.from([0xff,0x58,4,score.meter[0],Math.log2(score.meter[1])])),`${id}: exported MIDI meter must match score`);await writeFile(new URL(`music-${id}.wav`,out),wave);await writeFile(new URL(`music-${id}.score.json`,out),JSON.stringify(score,null,2));await writeFile(new URL(`music-${id}.mid`,out),midi);report.music.push({source:id,character:score.identity.character,key:score.key,meter:score.meter.join('/'),bpm:score.tempo_bpm,tracks:score.tracks.length,leadNotes:score.tracks.find(t=>t.name==='Lead').notes.length,intervalSignature:interval(score),rhythmSignature:rhythm(score),progression:score.identity.progression});}
const repeatedA=await post(corpus[0][1],'audio',{target_seconds:16});const repeatedB=await post(corpus[0][1],'audio',{target_seconds:16});assert.equal(artifact(repeatedA,'MidiJsonBase64').data_b64,artifact(repeatedB,'MidiJsonBase64').data_b64);
const take=await post(corpus[0][1],'audio',{target_seconds:16,variation:1});assert.notEqual(interval(JSON.parse(decode(artifact(take,'MidiJsonBase64')))),interval(JSON.parse(decode(artifact(repeatedA,'MidiJsonBase64')))));
const prefix='These first eight words are exactly the same. ';
const a=JSON.parse(decode(artifact(await post(prefix+'But the ending is a wild storm full of fear.','audio',{target_seconds:16}),'MidiJsonBase64')));
const b=JSON.parse(decode(artifact(await post(prefix+'But the ending is a peaceful garden full of hope.','audio',{target_seconds:16}),'MidiJsonBase64')));
assert.notEqual(interval(a),interval(b));
report.checks={artFamiliesAcrossTakes:12,uniqueMelodicIntervalSequences:new Set(report.music.map(x=>x.intervalSignature)).size,uniqueQuantizedRhythms:new Set(report.music.map(x=>x.rhythmSignature)).size,uniqueHarmonicSequences:new Set(report.music.map(x=>x.progression.join(','))).size,characters:new Set(report.music.map(x=>x.character)).size,meters:new Set(report.music.map(x=>x.meter)).size,repeatable:true,lateContentMatters:true,musicVariationsChangeMelody:true,midiMeterMatchesScore:true};
assert.ok(report.checks.uniqueMelodicIntervalSequences>=7);assert.ok(report.checks.uniqueQuantizedRhythms>=6);assert.ok(report.checks.uniqueHarmonicSequences>=6);assert.ok(report.checks.characters>=3);assert.ok(report.checks.meters>=2);
await writeFile(new URL('diversity-report.json',out),JSON.stringify(report,null,2));console.log(JSON.stringify(report.checks,null,2));
