defmodule EctoWorker.Repo do
  use Ecto.Repo,
    otp_app: :ecto_worker,
    adapter: Ecto.Adapters.Postgres
end
