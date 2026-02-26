#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>

const char *get_env(const char *name, const char *default_value) {
    const char *value = getenv(name);
    return value ? value : default_value;
}

void handle_request(int client_fd, const char *path) {
    const char *response;
    if (strcmp(path, "/") == 0) {
        response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"message\":\"Hello from C\"}";
    } else if (strcmp(path, "/health") == 0) {
        response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"healthy\"}";
    } else {
        response = "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\n\r\n{\"error\":\"Not found\"}";
    }
    send(client_fd, response, strlen(response), 0);
}

int main(void) {
    const char *port_str = get_env("PORT", "8080");
    int port = atoi(port_str);

    int server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd < 0) return 1;

    int opt = 1;
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

    struct sockaddr_in address;
    memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_addr.s_addr = INADDR_ANY;
    address.sin_port = htons((uint16_t)port);

    if (bind(server_fd, (struct sockaddr *)&address, sizeof(address)) < 0) {
        close(server_fd);
        return 1;
    }

    listen(server_fd, 16);
    printf("Listening on port %d\n", port);

    for (;;) {
        struct sockaddr_in client_addr;
        socklen_t client_len = sizeof(client_addr);
        int client_fd = accept(server_fd, (struct sockaddr *)&client_addr, &client_len);
        if (client_fd < 0) continue;

        char buffer[4096] = {0};
        recv(client_fd, buffer, sizeof(buffer) - 1, 0);

        /* Parse HTTP request line: "GET /path HTTP/1.1" */
        char method[16] = {0}, path[256] = {0};
        sscanf(buffer, "%15s %255s", method, path);

        handle_request(client_fd, path);
        close(client_fd);
    }

    close(server_fd);
    return 0;
}
