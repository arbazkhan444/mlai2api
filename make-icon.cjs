// Generates a multi-size Windows .ico + 256px png for the app icon.
const fs = require('fs');
const dir = 'C:/Users/Lenovo/Desktop/mlai-gui/crates/app/icons';
fs.mkdirSync(dir, { recursive: true });

function pixels(size) {
  const xor = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = ((size - 1 - y) * size + x) * 4;
      const t = (x + y) / (2 * size);
      let r = Math.round(79 + (125 - 79) * t);
      let g = Math.round(140 + (92 - 140) * t);
      let b = 255;
      const cx = x - size / 2 + 0.5, cy = y - size / 2 + 0.5;
      const radius = size * 0.42;
      const d = Math.max(Math.abs(cx), Math.abs(cy));
      const inside = d < radius;
      const stripe = Math.abs(cx - cy) < size * 0.075;
      if (inside) {
        if (stripe) { r = 255; g = 255; b = 255; }
        xor[i] = b; xor[i + 1] = g; xor[i + 2] = r; xor[i + 3] = 255;
      } else { xor[i + 3] = 0; }
    }
  }
  return xor;
}

function dib(size) {
  const xor = pixels(size);
  const maskRow = Math.ceil(size / 32) * 4;
  const mask = Buffer.alloc(size * maskRow);
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8);
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  header.writeUInt32LE(0, 16);
  header.writeUInt32LE(xor.length, 20);
  return Buffer.concat([header, xor, mask]);
}

const sizes = [16, 24, 32, 48, 64, 128, 256];
const images = sizes.map(dib);
const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(sizes.length, 4);
let offset = 6 + 16 * sizes.length;
const entries = [];
for (let i = 0; i < sizes.length; i++) {
  const size = sizes[i];
  const entry = Buffer.alloc(16);
  entry[0] = size === 256 ? 0 : size;
  entry[1] = size === 256 ? 0 : size;
  entry[2] = 0;
  entry[3] = 0;
  entry.writeUInt16LE(1, 4);
  entry.writeUInt16LE(32, 6);
  entry.writeUInt32LE(images[i].length, 8);
  entry.writeUInt32LE(offset, 12);
  offset += images[i].length;
  entries.push(entry);
}
const ico = Buffer.concat([header, ...entries, ...images]);
fs.writeFileSync(dir + '/icon.ico', ico);

// PNG for non-Windows / docs: 256x256, 8-bit RGBA, single IDAT
const zlib = require('zlib');
function png(size) {
  const rgba = pixels(size);
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    rgba.copy(raw, y * (size * 4 + 1) + 1, (size - 1 - y) * size * 4, (size - y) * size * 4);
  }
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length, 0);
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const crcTable = [];
    for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; crcTable[n] = c >>> 0; }
    let crc = 0xffffffff;
    for (const byte of body) crc = crcTable[(crc ^ byte) & 0xff] ^ (crc >>> 8);
    const crcBuf = Buffer.alloc(4); crcBuf.writeUInt32BE((crc ^ 0xffffffff) >>> 0, 0);
    return Buffer.concat([len, body, crcBuf]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0); ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}
fs.writeFileSync(dir + '/icon.png', png(256));
console.log('icon.ico bytes=' + ico.length + ' sizes=' + sizes.join(',') + ' png=' + fs.statSync(dir + '/icon.png').size);
