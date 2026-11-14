const fs = require('fs');
(async () => {
  const wasm = fs.readFileSync(process.argv[2]);
  const input = fs.readFileSync(process.argv[3]);
  const { instance } = await WebAssembly.instantiate(wasm, {});
  const e = instance.exports;
  const ptr = e.xt_alloc(input.length);
  new Uint8Array(e.memory.buffer, ptr, input.length).set(input);
  const code = e.xt_convert(ptr, input.length);
  e.xt_free(ptr, input.length);
  const info = Buffer.from(e.memory.buffer, e.xt_info_ptr(), e.xt_info_len()).toString('utf8');
  if (code) throw new Error(info);
  const data = Buffer.from(e.memory.buffer, e.xt_output_ptr(), e.xt_output_len());
  if (data.length < 100 || data.subarray(0, 4).toString() !== 'glTF') throw new Error('No GLB output');
  const meta = JSON.parse(info);
  if (!meta.triangles || meta.failed_faces || meta.truncated) throw new Error('Incomplete test model: ' + info);
  console.log(JSON.stringify({bytes:data.length, ...meta}));
})().catch(e => { console.error(e); process.exit(1); });

