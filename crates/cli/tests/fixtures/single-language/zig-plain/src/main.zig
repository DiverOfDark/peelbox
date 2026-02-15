const std = @import("std");

pub fn main() !void {
    const addr = std.net.Address.initIp4(.{ 0, 0, 0, 0 }, 8080);
    var server = try addr.listen(.{ .reuse_address = true });
    defer server.deinit();

    std.debug.print("Server listening on port 8080\n", .{});

    while (true) {
        var conn = server.accept() catch continue;
        defer conn.stream.close();

        var buf: [1024]u8 = undefined;
        _ = conn.stream.read(&buf) catch continue;

        const response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
        conn.stream.writeAll(response) catch {};
    }
}
