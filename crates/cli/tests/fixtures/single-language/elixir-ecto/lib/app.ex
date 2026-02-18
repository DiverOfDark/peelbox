defmodule EctoWorker.Application do
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      EctoWorker.Repo
    ]

    opts = [strategy: :one_for_one, name: EctoWorker.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
