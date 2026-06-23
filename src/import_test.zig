const std = @import("std");
const skill_importer = @import("skill_importer");

test "markdown import creates skill file manifest and ordered actions" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-import-markdown";
    defer std.fs.cwd().deleteTree(root) catch {};
    const markdown = "---\nname: demo\ndescription: d\n---\nbody\n";

    var result = (try skill_importer.importMarkdown(allocator, roots(root), markdown, "stdin")).ok;
    defer result.deinit(allocator);

    try std.testing.expectEqualStrings("demo", result.skill_name);
    try std.testing.expectEqual(skill_importer.ImportSourceType.markdown, result.manifest.source_type);
    try std.testing.expectEqualStrings("stdin", result.manifest.source_location.?);
    try std.testing.expect(std.mem.startsWith(u8, result.manifest.content_hash, "sha256:"));
    try std.testing.expectEqual(@as(usize, 3), result.actions.items.len);
    try std.testing.expectEqual(skill_importer.ImportActionKind.create_directory, result.actions.items[0].action);
    try std.testing.expectEqual(skill_importer.ImportActionKind.write_skill, result.actions.items[1].action);
    try std.testing.expectEqual(skill_importer.ImportActionKind.write_manifest, result.actions.items[2].action);

    const skill_file = try std.fs.cwd().readFileAlloc(allocator, root ++ "/imports/demo/SKILL.md", 1024);
    defer allocator.free(skill_file);
    try std.testing.expectEqualStrings(markdown, skill_file);
    const manifest_bytes = try std.fs.cwd().readFileAlloc(allocator, root ++ "/imports/demo/import.json", 1024);
    defer allocator.free(manifest_bytes);
    try std.testing.expect(!std.mem.endsWith(u8, manifest_bytes, "\n"));
}

test "markdown import rejects invalid metadata and collisions" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-import-invalid";
    defer std.fs.cwd().deleteTree(root) catch {};

    const invalid = try skill_importer.importMarkdown(allocator, roots(root), "---\nname: nested/demo\ndescription: d\n---\n", null);
    try std.testing.expectEqual(skill_importer.ErrorKind.validation, invalid.err.kind);
    try std.testing.expectEqualStrings("name", invalid.err.field.?);

    var first = (try skill_importer.importMarkdown(allocator, roots(root), "---\nname: demo\ndescription: d\n---\n", null)).ok;
    defer first.deinit(allocator);
    var collision = (try skill_importer.importMarkdown(allocator, roots(root), "---\nname: demo\ndescription: d\n---\n", null)).err;
    defer collision.deinit(allocator);
    try std.testing.expectEqual(skill_importer.ErrorKind.collision, collision.kind);
}

fn roots(comptime root: []const u8) skill_importer.DiscoveryRoots {
    return .{
        .canonical_root = root ++ "/canonical",
        .imports_root = root ++ "/imports",
        .claude_code_root = root ++ "/claude",
        .codex_root = root ++ "/codex",
    };
}
