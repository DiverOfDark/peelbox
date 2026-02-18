import Config

config :ecto_worker,
  ecto_repos: [EctoWorker.Repo]

config :ecto_worker, EctoWorker.Repo,
  database: System.get_env("DATABASE_URL", "ecto_worker_dev"),
  pool_size: 10
