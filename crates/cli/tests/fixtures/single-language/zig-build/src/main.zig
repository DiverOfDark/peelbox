const std = @import("std");
const net = std.net;
const mem = std.mem;
const posix = std.posix;

const http_200 = "HTTP/1.1 200 OK\r\n";
const http_404 = "HTTP/1.1 404 Not Found\r\n";
const content_type_json = "Content-Type: application/json\r\n";
const connection_close = "Connection: close\r\n";

const root_body =
    \\{"message":"Hello from Zig","version":"1.0.0","endpoints":["/","/health"]}
;

const health_body =
    \\{"status":"healthy"}
;

const not_found_body =
    \\{"error":"Not found"}
;

fn sendResponse(stream: net.Stream, status_line: []const u8, body: []const u8) void {
    var content_length_buf: [64]u8 = undefined;
    const content_length = std.fmt.bufPrint(&content_length_buf, "Content-Length: {d}\r\n", .{body.len}) catch return;

    _ = stream.write(status_line) catch return;
    _ = stream.write(content_type_json) catch return;
    _ = stream.write(content_length) catch return;
    _ = stream.write(connection_close) catch return;
    _ = stream.write("\r\n") catch return;
    _ = stream.write(body) catch return;
}

fn getRequestPath(buf: []const u8) ?[]const u8 {
    if (buf.len < 5) return null;
    // Parse "GET /path HTTP/1.x\r\n"
    if (!mem.startsWith(u8, buf, "GET ")) return null;
    const after_method = buf[4..];
    const space_idx = mem.indexOf(u8, after_method, " ") orelse return null;
    return after_method[0..space_idx];
}

fn handleClient(stream: net.Stream) void {
    defer stream.close();

    var buf: [4096]u8 = undefined;
    const bytes_read = stream.read(&buf) catch return;
    if (bytes_read == 0) return;

    const request = buf[0..bytes_read];
    const path = getRequestPath(request) orelse {
        sendResponse(stream, http_404, not_found_body);
        return;
    };

    if (mem.eql(u8, path, "/")) {
        sendResponse(stream, http_200, root_body);
    } else if (mem.eql(u8, path, "/health")) {
        sendResponse(stream, http_200, health_body);
    } else {
        sendResponse(stream, http_404, not_found_body);
    }
}

pub fn main() !void {
    const address = try net.Address.parseIp4("0.0.0.0", 8080);
    var server = try address.listen(.{
        .reuse_address = true,
    });
    defer server.deinit();

    const stdout = std.fs.File.stdout();
    try stdout.writeAll("Server listening on port 8080\n");

    while (true) {
        const connection = server.accept() catch |err| {
            std.log.err("Accept error: {}", .{err});
            continue;
        };
        handleClient(connection.stream);
    }
}
