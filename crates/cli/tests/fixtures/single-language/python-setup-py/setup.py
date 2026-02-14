from setuptools import setup, find_packages

setup(
    name="myapp",
    version="1.0.0",
    description="Example Python application",
    packages=find_packages(),
    install_requires=[
        "flask>=3.0.0",
        "requests>=2.31.0",
    ],
    python_requires=">=3.9",
)
