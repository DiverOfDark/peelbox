defmodule ElixirApi.Application do
  use Application
  require Logger

  def start(_type, _args) do
    port = String.to_integer(System.get_env("PORT") || "4000")
    Logger.info("Starting server on port #{port}")

    children = [
      {Task, fn -> listen(port) end}
    ]

    opts = [strategy: :one_for_one, name: ElixirApi.Supervisor]
    Supervisor.start_link(children, opts)
  end

  def listen(port) do
    {:ok, socket} = :gen_tcp.listen(port, [:binary, packet: :raw, active: false, reuseaddr: true])
    Logger.info("Listening on #{port}")
    accept_loop(socket)
  end

  def accept_loop(socket) do
    {:ok, client} = :gen_tcp.accept(socket)
    Task.start(fn -> handle_client(client) end)
    accept_loop(socket)
  end

  def handle_client(client) do
    case :gen_tcp.recv(client, 0) do
      {:ok, _data} ->
        resp = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"
        :gen_tcp.send(client, resp)
        :gen_tcp.close(client)
      _ ->
        :gen_tcp.close(client)
    end
  end
end
