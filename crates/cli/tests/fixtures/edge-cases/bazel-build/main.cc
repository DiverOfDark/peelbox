#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <signal.h>

static const char* json_response(const char* target, int* status_code) {
    if (strcmp(target, "/") == 0 || strcmp(target, "") == 0) {
        *status_code = 200;
        return "{\"message\":\"Bazel C++ Server\",\"version\":\"1.0.0\"}";
    }
    if (strcmp(target, "/health") == 0) {
        *status_code = 200;
        return "{\"status\":\"healthy\"}";
    }
    *status_code = 404;
    return "{\"error\":\"not found\"}";
}

static void handle_client(int client_fd) {
    char buf[4096];
    ssize_t n = read(client_fd, buf, sizeof(buf) - 1);
    if (n <= 0) {
        close(client_fd);
        return;
    }
    buf[n] = '\0';

    // Parse the request line: "GET /path HTTP/1.x\r\n..."
    char method[16] = {0};
    char path[256] = {0};
    sscanf(buf, "%15s %255s", method, path);

    int status_code = 200;
    const char* body = json_response(path, &status_code);
    const char* status_text = (status_code == 200) ? "OK" : "Not Found";

    char response[4096];
    int len = snprintf(response, sizeof(response),
        "HTTP/1.1 %d %s\r\n"
        "Content-Type: application/json\r\n"
        "Content-Length: %zu\r\n"
        "Connection: close\r\n"
        "\r\n"
        "%s",
        status_code, status_text, strlen(body), body);

    write(client_fd, response, len);
    close(client_fd);
}

int main() {
    signal(SIGCHLD, SIG_IGN);

    const char* port_env = getenv("PORT");
    int port = (port_env && port_env[0] != '\0') ? atoi(port_env) : 8080;

    int server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd < 0) {
        perror("socket");
        return 1;
    }

    int opt = 1;
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons(port);

    if (bind(server_fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        perror("bind");
        close(server_fd);
        return 1;
    }

    if (listen(server_fd, 128) < 0) {
        perror("listen");
        close(server_fd);
        return 1;
    }

    fprintf(stdout, "Server listening on port %d\n", port);
    fflush(stdout);

    for (;;) {
        struct sockaddr_in client_addr;
        socklen_t client_len = sizeof(client_addr);
        int client_fd = accept(server_fd, (struct sockaddr*)&client_addr, &client_len);
        if (client_fd < 0) {
            perror("accept");
            continue;
        }
        handle_client(client_fd);
    }

    close(server_fd);
    return 0;
}
