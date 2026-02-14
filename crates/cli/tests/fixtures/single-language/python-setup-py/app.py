from flask import Flask, jsonify, request

app = Flask(__name__)

users = [
    {"id": 1, "name": "Alice", "email": "alice@example.com"},
    {"id": 2, "name": "Bob", "email": "bob@example.com"},
]

@app.route("/")
def index():
    return jsonify({
        "message": "User API Server",
        "version": "1.0.0",
        "endpoints": ["/users", "/users/<id>", "/health"]
    })

@app.route("/health")
def health():
    return jsonify({"status": "healthy"})

@app.route("/users")
def get_users():
    return jsonify({"users": users})

@app.route("/users/<int:user_id>")
def get_user(user_id):
    user = next((u for u in users if u["id"] == user_id), None)
    if user:
        return jsonify({"user": user})
    return jsonify({"error": "User not found"}), 404

@app.route("/users", methods=["POST"])
def create_user():
    data = request.get_json()
    new_user = {
        "id": len(users) + 1,
        "name": data.get("name"),
        "email": data.get("email")
    }
    users.append(new_user)
    return jsonify({"user": new_user}), 201

if __name__ == "__main__":
    app.run(debug=True)
