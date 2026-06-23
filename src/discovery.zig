const std = @import("std");
const frontmatter = @import("frontmatter.zig");
const fsutil = @import("fsutil.zig");
const manifest = @import("manifest.zig");
const types = @import("types.zig");

const EntryPath = struct {
    path: []const u8,
    name: []const u8,
    kind: std.fs.File.Kind,
};

const Metadata = struct {
    name: []const u8,
    description: ?[]const u8 = null,

    fn deinit(self: *Metadata, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
        if (self.description) |value| allocator.free(value);
        self.* = undefined;
    }
};

pub fn discoverSkills(allocator: std.mem.Allocator, roots: types.DiscoveryRoots) !types.SkillInventory {
    var inventory = types.SkillInventory{};
    errdefer inventory.deinit(allocator);

    try scanOwnedRoot(allocator, &inventory, roots.canonical_root, .canonical, roots);
    try scanOwnedRoot(allocator, &inventory, roots.imports_root, .imported, roots);
    try scanAgentRoot(allocator, &inventory, roots.claude_code_root, .claude_code, roots);
    try scanAgentRoot(allocator, &inventory, roots.codex_root, .codex, roots);

    for (inventory.skills.items) |*skill| {
        skill.enablement = skill.agent_entries.enablement();
    }
    std.mem.sort(types.SkillEntry, inventory.skills.items, {}, skillLessThan);
    try buildSourceRepositories(allocator, &inventory);
    return inventory;
}

fn scanOwnedRoot(
    allocator: std.mem.Allocator,
    inventory: *types.SkillInventory,
    root: []const u8,
    source: types.SkillSource,
    roots: types.DiscoveryRoots,
) !void {
    _ = roots;
    const entries = try readSortedEntries(allocator, root);
    defer freeEntries(allocator, entries);

    for (entries.items) |entry| {
        if (entry.kind != .directory) continue;
        var metadata = (try readSkillMetadata(allocator, entry.path)) orelse continue;
        defer metadata.deinit(allocator);

        var source_repository: ?types.ImportSourceRepository = null;
        var promoted = false;
        if (source == .imported) {
            const manifest_path = try std.fs.path.join(allocator, &.{ entry.path, "import.json" });
            defer allocator.free(manifest_path);
            var import_manifest = try manifest.readImportManifest(allocator, manifest_path);
            defer import_manifest.deinit(allocator);
            promoted = import_manifest.promoted;
            if (import_manifest.source_repository) |repository| {
                source_repository = .{
                    .repository = try allocator.dupe(u8, repository.repository),
                    .skill_path = try allocator.dupe(u8, repository.skill_path),
                };
            }
        }
        defer {
            if (source_repository) |repository| {
                allocator.free(repository.repository);
                allocator.free(repository.skill_path);
            }
        }

        const analysis_dir = try allocator.dupe(u8, entry.path);
        defer allocator.free(analysis_dir);
        _ = try mergeSkill(allocator, inventory, metadata, source, promoted, source_repository, analysis_dir);
    }
}

fn scanAgentRoot(
    allocator: std.mem.Allocator,
    inventory: *types.SkillInventory,
    root: []const u8,
    agent: types.SkillAgent,
    roots: types.DiscoveryRoots,
) !void {
    const entries = try readSortedEntries(allocator, root);
    defer freeEntries(allocator, entries);

    for (entries.items) |entry| {
        const status = (try fsutil.agentEntryStatus(allocator, entry.path, roots));
        if (status == .missing) continue;

        var metadata = (try readSkillMetadata(allocator, entry.path)) orelse Metadata{
            .name = try allocator.dupe(u8, entry.name),
            .description = null,
        };
        defer metadata.deinit(allocator);

        const analysis_dir = if (status != .broken_symlink) std.fs.realpathAlloc(allocator, entry.path) catch null else null;
        defer if (analysis_dir) |value| allocator.free(value);
        const index = try mergeSkill(allocator, inventory, metadata, .agent_only, false, null, analysis_dir);
        switch (agent) {
            .claude_code => inventory.skills.items[index].agent_entries.claude_code = status,
            .codex => inventory.skills.items[index].agent_entries.codex = status,
        }
    }
}

fn mergeSkill(
    allocator: std.mem.Allocator,
    inventory: *types.SkillInventory,
    metadata: Metadata,
    source: types.SkillSource,
    promoted: bool,
    source_repository: ?types.ImportSourceRepository,
    analysis_skill_dir: ?[]const u8,
) !usize {
    if (findSkill(inventory, metadata.name)) |index| {
        var skill = &inventory.skills.items[index];
        skill.promoted = skill.promoted or promoted;
        if (source == .imported and source_repository != null and skill.source_repository == null) {
            skill.source_repository = try cloneRepository(allocator, source_repository.?);
        }
        if (sourcePrecedence(source) < sourcePrecedence(skill.source)) {
            skill.source = source;
            if (skill.analysis_skill_dir) |value| allocator.free(value);
            skill.analysis_skill_dir = if (analysis_skill_dir) |value| try allocator.dupe(u8, value) else null;
        }
        if (skill.description == null) {
            skill.description = if (metadata.description) |value| try allocator.dupe(u8, value) else null;
        }
        return index;
    }

    try inventory.skills.append(allocator, .{
        .name = try allocator.dupe(u8, metadata.name),
        .description = if (metadata.description) |value| try allocator.dupe(u8, value) else null,
        .source = source,
        .source_repository = if (source_repository) |repository| try cloneRepository(allocator, repository) else null,
        .promoted = promoted,
        .agent_entries = .{ .claude_code = .missing, .codex = .missing },
        .analysis_skill_dir = if (analysis_skill_dir) |value| try allocator.dupe(u8, value) else null,
    });
    return inventory.skills.items.len - 1;
}

