# Test Python Project

A sample Python project for testing aipack detection capabilities.

## Installation

```bash
pip install -e .
```

Or with development dependencies:

```bash
pip install -e ".[dev]"
```

## Testing

```bash
pytest
```

With coverage:

```bash
pytest --cov
```

## Building

```bash
python -m build
```

## Code Quality

```bash
# Format code
black src tests

# Sort imports
isort src tests

# Lint
ruff src tests

# Type check
mypy src
```
