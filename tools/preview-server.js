// Раздаёт папку ui для просмотра интерфейса в браузере, без сборки приложения.
// В браузере служба недоступна, поэтому app.js подставляет демонстрационные данные.
const http = require('http');
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..', 'ui');
const port = Number(process.argv[2]) || 5173;
const types = { '.wasm':'application/wasm', '.webm': 'video/webm', '.ttf': 'font/ttf', '.otf': 'font/otf', '.woff2': 'font/woff2', '.gif': 'image/gif', '.png': 'image/png', '.webp': 'image/webp', '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8', '.js': 'text/javascript; charset=utf-8' };

http.createServer((request, response) => {
  const name = path.normalize(decodeURIComponent(request.url.split('?')[0])).replace(/^[\\/]+/, '') || 'index.html';
  const file = path.join(root, name);
  if (!file.startsWith(root)) { response.writeHead(403).end(); return; }
  fs.readFile(file, (error, data) => {
    if (error) { response.writeHead(404).end('not found'); return; }
    response.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(data);
  });
}).listen(port, '127.0.0.1', () => console.log(`http://localhost:${port}`));
