// Real-file acceptance run. Reads selected Downloads files; never changes the originals.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {join} from 'node:path';
import assert from 'node:assert/strict';
const wasm=process.argv[2]==='--wasm'?await(await import('./wasm-test-engine.mjs')).testEngine():null;
const base=process.argv[2]||'http://127.0.0.1:8081', downloads='C:/Users/ioan/Downloads';
const out=new URL('../../outputs/MVP-review/',import.meta.url);await mkdir(out,{recursive:true});
const cases=[
 ['Scottish-Loch_by_-1024x768.jpg','image',['text','audio','json']],
 ['20211119-rectoryterrace-397-686bcf82841e2.jpeg','image',['text','json']],
 ['ai_fft.png','image',['text','audio','json']],
 ['IMG_5951.jpg','image',['text','json']],
 ['xformed.wav','audio',['image','text','json']],
 ['fuhhhh.mp3','audio',['image','text','json']],
 ['videoplayback.mp3','audio',['image','text']]
];
const report=[];let index=0;
for(const [name,from,targets] of cases){
 const data=await readFile(join(downloads,name));
 for(const to of targets){const start=performance.now();const body={from,to,options:{target_seconds:3,image_style:from==='audio'?'bloom':'nebula',text_style:index%2?'vignette':'poem'},payload:{kind:from==='image'?'ImageBase64':'AudioBase64',data_b64:data.toString('base64')}};
 let result;
 if(wasm)result=await wasm.engine.convert(body);
 else {const response=await fetch(base+'/convert',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const raw=await response.text();assert.equal(response.status,200,`${name} -> ${to}: ${raw.slice(0,200)}`);result=JSON.parse(raw);}
 if(to==='text'){const text=result.artifacts.find(a=>a.type==='Text').text;assert.ok(text.length>100);assert.ok(!/\b(pixels|RMS|Hz|centroid|entropy|dBFS)\b/.test(text));await writeFile(new URL(`download-${index}-writing.txt`,out),text);}
 if(to==='image'){const png=Buffer.from(result.artifacts.find(a=>a.type==='PngBase64').data_b64,'base64');assert.equal(png.subarray(1,4).toString(),'PNG');assert.equal(png.readUInt32BE(16),1280);await writeFile(new URL(`download-${index}-art.png`,out),png);}
 if(to==='audio'){assert.ok(result.artifacts.some(a=>a.type==='WavBase64'));assert.ok(result.artifacts.some(a=>a.type==='MidiBase64'));}
 if(to!=='json'&&to!=='audio')assert.ok(!result.artifacts.some(a=>a.type==='Json'));
 const info=result.artifacts.find(a=>a.type==='CreativeInfo');const row={source:name,from,to,status:'PASS',milliseconds:Math.round(performance.now()-start),mood:info?.mood,title:info?.title};report.push(row);console.log(JSON.stringify(row));index++;
 }
}
await writeFile(new URL(wasm?'downloads-wasm-acceptance.json':'downloads-acceptance.json',out),JSON.stringify(report,null,2));
console.log(`PASS: ${report.length} real-file transformations across ${cases.length} Downloads files. Originals unchanged.`);

wasm?.close();
