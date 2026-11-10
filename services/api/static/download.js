/* Dependency-free ZIP store writer. Files stay in the browser. */
(() => {
  const table=Uint32Array.from({length:256},(_,n)=>{for(let j=0;j<8;j++)n=n&1?0xedb88320^(n>>>1):n>>>1;return n>>>0;});
  const crc=data=>{let value=0xffffffff;for(const byte of data)value=table[(value^byte)&255]^(value>>>8);return(value^0xffffffff)>>>0;};
  const header=(length)=>{const bytes=new Uint8Array(length);return{bytes,v:new DataView(bytes.buffer)};};
  window.XformedZip=files=>{
    const chunks=[],central=[];let offset=0,size=0;
    for(const file of files){
      const name=new TextEncoder().encode(file.name),data=file.data,checksum=crc(data),local=header(30),directory=header(46);
      local.v.setUint32(0,0x04034b50,true);local.v.setUint16(4,20,true);local.v.setUint16(6,0x800,true);local.v.setUint16(12,33,true);local.v.setUint32(14,checksum,true);local.v.setUint32(18,data.length,true);local.v.setUint32(22,data.length,true);local.v.setUint16(26,name.length,true);
      chunks.push(local.bytes,name,data);
      directory.v.setUint32(0,0x02014b50,true);directory.v.setUint16(4,20,true);directory.v.setUint16(6,20,true);directory.v.setUint16(8,0x800,true);directory.v.setUint16(14,33,true);directory.v.setUint32(16,checksum,true);directory.v.setUint32(20,data.length,true);directory.v.setUint32(24,data.length,true);directory.v.setUint16(28,name.length,true);directory.v.setUint32(42,offset,true);
      central.push(directory.bytes,name);offset+=30+name.length+data.length;size+=46+name.length;
    }
    const end=header(22);end.v.setUint32(0,0x06054b50,true);end.v.setUint16(8,files.length,true);end.v.setUint16(10,files.length,true);end.v.setUint32(12,size,true);end.v.setUint32(16,offset,true);
    return new Blob([...chunks,...central,end.bytes],{type:'application/zip'});
  };
})();
