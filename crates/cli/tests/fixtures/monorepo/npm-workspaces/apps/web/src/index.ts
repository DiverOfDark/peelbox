import express from 'express';
import { Button } from '@monorepo/ui';
import { api } from '@monorepo/api';

const app = express();
const port = 3000;

app.get('/', (req, res) => {
  res.json({
    message: 'Monorepo Web Server',
    ui: Button(),
    api: api()
  });
});

app.get('/health', (req, res) => {
  res.json({ status: 'healthy' });
});

app.get('/components', (req, res) => {
  res.json({
    button: Button(),
    api: api()
  });
});

app.listen(port, () => {
  console.log(`Monorepo web server running on port ${port}`);
});
