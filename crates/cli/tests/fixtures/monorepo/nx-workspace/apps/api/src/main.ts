import express from 'express';
import { greet } from '@nx-workspace/shared';

const app = express();
const port = 3000;

app.get('/', (req, res) => {
  res.json({
    message: greet('Nx Workspace API')
  });
});

app.get('/health', (req, res) => {
  res.json({ status: 'healthy' });
});

app.listen(port, () => {
  console.log(`API server running on port ${port}`);
});
