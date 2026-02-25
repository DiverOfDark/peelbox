import express from "express";
import { PrismaClient } from "@prisma/client";

const app = express();
const port = process.env.PORT || 3000;

let prisma: PrismaClient | null = null;

function getPrisma(): PrismaClient {
  if (!prisma) {
    prisma = new PrismaClient();
  }
  return prisma;
}

app.use(express.json());

app.get("/health", (_req, res) => {
  res.json({ status: "ok" });
});

app.get("/users", async (_req, res) => {
  const users = await getPrisma().user.findMany({
    include: { posts: true },
  });
  res.json(users);
});

app.post("/users", async (req, res) => {
  const { email, name } = req.body;
  const user = await getPrisma().user.create({
    data: { email, name },
  });
  res.status(201).json(user);
});

app.get("/posts", async (_req, res) => {
  const posts = await getPrisma().post.findMany({
    where: { published: true },
    include: { author: true },
  });
  res.json(posts);
});

app.listen(port, () => {
  console.log(`Server running on port ${port}`);
});
