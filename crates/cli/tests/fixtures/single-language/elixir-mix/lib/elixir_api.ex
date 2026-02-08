defmodule ElixirApi do
  use Plug.Router

  plug :match
  plug :dispatch

  get "/" do
    response = %{
      message: "Elixir API Server",
      version: "1.0.0",
      endpoints: ["/", "/health", "/users"]
    }
    send_json(conn, 200, response)
  end

  get "/health" do
    response = %{
      status: "healthy",
      uptime: :erlang.system_time(:second)
    }
    send_json(conn, 200, response)
  end

  get "/users" do
    users = [
      %{id: 1, name: "Alice", email: "alice@example.com"},
      %{id: 2, name: "Bob", email: "bob@example.com"}
    ]
    send_json(conn, 200, %{users: users})
  end

  match _ do
    send_json(conn, 404, %{error: "Not found"})
  end

  defp send_json(conn, status, data) do
    conn
    |> put_resp_content_type("application/json")
    |> send_resp(status, Jason.encode!(data))
  end
end

defmodule ElixirApi.Application do
  use Application

  def start(_type, _args) do
    port = System.get_env("PORT", "4000") |> String.to_integer()

    IO.puts("Starting server on port #{port}")

    children = [
      {Plug.Cowboy, scheme: :http, plug: ElixirApi, options: [port: port]}
    ]

    opts = [strategy: :one_for_one, name: ElixirApi.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
