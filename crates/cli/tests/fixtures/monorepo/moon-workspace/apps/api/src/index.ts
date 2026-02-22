import express from 'express';

// eslint-disable-next-line @typescript-eslint/no-var-requires
const { greet } = require('@moon-workspace/shared');

const app = express();
const port = 3000;

app.get('/', (_req: express.Request, res: express.Response) => {
  res.json({
    message: 'Moon Workspace API',
    greeting: greet()
  });
});

app.get('/health', (_req: express.Request, res: express.Response) => {
  res.json({ status: 'healthy' });
});

app.listen(port, () => {
  console.log(`Moon workspace API running on port ${port}`);
});