fn buildSourceRepositories(allocator: std.mem.Allocator, inventory: *types.SkillInventory) !void {
    for (inventory.skills.items) |skill| {
        const repository = skill.source_repository orelse continue;
        const index = try repositoryIndex(allocator, &inventory.source_repositories, repository.repository);
        try inventory.source_repositories.items[index].skills.append(allocator, .{
            .skill_name = try allocator.dupe(u8, skill.name),
            .skill_path = try allocator.dupe(u8, repository.skill_path),
        });
    }
    std.mem.sort(types.SourceRepositoryEntry, inventory.source_repositories.items, {}, repositoryLessThan);
    for (inventory.source_repositories.items) |*entry| {
        std.mem.sort(types.SourceRepositorySkill, entry.skills.items, {}, repositorySkillLessThan);
    }
}

fn repositoryIndex(
    allocator: std.mem.Allocator,
    repositories: *std.ArrayList(types.SourceRepositoryEntry),
    repository: []const u8,
) !usize {
    for (repositories.items, 0..) |entry, index| {
        if (std.mem.eql(u8, entry.repository, repository)) return index;
    }
    try repositories.append(allocator, .{ .repository = try allocator.dupe(u8, repository) });
    return repositories.items.len - 1;
}

fn readSkillMetadata(allocator: std.mem.Allocator, skill_dir: []const u8) !?Metadata {
    const skill_file = try std.fs.path.join(allocator, &.{ skill_dir, "SKILL.md" });
    defer allocator.free(skill_file);
    const contents = std.fs.cwd().readFileAlloc(allocator, skill_file, 1024 * 1024) catch |err| switch (err) {
        error.FileNotFound => return null,
        else => return err,
    };
    defer allocator.free(contents);
    const parsed = frontmatter.parseSkillMetadata(contents) orelse return null;
    return .{
        .name = try allocator.dupe(u8, parsed.name),
        .description = if (parsed.description) |value| try allocator.dupe(u8, value) else null,
    };
}

fn readSortedEntries(allocator: std.mem.Allocator, root: []const u8) !std.ArrayList(EntryPath) {
    var entries: std.ArrayList(EntryPath) = .empty;
    errdefer freeEntries(allocator, entries);

    var dir = std.fs.cwd().openDir(root, .{ .iterate = true }) catch |err| switch (err) {
        error.FileNotFound => return entries,
        else => return err,
    };
    defer dir.close();

    var iterator = dir.iterate();
    while (try iterator.next()) |entry| {
        const path = try std.fs.path.join(allocator, &.{ root, entry.name });
        errdefer allocator.free(path);
        try entries.append(allocator, .{
            .path = path,
            .name = try allocator.dupe(u8, entry.name),
            .kind = entry.kind,
        });
    }
    std.mem.sort(EntryPath, entries.items, {}, entryPathLessThan);
    return entries;
}

fn freeEntries(allocator: std.mem.Allocator, entries: std.ArrayList(EntryPath)) void {
    for (entries.items) |entry| {
        allocator.free(entry.path);
        allocator.free(entry.name);
    }
    var mutable = entries;
    mutable.deinit(allocator);
}

fn cloneRepository(allocator: std.mem.Allocator, repository: types.ImportSourceRepository) !types.ImportSourceRepository {
    return .{
        .repository = try allocator.dupe(u8, repository.repository),
        .skill_path = try allocator.dupe(u8, repository.skill_path),
    };
}

fn findSkill(inventory: *const types.SkillInventory, name: []const u8) ?usize {
    for (inventory.skills.items, 0..) |skill, index| {
        if (std.mem.eql(u8, skill.name, name)) return index;
    }
    return null;
}

fn sourcePrecedence(source: types.SkillSource) usize {
    return switch (source) {
        .canonical => 0,
        .imported => 1,
        .agent_only => 2,
    };
}

fn entryPathLessThan(_: void, left: EntryPath, right: EntryPath) bool {
    return std.mem.lessThan(u8, left.path, right.path);
}

fn skillLessThan(_: void, left: types.SkillEntry, right: types.SkillEntry) bool {
    return std.mem.lessThan(u8, left.name, right.name);
}

fn repositoryLessThan(_: void, left: types.SourceRepositoryEntry, right: types.SourceRepositoryEntry) bool {
    return std.mem.lessThan(u8, left.repository, right.repository);
}

fn repositorySkillLessThan(_: void, left: types.SourceRepositorySkill, right: types.SourceRepositorySkill) bool {
    return std.mem.lessThan(u8, left.skill_name, right.skill_name) or
        (std.mem.eql(u8, left.skill_name, right.skill_name) and std.mem.lessThan(u8, left.skill_path, right.skill_path));
}
