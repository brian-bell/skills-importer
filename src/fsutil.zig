const std = @import("std");
const types = @import("types.zig");

pub fn canonicalizeExistingAncestor(allocator: std.mem.Allocator, path: []const u8) ![]const u8 {
    var absolute = try std.fs.path.resolve(allocator, &.{path});
    defer allocator.free(absolute);

    var missing: std.ArrayList([]const u8) = .empty;
    defer {
        for (missing.items) |part| allocator.free(part);
        missing.deinit(allocator);
    }

    while (true) {
        if (std.fs.realpathAlloc(allocator, absolute)) |real| {
            var resolved = real;
            for (missing.items) |part| {
                const next = try std.fs.path.join(allocator, &.{ resolved, part });
                allocator.free(resolved);
                resolved = next;
            }
            return resolved;
        } else |err| switch (err) {
            error.FileNotFound => {
                const base = std.fs.path.basename(absolute);
                try missing.insert(allocator, 0, try allocator.dupe(u8, base));
                const parent = std.fs.path.dirname(absolute) orelse return error.FileNotFound;
                const next = try allocator.dupe(u8, parent);
                allocator.free(absolute);
                absolute = next;
            },
            else => return err,
        }
    }
}

pub fn lexicalSymlinkTarget(allocator: std.mem.Allocator, path: []const u8) ![]const u8 {
    var buffer: [std.fs.max_path_bytes]u8 = undefined;
    const target = try std.fs.cwd().readLink(path, &buffer);
    if (std.fs.path.isAbsolute(target)) return try allocator.dupe(u8, target);
    const parent = std.fs.path.dirname(path) orelse ".";
    return try std.fs.path.resolve(allocator, &.{ parent, target });
}

pub fn agentEntryStatus(allocator: std.mem.Allocator, path: []const u8, roots: types.DiscoveryRoots) !types.AgentEntryStatus {
    const parent = std.fs.path.dirname(path) orelse ".";
    const name = std.fs.path.basename(path);
    var dir = try std.fs.cwd().openDir(parent, .{ .iterate = true });
    defer dir.close();

    var iterator = dir.iterate();
    while (try iterator.next()) |entry| {
        if (!std.mem.eql(u8, entry.name, name)) continue;
        return switch (entry.kind) {
            .sym_link => symlinkEntryStatus(allocator, path, roots),
            .directory => .skill_directory,
            else => .missing,
        };
    }
    return error.FileNotFound;
}

fn symlinkEntryStatus(allocator: std.mem.Allocator, path: []const u8, roots: types.DiscoveryRoots) !types.AgentEntryStatus {
    const lexical = try lexicalSymlinkTarget(allocator, path);
    defer allocator.free(lexical);
    const real = std.fs.realpathAlloc(allocator, lexical) catch |err| switch (err) {
        error.FileNotFound => return .broken_symlink,
        else => return err,
    };
    defer allocator.free(real);
    const stat = std.fs.cwd().statFile(real) catch |err| switch (err) {
        error.FileNotFound => return .broken_symlink,
        else => return err,
    };
    if (stat.kind != .directory) return .missing;
    return try classifySymlinkTarget(allocator, real, roots);
}

pub fn classifySymlinkTarget(allocator: std.mem.Allocator, target: []const u8, roots: types.DiscoveryRoots) !types.AgentEntryStatus {
    if (try pathIsWithinExistingRoot(allocator, target, roots.canonical_root)) return .canonical_symlink;
    if (try pathIsWithinExistingRoot(allocator, target, roots.imports_root)) return .imported_symlink;
    return .external_symlink;
}

fn pathIsWithinExistingRoot(allocator: std.mem.Allocator, path: []const u8, root: []const u8) !bool {
    const real_root = std.fs.realpathAlloc(allocator, root) catch |err| switch (err) {
        error.FileNotFound => return false,
        else => return err,
    };
    defer allocator.free(real_root);
    return std.mem.eql(u8, path, real_root) or
        (std.mem.startsWith(u8, path, real_root) and path.len > real_root.len and path[real_root.len] == '/');
}

pub fn copyTree(allocator: std.mem.Allocator, source: []const u8, destination: []const u8) !void {
    try std.fs.cwd().makePath(destination);
    try copyTreeEntries(allocator, source, destination);
}

fn copyTreeEntries(allocator: std.mem.Allocator, source: []const u8, destination: []const u8) !void {
    var dir = try std.fs.cwd().openDir(source, .{ .iterate = true });
    defer dir.close();

    var iterator = dir.iterate();
    while (try iterator.next()) |entry| {
        const source_child = try std.fs.path.join(allocator, &.{ source, entry.name });
        defer allocator.free(source_child);
        const destination_child = try std.fs.path.join(allocator, &.{ destination, entry.name });
        defer allocator.free(destination_child);

        switch (entry.kind) {
            .directory => {
                try std.fs.cwd().makePath(destination_child);
                try copyTreeEntries(allocator, source_child, destination_child);
            },
            .file => try std.fs.cwd().copyFile(source_child, std.fs.cwd(), destination_child, .{}),
            .sym_link => {
                var buffer: [std.fs.max_path_bytes]u8 = undefined;
                const target = try std.fs.cwd().readLink(source_child, &buffer);
                try std.fs.cwd().symLink(target, destination_child, .{});
            },
            else => return error.UnsupportedDirectoryEntry,
        }
    }
}
