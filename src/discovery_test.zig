const std = @import("std");
const skill_importer = @import("skill_importer");

test "discovery treats missing roots as empty inventory" {
    const allocator = std.testing.allocator;
    var inventory = try skill_importer.discoverSkills(allocator, roots("zig-cache-discovery-missing"));
    defer inventory.deinit(allocator);

    try std.testing.expectEqual(@as(usize, 0), inventory.skills.items.len);
    try std.testing.expectEqual(@as(usize, 0), inventory.source_repositories.items.len);
}

test "discovery reads canonical, imported, and agent entries with merge precedence" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-discovery-merge";
    defer std.fs.cwd().deleteTree(root) catch {};
    const r = roots(root);
    try writeSkill(r.canonical_root, "shared-dir", "shared", null);
    try writeImportedSkill(r.imports_root, "shared-import", "shared", "imported description", true, null);
    try writeImportedSkill(r.imports_root, "repo-helper", "repo-helper", "repo description", false, .{
        .repository = "https://example.test/repo.git",
        .skill_path = "skills/repo-helper",
    });
    try std.fs.cwd().makePath(r.codex_root);
    try std.fs.cwd().symLink("../canonical/shared-dir", root ++ "/codex/shared", .{});

    var inventory = try skill_importer.discoverSkills(allocator, r);
    defer inventory.deinit(allocator);

    try std.testing.expectEqual(@as(usize, 2), inventory.skills.items.len);
    const shared = findSkill(&inventory, "shared").?;
    try std.testing.expectEqual(skill_importer.SkillSource.canonical, shared.source);
    try std.testing.expect(shared.promoted);
    try std.testing.expectEqual(skill_importer.AgentEntryStatus.canonical_symlink, shared.agent_entries.codex);
    try std.testing.expectEqual(skill_importer.AgentEnablement.codex, shared.enablement);
    try std.testing.expectEqualStrings("imported description", shared.description.?);

    const repo = findSkill(&inventory, "repo-helper").?;
    try std.testing.expectEqual(skill_importer.SkillSource.imported, repo.source);
    try std.testing.expectEqualStrings("https://example.test/repo.git", repo.source_repository.?.repository);
    try std.testing.expectEqual(@as(usize, 1), inventory.source_repositories.items.len);
    try std.testing.expectEqualStrings("repo-helper", inventory.source_repositories.items[0].skills.items[0].skill_name);
}

test "agent-only directories are included by directory name when metadata is absent" {
    const allocator = std.testing.allocator;
    const root = "zig-cache-discovery-agent-only";
    defer std.fs.cwd().deleteTree(root) catch {};
    const r = roots(root);
    try std.fs.cwd().makePath(root ++ "/claude/loose-helper");

    var inventory = try skill_importer.discoverSkills(allocator, r);
    defer inventory.deinit(allocator);

    try std.testing.expectEqual(@as(usize, 1), inventory.skills.items.len);
    try std.testing.expectEqualStrings("loose-helper", inventory.skills.items[0].name);
    try std.testing.expectEqual(skill_importer.SkillSource.agent_only, inventory.skills.items[0].source);
    try std.testing.expectEqual(skill_importer.AgentEntryStatus.skill_directory, inventory.skills.items[0].agent_entries.claude_code);
}

fn roots(comptime root: []const u8) skill_importer.DiscoveryRoots {
    return .{
        .canonical_root = root ++ "/canonical",
        .imports_root = root ++ "/imports",
        .claude_code_root = root ++ "/claude",
        .codex_root = root ++ "/codex",
    };
}

fn writeSkill(root: []const u8, dir_name: []const u8, name: []const u8, description: ?[]const u8) !void {
    const path = try std.fmt.allocPrint(std.testing.allocator, "{s}/{s}", .{ root, dir_name });
    defer std.testing.allocator.free(path);
    try std.fs.cwd().makePath(path);
    const body = if (description) |value|
        try std.fmt.allocPrint(
            std.testing.allocator,
            "---\nname: {s}\ndescription: {s}\n---\n",
            .{ name, value },
        )
    else
        try std.fmt.allocPrint(
            std.testing.allocator,
            "---\nname: {s}\n---\n",
            .{name},
        );
    defer std.testing.allocator.free(body);
    const skill_file = try std.fmt.allocPrint(std.testing.allocator, "{s}/SKILL.md", .{path});
    defer std.testing.allocator.free(skill_file);
    try std.fs.cwd().writeFile(.{ .sub_path = skill_file, .data = body });
}

fn writeImportedSkill(
    root: []const u8,
    dir_name: []const u8,
    name: []const u8,
    description: []const u8,
    promoted: bool,
    repository: ?skill_importer.ImportSourceRepository,
) !void {
    try writeSkill(root, dir_name, name, description);
    const dir = try std.fmt.allocPrint(std.testing.allocator, "{s}/{s}", .{ root, dir_name });
    defer std.testing.allocator.free(dir);
    const manifest_path = try std.fmt.allocPrint(std.testing.allocator, "{s}/import.json", .{dir});
    defer std.testing.allocator.free(manifest_path);
    const json = try skill_importer.manifestJsonAlloc(std.testing.allocator, .{
        .source_type = if (repository == null) .markdown else .repository,
        .source_location = null,
        .source_repository = repository,
        .imported_at = 1,
        .content_hash = "sha256:test",
        .promoted = promoted,
    });
    defer std.testing.allocator.free(json);
    try std.fs.cwd().writeFile(.{ .sub_path = manifest_path, .data = json });
}

fn findSkill(inventory: *const skill_importer.SkillInventory, name: []const u8) ?skill_importer.SkillEntry {
    for (inventory.skills.items) |skill| {
        if (std.mem.eql(u8, skill.name, name)) return skill;
    }
    return null;
}
