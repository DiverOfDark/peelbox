const port = 8000;

const handler = (request: Request): Response => {
  const url = new URL(request.url);

  if (url.pathname === "/") {
    return new Response(
      JSON.stringify({ message: "Hello from Deno 2", version: "2.0.0" }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  }

  if (url.pathname === "/health") {
    return new Response(
      JSON.stringify({ status: "healthy" }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  }

  return new Response("Not Found", { status: 404 });
};

console.log(`Server running on port ${port}`);
Deno.serve({ port }, handler);
