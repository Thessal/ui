const fs = require('fs');
const path = require('path');

const src = path.join(__dirname, 'node_modules', 'lightweight-charts', 'dist', 'lightweight-charts.standalone.production.js');
const destDir = path.join(__dirname, 'media');
const dest = path.join(destDir, 'lightweight-charts.standalone.production.js');

if (!fs.existsSync(destDir)) {
    fs.mkdirSync(destDir);
}

fs.copyFileSync(src, dest);
console.log(`Copied ${src} to ${dest}`);
