// Quick visual sanity check: samples the screenshot and prints an ASCII luminance
// map plus colour statistics, so a blank or broken window is obvious in text.
const { readFileSync } = require('node:fs');
const { inflateSync } = require('node:zlib');

const file = process.argv[2] ?? 'shot.png';
const buf = readFileSync(file);

let pos = 8;
let width = 0;
let height = 0;
let bitDepth = 0;
let colorType = 0;
const idat = [];

while (pos < buf.length) {
  const len = buf.readUInt32BE(pos);
  const type = buf.toString('ascii', pos + 4, pos + 8);
  const data = buf.subarray(pos + 8, pos + 8 + len);
  if (type === 'IHDR') {
    width = data.readUInt32BE(0);
    height = data.readUInt32BE(4);
    bitDepth = data[8];
    colorType = data[9];
  } else if (type === 'IDAT') {
    idat.push(data);
  } else if (type === 'IEND') {
    break;
  }
  pos += 12 + len;
}

if (bitDepth !== 8) throw new Error(`unsupported bit depth ${bitDepth}`);
const channels = colorType === 6 ? 4 : colorType === 2 ? 3 : colorType === 0 ? 1 : 0;
if (!channels) throw new Error(`unsupported colour type ${colorType}`);

const raw = inflateSync(Buffer.concat(idat));
const stride = width * channels;
const pixels = Buffer.alloc(height * stride);
const bpp = channels;

for (let y = 0; y < height; y++) {
  const filter = raw[y * (stride + 1)];
  const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
  const out = pixels.subarray(y * stride, (y + 1) * stride);
  const prev = y > 0 ? pixels.subarray((y - 1) * stride, y * stride) : null;

  for (let x = 0; x < stride; x++) {
    const a = x >= bpp ? out[x - bpp] : 0;
    const b = prev ? prev[x] : 0;
    const c = prev && x >= bpp ? prev[x - bpp] : 0;
    const v = line[x];
    let value;
    switch (filter) {
      case 0: value = v; break;
      case 1: value = v + a; break;
      case 2: value = v + b; break;
      case 3: value = v + ((a + b) >> 1); break;
      case 4: {
        const p = a + b - c;
        const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        value = v + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c);
        break;
      }
      default: throw new Error(`bad filter ${filter}`);
    }
    out[x] = value & 0xff;
  }
}

const colours = new Set();
const COLS = 64;
const ROWS = 24;
const grid = Array.from({ length: ROWS }, () => Array(COLS).fill(0));
const counts = Array.from({ length: ROWS }, () => Array(COLS).fill(0));

for (let y = 0; y < height; y++) {
  for (let x = 0; x < width; x++) {
    const i = y * stride + x * channels;
    const r = pixels[i], g = pixels[i + 1] ?? r, b = pixels[i + 2] ?? r;
    colours.add((r << 16) | (g << 8) | b);
    const gy = Math.min(ROWS - 1, Math.floor((y / height) * ROWS));
    const gx = Math.min(COLS - 1, Math.floor((x / width) * COLS));
    grid[gy][gx] += 0.2126 * r + 0.7152 * g + 0.0722 * b;
    counts[gy][gx] += 1;
  }
}

const ramp = ' .:-=+*#%@';
console.log(`size ${width}x${height}  unique colours ${colours.size}`);
for (let y = 0; y < ROWS; y++) {
  let line = '';
  for (let x = 0; x < COLS; x++) {
    const avg = counts[y][x] ? grid[y][x] / counts[y][x] : 0;
    line += ramp[Math.min(ramp.length - 1, Math.floor((avg / 255) * ramp.length))];
  }
  console.log(line);
}
