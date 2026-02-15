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
        std::string body = R"({"message":"Meson C++ Server","version":"1.0.0"})";
        return build_http_response(200, "OK", body);
    } else if (path == "/health") {
        std::string body = R"({"status":"healthy"})";
        return build_http_response(200, "OK", body);
    }

    std::string body = R"({"error":"Not found"})";
    return build_http_response(404, "Not Found", body);
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
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

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

    listen(server_fd, 16);
    std::cout << "Server listening on port " << port << std::endl;

    for (;;) {
        struct sockaddr_in client_addr;
        socklen_t client_len = sizeof(client_addr);
        int client_fd = accept(server_fd,
                               reinterpret_cast<struct sockaddr*>(&client_addr),
                               &client_len);
        if (client_fd < 0) continue;

        char buffer[4096];
        memset(buffer, 0, sizeof(buffer));
        recv(client_fd, buffer, sizeof(buffer) - 1, 0);

        std::string request(buffer);
        std::istringstream stream(request);
        std::string method, path, version;
        stream >> method >> path >> version;

        std::string response = handle_request(method, path);
        send(client_fd, response.c_str(), response.size(), 0);
        close(client_fd);
    }

    close(server_fd);
    return 0;
}
