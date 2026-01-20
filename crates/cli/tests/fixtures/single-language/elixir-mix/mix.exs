defmodule ElixirApi.MixProject do
  use Mix.Project

  def project do
    [
      app: :elixir_api,
      version: "1.0.0",
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {ElixirApi.Application, []}
    ]
  end

  defp deps do
    [
    ]
  end
end
