defmodule EctoWorker.Application do
  use Application

  @impl true
  def start(_type, _args) do
    port = System.get_env("PORT", "4000") |> String.to_integer()

    children = [
      {Plug.Cowboy, scheme: :http, plug: EctoWorker.Router, options: [port: port]}
    ]

    opts = [strategy: :one_for_one, name: EctoWorker.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
