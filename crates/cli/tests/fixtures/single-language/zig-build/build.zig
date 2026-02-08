const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const zap = b.dependency("zap", .{
        .target = target,
        .optimize = optimize,
    });

    const exe_mod = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
        .imports = &.{
            .{ .name = "zap", .module = zap.module("zap") },
        },
    });

    // Set rpath so the binary can find libfacil.io.so at runtime
    exe_mod.addRPath(.{ .cwd_relative = "/app/lib" });

    const exe = b.addExecutable(.{
        .name = "app",
        .root_module = exe_mod,
    });

    b.installArtifact(exe);

    // Install libfacil.io.so alongside the binary
    const facilio = zap.artifact("facil.io");
    b.installArtifact(facilio);
}
