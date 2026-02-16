import { APP_BASE_HREF } from '@angular/common';
import { CommonEngine } from '@angular/ssr/node';
import express from 'express';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const serverDistFolder = dirname(fileURLToPath(import.meta.url));
const browserDistFolder = resolve(serverDistFolder, '../browser');

const app = express();
const commonEngine = new CommonEngine();

app.get('**', express.static(browserDistFolder, { maxAge: '1y', index: 'index.html' }));

app.get('**', (req, res, next) => {
  const { protocol, originalUrl, baseUrl, headers } = req;
  commonEngine
    .render({
      documentFilePath: join(browserDistFolder, 'index.html'),
      url: `${protocol}://${headers.host}${originalUrl}`,
      providers: [{ provide: APP_BASE_HREF, useValue: baseUrl }],
    })
    .then((html) => res.send(html))
    .catch((err) => next(err));
});

const port = process.env['PORT'] || 4000;
app.listen(port, () => {
  console.log(`Node Express server listening on http://localhost:${port}`);
});
