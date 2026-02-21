import express from 'express';

// eslint-disable-next-line @typescript-eslint/no-var-requires
const { Button } = require('@monorepo/ui');
// eslint-disable-next-line @typescript-eslint/no-var-requires
const { api } = require('@monorepo/api');

const app = express();
const port = 3000;

app.get('/', (_req: express.Request, res: express.Response) => {
  res.json({
    message: 'Monorepo Web Server',
    ui: Button(),
    api: api()
  });
});

app.get('/health', (_req: express.Request, res: express.Response) => {
  res.json({ status: 'healthy' });
});

app.get('/components', (_req: express.Request, res: express.Response) => {
  res.json({
    button: Button(),
    api: api()
  });
});

app.listen(port, () => {
  console.log(`Monorepo web server running on port ${port}`);
});
