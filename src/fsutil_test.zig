const std = @import("std");
const skill_importer = @import("skill_importer");

test "canonicalize existing ancestor resolves existing prefix and preserves missing suffix" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-fs-ancestor";
    defer std.fs.cwd().deleteTree(root) catch {};
    try std.fs.cwd().makePath(root ++ "/existing");

    const resolved = try skill_importer.canonicalizeExistingAncestor(allocator, root ++ "/existing/missing/leaf");
    defer allocator.free(resolved);
    try std.testing.expect(std.mem.endsWith(u8, resolved, "/zig-cache-fs-ancestor/existing/missing/leaf"));
}

test "lexical symlink target resolves relative target without requiring existence" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-fs-lexical";
    defer std.fs.cwd().deleteTree(root) catch {};
    try std.fs.cwd().makePath(root);
    try std.fs.cwd().symLink("../missing-target", root ++ "/link", .{});

    const target = try skill_importer.lexicalSymlinkTarget(allocator, root ++ "/link");
    defer allocator.free(target);
    try std.testing.expect(std.mem.endsWith(u8, target, "missing-target"));
}

test "agent entry status classifies managed and broken symlinks" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-fs-status";
    defer std.fs.cwd().deleteTree(root) catch {};
    try std.fs.cwd().makePath(root ++ "/canonical/helper");
    try std.fs.cwd().makePath(root ++ "/imports/draft");
    try std.fs.cwd().makePath(root ++ "/agent");
    try std.fs.cwd().symLink("../canonical/helper", root ++ "/agent/helper", .{});
    try std.fs.cwd().symLink("../missing", root ++ "/agent/broken", .{});

    const roots = skill_importer.DiscoveryRoots{
        .canonical_root = root ++ "/canonical",
        .imports_root = root ++ "/imports",
        .claude_code_root = root ++ "/agent",
        .codex_root = root ++ "/agent",
    };
    try std.testing.expectEqual(skill_importer.AgentEntryStatus.canonical_symlink, try skill_importer.agentEntryStatus(allocator, root ++ "/agent/helper", roots));
    try std.testing.expectEqual(skill_importer.AgentEntryStatus.broken_symlink, try skill_importer.agentEntryStatus(allocator, root ++ "/agent/broken", roots));
}

test "copy tree recreates symlinks instead of dereferencing them" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-fs-copy";
    defer std.fs.cwd().deleteTree(root) catch {};
    try std.fs.cwd().makePath(root ++ "/source/nested");
    try std.fs.cwd().writeFile(.{ .sub_path = root ++ "/source/nested/file", .data = "data" });
    try std.fs.cwd().symLink("nested/file", root ++ "/source/link", .{});

    try skill_importer.copyTree(allocator, root ++ "/source", root ++ "/destination");

    const copied = try std.fs.cwd().readFileAlloc(allocator, root ++ "/destination/nested/file", 1024);
    defer allocator.free(copied);
    try std.testing.expectEqualStrings("data", copied);
    var buffer: [std.fs.max_path_bytes]u8 = undefined;
    const link_target = try std.fs.cwd().readLink(root ++ "/destination/link", &buffer);
    try std.testing.expectEqualStrings("nested/file", link_target);
}
