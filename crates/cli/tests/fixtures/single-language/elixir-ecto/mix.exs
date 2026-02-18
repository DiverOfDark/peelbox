defmodule EctoWorker.MixProject do
  use Mix.Project

  def project do
    [
      app: :ecto_worker,
      version: "0.1.0",
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {EctoWorker.Application, []}
    ]
  end

  defp deps do
    [
      {:ecto, "~> 3.11"},
      {:ecto_sql, "~> 3.11"},
      {:postgrex, "~> 0.18"},
      {:jason, "~> 1.4"}
    ]
  end
end
