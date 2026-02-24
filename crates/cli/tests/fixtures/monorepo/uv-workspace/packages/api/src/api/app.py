from flask import Flask

app = Flask(__name__)


@app.route("/health")
def health():
    return {"status": "ok"}


@app.route("/")
def index():
    return {"message": "Hello from API"}
