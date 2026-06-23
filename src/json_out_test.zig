const std = @import("std");
const skill_importer = @import("skill_importer");

test "inventory JSON preserves key order and omits absent optional skill fields" {
    const allocator = std.testing.allocator;
    var inventory = skill_importer.SkillInventory{};
    defer inventory.deinit(allocator);
    try inventory.skills.append(allocator, .{
        .name = try allocator.dupe(u8, "alpha"),
        .source = .canonical,
        .promoted = false,
        .agent_entries = .{ .claude_code = .missing, .codex = .canonical_symlink },
    });

    const json = try skill_importer.inventoryJsonAlloc(allocator, inventory);
    defer allocator.free(json);

    const expected =
        \\{
        \\  "skills": [
        \\    {
        \\      "name": "alpha",
        \\      "source": "canonical",
        \\      "promoted": false,
        \\      "enablement": {
        \\        "claude_code": false,
        \\        "codex": true
        \\      },
        \\      "agent_entries": {
        \\        "claude_code": "missing",
        \\        "codex": "canonical_symlink"
        \\      }
        \\    }
        \\  ],
        \\  "source_repositories": []
        \\}
    ;
    try std.testing.expectEqualStrings(expected, json);
}

test "inventory JSON includes repository metadata in stable shape" {
    const allocator = std.testing.allocator;
    var inventory = skill_importer.SkillInventory{};
    defer inventory.deinit(allocator);
    try inventory.skills.append(allocator, .{
        .name = try allocator.dupe(u8, "repo-helper"),
        .description = try allocator.dupe(u8, "repo description"),
        .source = .imported,
        .source_repository = .{
            .repository = try allocator.dupe(u8, "repo"),
            .skill_path = try allocator.dupe(u8, "skills/repo-helper"),
        },
        .promoted = true,
        .agent_entries = .{ .claude_code = .imported_symlink, .codex = .external_symlink },
    });
    var repository_entry = skill_importer.SourceRepositoryEntry{ .repository = try allocator.dupe(u8, "repo") };
    try repository_entry.skills.append(allocator, .{
        .skill_name = try allocator.dupe(u8, "repo-helper"),
        .skill_path = try allocator.dupe(u8, "skills/repo-helper"),
    });
    try inventory.source_repositories.append(allocator, repository_entry);

    const json = try skill_importer.inventoryJsonAlloc(allocator, inventory);
    defer allocator.free(json);

    try std.testing.expect(std.mem.indexOf(u8, json, "\"description\": \"repo description\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"source_repository\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"claude_code\": true") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"codex\": true") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"source_repositories\"") != null);
}

test "import result JSON emits manifest and ordered actions" {
    const allocator = std.testing.allocator;
    var result = skill_importer.ImportResult{
        .skill_name = try allocator.dupe(u8, "demo"),
        .skill_path = try allocator.dupe(u8, "imports/demo"),
        .manifest_path = try allocator.dupe(u8, "imports/demo/import.json"),
        .manifest = .{
            .source_type = .markdown,
            .source_location = try allocator.dupe(u8, "stdin"),
            .imported_at = 1,
            .content_hash = try allocator.dupe(u8, "sha256:abc"),
            .promoted = false,
        },
    };
    defer result.deinit(allocator);
    try result.actions.append(allocator, .{ .action = .create_directory, .path = try allocator.dupe(u8, "imports/demo") });
    try result.actions.append(allocator, .{ .action = .write_skill, .path = try allocator.dupe(u8, "imports/demo/SKILL.md") });

    const json = try skill_importer.importResultJsonAlloc(allocator, result);
    defer allocator.free(json);

    try std.testing.expect(std.mem.indexOf(u8, json, "\"skill_name\": \"demo\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"source_type\": \"markdown\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"action\": \"create_directory\"") != null);
    try std.testing.expect(!std.mem.endsWith(u8, json, "\n"));
}
