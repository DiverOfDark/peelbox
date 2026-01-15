using Microsoft.AspNetCore.Mvc;

var builder = WebApplication.CreateBuilder(args);
var app = builder.Build();

var users = new List<User>
{
    new User { Id = 1, Name = "Alice", Email = "alice@example.com" },
    new User { Id = 2, Name = "Bob", Email = "bob@example.com" }
};

app.MapGet("/", () => new
{
    message = "User API Server",
    version = "1.0.0",
    endpoints = new[] { "/users", "/users/{id}", "/health" }
});

app.MapGet("/health", () => new { status = "healthy" });

app.MapGet("/users", () => new { users });

app.MapGet("/users/{id}", (int id) =>
{
    var user = users.FirstOrDefault(u => u.Id == id);
    return user != null
        ? Results.Ok(new { user })
        : Results.NotFound(new { error = "User not found" });
});

app.MapPost("/users", ([FromBody] User user) =>
{
    user.Id = users.Count + 1;
    users.Add(user);
    return Results.Created($"/users/{user.Id}", new { user });
});

app.Run();

record User
{
    public int Id { get; set; }
    public string Name { get; set; } = "";
    public string Email { get; set; } = "";
}
