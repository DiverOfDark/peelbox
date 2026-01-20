const std = @import("std");

pub fn main() !void {
    std.debug.print("Hello from Zig E2E test server!\n", .{});
    while (true) {
        std.posix.nanosleep(1, 0);
    }
}
