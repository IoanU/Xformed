/* Local result collection with short source labels. Original file bytes are not persisted. */
(() => {
  let pending;
  function db(){if(!pending)pending=new Promise((resolve,reject)=>{const request=indexedDB.open('xformed-creations',1);request.onupgradeneeded=()=>request.result.createObjectStore('results',{keyPath:'id'});request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});return pending;}
  async function transaction(mode,work){const connection=await db();return new Promise((resolve,reject)=>{const tx=connection.transaction('results',mode);let answer;work(tx.objectStore('results'),value=>{answer=value;});tx.oncomplete=()=>resolve(answer);tx.onerror=()=>reject(tx.error);tx.onabort=()=>reject(tx.error);});}
  async function list(){return(await transaction('readonly',(store,done)=>{const r=store.getAll();r.onsuccess=()=>done(r.result);})).sort((a,b)=>b.created-a.created);}
  window.XformedCollection={list,async save(item){await transaction('readwrite',store=>store.put(item));const items=await list();if(items.length>6)await transaction('readwrite',store=>items.slice(6).forEach(item=>store.delete(item.id)));},clear(){return transaction('readwrite',store=>store.clear());}};
})();
