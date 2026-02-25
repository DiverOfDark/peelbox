import express from 'express';
import { greet } from '@bun-monorepo/shared';

const app = express();
const port = 3000;

app.get('/', (_req: express.Request, res: express.Response) => {
  res.json({
    message: greet('World'),
    service: 'api'
  });
});

app.get('/health', (_req: express.Request, res: express.Response) => {
  res.json({ status: 'healthy' });
});

app.listen(port, () => {
  console.log(`API server running on port ${port}`);
});
