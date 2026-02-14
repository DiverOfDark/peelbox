#include <iostream>
#include <string>
#include <cstring>
#include <cstdlib>
#include <sstream>

#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>

std::string get_env(const char* name, const char* default_value) {
    const char* value = std::getenv(name);
    return value ? value : default_value;
}

std::string handle_request(const std::string& path) {
    if (path == "/") {
        return "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"message\":\"Hello\"}";
    } else if (path == "/health") {
        return "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"healthy\"}";
    }
    return "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\n\r\n{\"error\":\"Not found\"}";
}

int main() {
    std::string port_str = get_env("PORT", "8080");
    int port = std::stoi(port_str);

    int server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd < 0) return 1;

    int opt = 1;
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

    struct sockaddr_in address;
    memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_addr.s_addr = INADDR_ANY;
    address.sin_port = htons(static_cast<uint16_t>(port));

    if (bind(server_fd, reinterpret_cast<struct sockaddr*>(&address), sizeof(address)) < 0) {
        close(server_fd);
        return 1;
    }

    listen(server_fd, 16);
    std::cout << "Listening on port " << port << std::endl;

    for (;;) {
        struct sockaddr_in client_addr;
        socklen_t client_len = sizeof(client_addr);
        int client_fd = accept(server_fd,
                               reinterpret_cast<struct sockaddr*>(&client_addr),
                               &client_len);
        if (client_fd < 0) continue;

        char buffer[4096] = {};
        recv(client_fd, buffer, sizeof(buffer) - 1, 0);

        std::string request(buffer);
        std::istringstream stream(request);
        std::string method, path, version;
        stream >> method >> path >> version;

        std::string response = handle_request(path);
        send(client_fd, response.c_str(), response.size(), 0);
        close(client_fd);
    }

    close(server_fd);
    return 0;
}
