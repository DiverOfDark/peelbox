const puppeteer = require('puppeteer');
const express = require('express');

const app = express();
const PORT = 3000;

app.get('/screenshot', async (req, res) => {
  const browser = await puppeteer.launch({
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });
  const page = await browser.newPage();
  await page.goto(req.query.url || 'https://example.com');
  const screenshot = await page.screenshot();
  await browser.close();
  res.type('image/png').send(screenshot);
});

app.get('/health', (req, res) => {
  res.json({ status: 'ok' });
});

app.listen(PORT, () => {
  console.log(`Server running on port ${PORT}`);
});
