const std = @import("std");
const skill_importer = @import("skill_importer");

test "content hash is stable sha256 hex" {
    const allocator = std.testing.allocator;
    const hash = try skill_importer.contentHash(allocator, "hello");
    defer allocator.free(hash);
    try std.testing.expectEqualStrings("sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", hash);
}

test "directory content hash is deterministic by file name" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-hash-fixture";
    defer std.fs.cwd().deleteTree(root) catch {};
    try std.fs.cwd().makePath(root ++ "/nested");
    try std.fs.cwd().writeFile(.{ .sub_path = root ++ "/z.txt", .data = "z" });
    try std.fs.cwd().writeFile(.{ .sub_path = root ++ "/a.txt", .data = "a" });
    try std.fs.cwd().writeFile(.{ .sub_path = root ++ "/nested/skill.txt", .data = "nested" });

    const first = try skill_importer.directoryContentHash(allocator, root);
    defer allocator.free(first);
    const second = try skill_importer.directoryContentHash(allocator, root);
    defer allocator.free(second);

    try std.testing.expectEqualStrings(first, second);
    try std.testing.expectEqualStrings("sha256:5bdd7f6828307117104c419b4488a112aff0aacfeca2201db898a8c0061f2baa", first);
}

test "directory content hash rejects symlinks" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-hash-symlink";
    defer std.fs.cwd().deleteTree(root) catch {};
    try std.fs.cwd().makePath(root);
    try std.fs.cwd().writeFile(.{ .sub_path = root ++ "/target", .data = "target" });
    try std.posix.symlink("target", root ++ "/link");

    try std.testing.expectError(error.UnsupportedDirectoryEntry, skill_importer.directoryContentHash(allocator, root));
}
