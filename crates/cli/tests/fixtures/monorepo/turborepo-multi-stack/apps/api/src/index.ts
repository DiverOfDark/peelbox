import express from "express";

const app = express();
const PORT = 3000;

app.get("/", (req, res) => {
  res.json({ message: "Hello from Express API" });
});

app.get("/health", (req, res) => {
  res.json({ status: "healthy" });
});

app.listen(PORT, () => {
  console.log(`API server running on port ${PORT}`);
});
