#include <iostream>
#include <string>
#include <boost/asio.hpp>
#include <boost/beast.hpp>
#include <cstdlib>

namespace beast = boost::beast;
namespace http = beast::http;
namespace net = boost::asio;
using tcp = net::ip::tcp;

std::string get_env(const char* name, const char* default_value) {
    const char* value = std::getenv(name);
    return value ? value : default_value;
}

std::string handle_request(const std::string& target) {
    if (target == "/") {
        return R"({"message":"C++ API Server","version":"1.0.0","endpoints":["/","/health","/users"]})";
    } else if (target == "/health") {
        return R"({"status":"healthy","uptime":12345})";
    } else if (target == "/users") {
        return R"({"users":[{"id":1,"name":"Alice","email":"alice@example.com"},{"id":2,"name":"Bob","email":"bob@example.com"}]})";
    }
    return R"({"error":"Not found"})";
}

void handle_session(tcp::socket socket) {
    try {
        beast::flat_buffer buffer;
        http::request<http::string_body> req;
        http::read(socket, buffer, req);

        std::string body = handle_request(std::string(req.target()));

        http::response<http::string_body> res{http::status::ok, req.version()};
        res.set(http::field::server, "C++ API");
        res.set(http::field::content_type, "application/json");
        res.body() = body;
        res.prepare_payload();

        http::write(socket, res);
    } catch (std::exception const& e) {
        std::cerr << "Error: " << e.what() << std::endl;
    }
}

int main() {
    try {
        std::string port_str = get_env("PORT", "8080");
        std::string db_url = get_env("DATABASE_URL", "postgres://localhost/myapp");

        int port = std::stoi(port_str);

        net::io_context ioc{1};
        tcp::acceptor acceptor{ioc, {tcp::v4(), static_cast<unsigned short>(port)}};

        std::cout << "Server listening on port " << port << std::endl;

        for (;;) {
            tcp::socket socket{ioc};
            acceptor.accept(socket);
            std::thread{std::bind(handle_session, std::move(socket))}.detach();
        }
    } catch (std::exception const& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return EXIT_FAILURE;
    }
}
