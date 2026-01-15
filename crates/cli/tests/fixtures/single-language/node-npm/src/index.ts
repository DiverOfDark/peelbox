import express from 'express';

const app = express();
const port = 3000;

app.use(express.json());

interface User {
  id: number;
  name: string;
  email: string;
}

const users: User[] = [
  { id: 1, name: 'Alice', email: 'alice@example.com' },
  { id: 2, name: 'Bob', email: 'bob@example.com' },
];

app.get('/', (req, res) => {
  res.json({
    message: 'User API Server',
    version: '1.0.0',
    endpoints: ['/users', '/users/:id', '/health'],
  });
});

app.get('/health', (req, res) => {
  res.json({ status: 'healthy', uptime: process.uptime() });
});

app.get('/users', (req, res) => {
  res.json({ users });
});

app.get('/users/:id', (req, res) => {
  const user = users.find(u => u.id === parseInt(req.params.id));
  if (user) {
    res.json({ user });
  } else {
    res.status(404).json({ error: 'User not found' });
  }
});

app.post('/users', (req, res) => {
  const newUser: User = {
    id: users.length + 1,
    name: req.body.name,
    email: req.body.email,
  };
  users.push(newUser);
  res.status(201).json({ user: newUser });
});

app.listen(port, () => {
  console.log(`Server running on port ${port}`);
});
