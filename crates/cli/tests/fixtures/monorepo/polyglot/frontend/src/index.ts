const express = require('express');
const app = express();
const port = 3000;

app.use(express.json());

const pages = [
  { id: 1, title: 'Home', content: 'Welcome to the frontend' },
  { id: 2, title: 'About', content: 'About our polyglot application' },
];

app.get('/', (req, res) => {
  res.json({
    service: 'Frontend',
    language: 'JavaScript',
    endpoints: ['/', '/health', '/pages'],
  });
});

app.get('/health', (req, res) => {
  res.json({ status: 'healthy' });
});

app.get('/pages', (req, res) => {
  res.json({ pages });
});

app.get('/pages/:id', (req, res) => {
  const page = pages.find(p => p.id === parseInt(req.params.id));
  if (page) {
    res.json({ page });
  } else {
    res.status(404).json({ error: 'Page not found' });
  }
});

app.post('/pages', (req, res) => {
  const newPage = {
    id: pages.length + 1,
    title: req.body.title,
    content: req.body.content,
  };
  pages.push(newPage);
  res.status(201).json({ page: newPage });
});

app.listen(port, () => {
  console.log(`Frontend server running on port ${port}`);
});
