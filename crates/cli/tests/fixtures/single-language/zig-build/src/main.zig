const std = @import("std");
const zap = @import("zap");

fn onHealth(r: zap.Request) !void {
    r.setHeader("Content-Type", "application/json") catch {};
    r.sendBody("{\"status\":\"healthy\"}") catch return;
}

fn onRoot(r: zap.Request) !void {
    r.setHeader("Content-Type", "application/json") catch {};
    r.sendBody("{\"message\":\"Hello from Zap\",\"endpoints\":[\"/\",\"/health\"]}") catch return;
}

fn notFound(r: zap.Request) !void {
    r.setHeader("Content-Type", "application/json") catch {};
    r.setStatus(.not_found);
    r.sendBody("{\"error\":\"Not found\"}") catch return;
}

fn onRequest(r: zap.Request) !void {
    if (r.path) |path| {
        if (std.mem.eql(u8, path, "/")) return onRoot(r);
        if (std.mem.eql(u8, path, "/health")) return onHealth(r);
    }
    return notFound(r);
}

pub fn main() !void {
    var listener = zap.HttpListener.init(.{
        .port = 3000,
        .on_request = onRequest,
        .log = false,
    });
    try listener.listen();

    std.debug.print("Server listening on port 3000\n", .{});

    zap.start(.{
        .threads = 1,
        .workers = 1,
    });
}
