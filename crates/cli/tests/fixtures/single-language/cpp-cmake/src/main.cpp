#include <iostream>
#include <string>
#include <cstring>
#include <cstdlib>
#include <thread>
#include <sstream>

#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>
#include <arpa/inet.h>

std::string get_env(const char* name, const char* default_value) {
    const char* value = std::getenv(name);
    return value ? value : default_value;
}

std::string build_http_response(int status_code, const std::string& status_text,
                                const std::string& body) {
    std::ostringstream response;
    response << "HTTP/1.1 " << status_code << " " << status_text << "\r\n";
    response << "Content-Type: application/json\r\n";
    response << "Content-Length: " << body.size() << "\r\n";
    response << "Connection: close\r\n";
    response << "Server: C++ API\r\n";
    response << "\r\n";
    response << body;
    return response.str();
}

std::string handle_request(const std::string& method, const std::string& path) {
    if (method != "GET") {
        std::string body = R"({"error":"Method not allowed"})";
        return build_http_response(405, "Method Not Allowed", body);
    }

    if (path == "/") {
        std::string body = R"({"message":"C++ API Server","version":"1.0.0","endpoints":["/","/health","/users"]})";
        return build_http_response(200, "OK", body);
    } else if (path == "/health") {
        std::string body = R"({"status":"healthy","uptime":12345})";
        return build_http_response(200, "OK", body);
    } else if (path == "/users") {
        std::string body = R"({"users":[{"id":1,"name":"Alice","email":"alice@example.com"},{"id":2,"name":"Bob","email":"bob@example.com"}]})";
        return build_http_response(200, "OK", body);
    }

    std::string body = R"({"error":"Not found"})";
    return build_http_response(404, "Not Found", body);
}

bool parse_request_line(const std::string& request, std::string& method, std::string& path) {
    // Parse "GET /path HTTP/1.1\r\n..."
    std::istringstream stream(request);
    std::string http_version;
    if (!(stream >> method >> path >> http_version)) {
        return false;
    }
    return true;
}

void handle_client(int client_fd) {
    char buffer[4096];
    memset(buffer, 0, sizeof(buffer));

    ssize_t bytes_read = recv(client_fd, buffer, sizeof(buffer) - 1, 0);
    if (bytes_read <= 0) {
        close(client_fd);
        return;
    }

    std::string request(buffer, bytes_read);
    std::string method, path;

    std::string response;
    if (parse_request_line(request, method, path)) {
        response = handle_request(method, path);
    } else {
        std::string body = R"({"error":"Bad request"})";
        response = build_http_response(400, "Bad Request", body);
    }

    ssize_t total_sent = 0;
    ssize_t to_send = static_cast<ssize_t>(response.size());
    while (total_sent < to_send) {
        ssize_t sent = send(client_fd, response.c_str() + total_sent,
                            to_send - total_sent, 0);
        if (sent <= 0) {
            break;
        }
        total_sent += sent;
    }

    close(client_fd);
}

int main() {
    std::string port_str = get_env("PORT", "8080");
    int port = std::stoi(port_str);

    int server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd < 0) {
        std::cerr << "Error: Failed to create socket" << std::endl;
        return EXIT_FAILURE;
    }

    int opt = 1;
    if (setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) < 0) {
        std::cerr << "Error: Failed to set socket options" << std::endl;
        close(server_fd);
        return EXIT_FAILURE;
    }

    struct sockaddr_in address;
    memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_addr.s_addr = INADDR_ANY;
    address.sin_port = htons(static_cast<uint16_t>(port));

    if (bind(server_fd, reinterpret_cast<struct sockaddr*>(&address), sizeof(address)) < 0) {
        std::cerr << "Error: Failed to bind to port " << port << std::endl;
        close(server_fd);
        return EXIT_FAILURE;
    }

    if (listen(server_fd, 16) < 0) {
        std::cerr << "Error: Failed to listen" << std::endl;
        close(server_fd);
        return EXIT_FAILURE;
    }

    std::cout << "Server listening on port " << port << std::endl;

    for (;;) {
        struct sockaddr_in client_addr;
        socklen_t client_len = sizeof(client_addr);
        int client_fd = accept(server_fd,
                               reinterpret_cast<struct sockaddr*>(&client_addr),
                               &client_len);
        if (client_fd < 0) {
            std::cerr << "Error: Failed to accept connection" << std::endl;
            continue;
        }

        std::thread(handle_client, client_fd).detach();
    }

    close(server_fd);
    return 0;
}
