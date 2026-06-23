const std = @import("std");

const Sha256 = std.crypto.hash.sha2.Sha256;

const Entry = struct {
    name: []const u8,
    kind: std.fs.File.Kind,
};

pub fn contentHash(allocator: std.mem.Allocator, contents: []const u8) ![]const u8 {
    var hasher = Sha256.init(.{});
    hasher.update(contents);
    return finishHash(allocator, &hasher);
}

pub fn directoryContentHash(allocator: std.mem.Allocator, root: []const u8) ![]const u8 {
    var hasher = Sha256.init(.{});
    try hashDirectory(allocator, root, root, &hasher);
    return finishHash(allocator, &hasher);
}

fn hashDirectory(allocator: std.mem.Allocator, root: []const u8, directory: []const u8, hasher: *Sha256) !void {
    var dir = try std.fs.cwd().openDir(directory, .{ .iterate = true });
    defer dir.close();

    var entries: std.ArrayList(Entry) = .empty;
    defer {
        for (entries.items) |entry| allocator.free(entry.name);
        entries.deinit(allocator);
    }

    var iterator = dir.iterate();
    while (try iterator.next()) |entry| {
        try entries.append(allocator, .{
            .name = try allocator.dupe(u8, entry.name),
            .kind = entry.kind,
        });
    }
    std.mem.sort(Entry, entries.items, {}, entryLessThan);

    for (entries.items) |entry| {
        const child = try std.fs.path.join(allocator, &.{ directory, entry.name });
        defer allocator.free(child);
        const relative_path = try relativePath(child, root);

        switch (entry.kind) {
            .directory => {
                hashPathRecord(hasher, "dir", relative_path);
                try hashDirectory(allocator, root, child, hasher);
            },
            .file => {
                const bytes = try std.fs.cwd().readFileAlloc(allocator, child, 1024 * 1024 * 1024);
                defer allocator.free(bytes);
                hashFileRecord(hasher, relative_path, bytes);
            },
            else => return error.UnsupportedDirectoryEntry,
        }
    }
}

fn entryLessThan(_: void, left: Entry, right: Entry) bool {
    return std.mem.lessThan(u8, left.name, right.name);
}

fn relativePath(path: []const u8, root: []const u8) ![]const u8 {
    if (std.mem.eql(u8, path, root)) return "";
    if (!std.mem.startsWith(u8, path, root)) return error.PathOutsideRoot;
    var relative = path[root.len..];
    while (relative.len > 0 and (relative[0] == '/' or relative[0] == '\\')) {
        relative = relative[1..];
    }
    return relative;
}

fn hashPathRecord(hasher: *Sha256, tag: []const u8, path: []const u8) void {
    hashLength(hasher, tag.len);
    hasher.update(tag);
    hashLength(hasher, path.len);
    hasher.update(path);
}

fn hashFileRecord(hasher: *Sha256, path: []const u8, contents: []const u8) void {
    hashPathRecord(hasher, "file", path);
    hashLength(hasher, contents.len);
    hasher.update(contents);
}

fn hashLength(hasher: *Sha256, length: usize) void {
    var bytes: [8]u8 = undefined;
    std.mem.writeInt(u64, &bytes, @intCast(length), .big);
    hasher.update(&bytes);
}

fn finishHash(allocator: std.mem.Allocator, hasher: *Sha256) ![]const u8 {
    var digest: [Sha256.digest_length]u8 = undefined;
    hasher.final(&digest);
    const hex = std.fmt.bytesToHex(digest, .lower);
    return try std.fmt.allocPrint(allocator, "sha256:{s}", .{hex});
}
