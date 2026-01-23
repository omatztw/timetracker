import pngToIco from 'png-to-ico';
import { readFileSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const iconsDir = join(__dirname, '../src-tauri/icons');

// Use multiple sizes for ICO (Windows uses different sizes in different contexts)
const pngFiles = [
  join(iconsDir, '32x32.png'),
  join(iconsDir, '128x128.png'),
  join(iconsDir, '128x128@2x.png'), // 256x256
];

console.log('Generating icon.ico...');

const icoBuffer = await pngToIco(pngFiles);
writeFileSync(join(iconsDir, 'icon.ico'), icoBuffer);

console.log('icon.ico generated successfully!');
