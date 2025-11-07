"""Command-line interface for test project."""

import click

@click.command()
@click.option("--name", default="World", help="Name to greet")
def main(name: str) -> None:
    """Simple CLI application."""
    click.echo(f"Hello, {name}!")

if __name__ == "__main__":
    main()
