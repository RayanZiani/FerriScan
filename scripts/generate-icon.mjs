/**
 * Generates the FerriScan application icon as a 1024x1024 PNG.
 *
 * The mark is a ferrite core: a dark rounded square, a disc ring, and a radial
 * wedge that suggests a scan sweeping the surface. Rendered procedurally so the
 * repository stays free of binary assets until `tauri icon` expands it.
 */
import { deflateSync } from 'node:zlib';
import { writeFileSync } from 'node:fs';

const SIZE = 1024;

function crc32(buf) {
  let c;
  const table = [];
  for (let n = 0; n < 256; n++) {
    c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  let crc = 0xffffffff;
  for (const byte of buf) crc = table[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeBuf = Buffer.from(type, 'ascii');
  const body = Buffer.concat([typeBuf, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

/** Signed distance to a rounded rectangle. */
function sdRoundRect(px, py, cx, cy, halfW, halfH, r) {
  const dx = Math.abs(px - cx) - (halfW - r);
  const dy = Math.abs(py - cy) - (halfH - r);
  const ax = Math.max(dx, 0);
  const ay = Math.max(dy, 0);
  return Math.sqrt(ax * ax + ay * ay) + Math.min(Math.max(dx, dy), 0) - r;
}

const pixels = Buffer.alloc(SIZE * SIZE * 4);
const cx = SIZE / 2;
const cy = SIZE / 2;

for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const i = (y * SIZE + x) * 4;
    let r = 0;
    let g = 0;
    let b = 0;
    let a = 0;

    // Rounded-square plate with a vertical gradient (deep slate to near black).
    const plate = sdRoundRect(x, y, cx, cy, 440, 440, 210);
    if (plate < 0) {
      const t = (y - (cy - 440)) / 880;
      r = Math.round(26 + 14 * (1 - t));
      g = Math.round(32 + 18 * (1 - t));
      b = Math.round(44 + 30 * (1 - t));
      a = 255;
    }

    // Ferrite core ring.
    const dist = Math.hypot(x - cx, y - cy);
    const ring = Math.abs(dist - 250);
    if (ring < 46) {
      const fade = Math.min(1, (46 - ring) / 10);
      const inner = Math.round(90 + 60 * (1 - dist / 512));
      r = Math.round(r * (1 - fade) + inner * fade);
      g = Math.round(g * (1 - fade) + (inner + 30) * fade);
      b = Math.round(b * (1 - fade) + 255 * fade);
    }

    // Radial wedge: suggests a scan sector sweeping the disc.
    const angle = Math.atan2(y - cy, x - cx);
    const norm = (angle + Math.PI * 2) % (Math.PI * 2);
    if (dist < 250 && norm > 4.6 && norm < 5.55) {
      const t = (norm - 4.6) / 0.95;
      r = Math.round(37 + 40 * (1 - t));
      g = Math.round(99 + 40 * (1 - t));
      b = Math.round(235 + 20 * (1 - t));
    }

    // Center hub.
    if (dist < 52) {
      const k = 1 - dist / 52;
      r = Math.round(r * (1 - k) + 235 * k);
      g = Math.round(g * (1 - k) + 240 * k);
      b = Math.round(b * (1 - k) + 255 * k);
    }

    pixels[i] = r;
    pixels[i + 1] = g;
    pixels[i + 2] = b;
    pixels[i + 3] = a;
  }
}

// Filter byte 0 per scanline.
const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0;
  pixels.copy(raw, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);

writeFileSync(process.argv[2] ?? 'icon.png', png);
