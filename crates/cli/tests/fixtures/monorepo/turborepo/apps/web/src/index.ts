import express from 'express';

const app = express();
const port = 3000;

app.get('/', (req, res) => {
  res.json({
    message: 'Monorepo Web Server',
  });
});

app.get('/health', (req, res) => {
  res.json({ status: 'healthy' });
});

app.listen(port, () => {
  console.log(`Monorepo web server running on port ${port}`);
});
