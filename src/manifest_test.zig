const std = @import("std");
const skill_importer = @import("skill_importer");

test "manifest JSON keeps key order and omits absent repository without trailing newline" {
    const allocator = std.testing.allocator;
    const json = try skill_importer.manifestJsonAlloc(allocator, .{
        .source_type = .markdown,
        .source_location = null,
        .imported_at = 42,
        .content_hash = "sha256:abc",
        .promoted = false,
    });
    defer allocator.free(json);

    const expected =
        \\{
        \\  "source_type": "markdown",
        \\  "source_location": null,
        \\  "imported_at": 42,
        \\  "content_hash": "sha256:abc",
        \\  "promoted": false
        \\}
    ;
    try std.testing.expectEqualStrings(expected, json);
}

test "manifest JSON includes repository metadata when present" {
    const allocator = std.testing.allocator;
    const json = try skill_importer.manifestJsonAlloc(allocator, .{
        .source_type = .repository,
        .source_location = "https://example.test/repo.git",
        .source_repository = .{
            .repository = "https://example.test/repo.git",
            .skill_path = "skills/demo",
        },
        .imported_at = 7,
        .content_hash = "sha256:def",
        .promoted = true,
    });
    defer allocator.free(json);

    try std.testing.expect(std.mem.indexOf(u8, json, "\"source_repository\"") != null);
    try std.testing.expect(!std.mem.endsWith(u8, json, "\n"));
}

test "manifest read duplicates parsed strings beyond parser lifetime" {
    const allocator = std.testing.allocator;
    const path = "zig-cache-test-manifest.json";
    defer std.fs.cwd().deleteFile(path) catch {};
    try std.fs.cwd().writeFile(.{
        .sub_path = path,
        .data =
        \\{
        \\  "source_type": "local_path",
        \\  "source_location": "/tmp/demo",
        \\  "source_repository": {"repository":"repo","skill_path":"demo"},
        \\  "imported_at": 99,
        \\  "content_hash": "sha256:123",
        \\  "promoted": true
        \\}
        ,
    });

    var manifest = try skill_importer.readImportManifest(allocator, path);
    defer manifest.deinit(allocator);

    try std.testing.expectEqual(skill_importer.ImportSourceType.local_path, manifest.source_type);
    try std.testing.expectEqualStrings("/tmp/demo", manifest.source_location.?);
    try std.testing.expectEqualStrings("repo", manifest.source_repository.?.repository);
    try std.testing.expectEqualStrings("demo", manifest.source_repository.?.skill_path);
    try std.testing.expectEqualStrings("sha256:123", manifest.content_hash);
    try std.testing.expect(manifest.promoted);
}
