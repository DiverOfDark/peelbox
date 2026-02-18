import Config

config :ecto_worker, EctoWorker.Repo,
  url: System.get_env("DATABASE_URL"),
  pool_size: 10

config :ecto_worker,
  ecto_repos: [EctoWorker.Repo]
