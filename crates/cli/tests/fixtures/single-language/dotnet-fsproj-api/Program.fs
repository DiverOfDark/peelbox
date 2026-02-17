open System
open Microsoft.AspNetCore.Builder

let builder = WebApplication.CreateBuilder()
let app = builder.Build()

app.MapGet("/", Func<string>(fun () -> "F# API Server")) |> ignore
app.MapGet("/health", Func<string>(fun () -> "ok")) |> ignore

app.Run()
