const std = @import("std");
const frontmatter = @import("frontmatter.zig");
const hash = @import("hash.zig");
const manifest = @import("manifest.zig");
const types = @import("types.zig");

pub fn importMarkdown(
    allocator: std.mem.Allocator,
    roots: types.DiscoveryRoots,
    markdown: []const u8,
    source_location: ?[]const u8,
) !types.Result(types.ImportResult) {
    const metadata_result = validateImportMarkdown(markdown);
    if (metadata_result == .err) return .{ .err = metadata_result.err };
    const metadata = metadata_result.ok;

    const skill_path = try std.fs.path.join(allocator, &.{ roots.imports_root, metadata.name });
    errdefer allocator.free(skill_path);
    if (pathExists(skill_path)) {
        const name = try allocator.dupe(u8, metadata.name);
        return .{ .err = .{
            .kind = .collision,
            .owned_payloads = true,
            .name = name,
            .path = skill_path,
        } };
    }

    try std.fs.cwd().makePath(roots.imports_root);
    try std.fs.cwd().makePath(skill_path);
    errdefer std.fs.cwd().deleteTree(skill_path) catch {};

    var actions: std.ArrayList(types.ImportAction) = .empty;
    errdefer {
        for (actions.items) |*action| action.deinit(allocator);
        actions.deinit(allocator);
    }
    try actions.append(allocator, .{
        .action = .create_directory,
        .path = try allocator.dupe(u8, skill_path),
    });

    const skill_file = try std.fs.path.join(allocator, &.{ skill_path, "SKILL.md" });
    defer allocator.free(skill_file);
    try std.fs.cwd().writeFile(.{ .sub_path = skill_file, .data = markdown });
    try actions.append(allocator, .{
        .action = .write_skill,
        .path = try allocator.dupe(u8, skill_file),
    });

    const content_hash = try hash.contentHash(allocator, markdown);
    errdefer allocator.free(content_hash);
    var import_manifest = types.ImportManifest{
        .source_type = .markdown,
        .source_location = if (source_location) |value| try allocator.dupe(u8, value) else null,
        .imported_at = @intCast(std.time.timestamp()),
        .content_hash = content_hash,
        .promoted = false,
    };
    errdefer import_manifest.deinit(allocator);

    const manifest_path = try std.fs.path.join(allocator, &.{ skill_path, "import.json" });
    errdefer allocator.free(manifest_path);
    try manifest.writeImportManifest(allocator, manifest_path, import_manifest);
    try actions.append(allocator, .{
        .action = .write_manifest,
        .path = try allocator.dupe(u8, manifest_path),
    });

    return .{ .ok = .{
        .skill_name = try allocator.dupe(u8, metadata.name),
        .skill_path = skill_path,
        .manifest_path = manifest_path,
        .manifest = import_manifest,
        .actions = actions,
    } };
}

fn validateImportMarkdown(markdown: []const u8) types.Result(frontmatter.SkillMetadata) {
    const raw_result = frontmatter.parseSkillFrontmatter(markdown);
    if (raw_result == .err) return .{ .err = raw_result.err };
    const raw = raw_result.ok;
    const name = raw.name orelse return validationError("name", "missing `name` field");
    if (std.mem.trim(u8, name, " \t\r\n").len == 0) return validationError("name", "`name` cannot be empty");
    const name_result = frontmatter.validateSkillName(name);
    if (name_result == .err) return .{ .err = name_result.err };
    const description = raw.description orelse return validationError("description", "missing `description` field");
    if (std.mem.trim(u8, description, " \t\r\n").len == 0) return validationError("description", "`description` cannot be empty");
    return .{ .ok = .{ .name = name, .description = description } };
}

fn validationError(field: []const u8, message: []const u8) types.Result(frontmatter.SkillMetadata) {
    return .{ .err = .{ .kind = .validation, .field = field, .message = message } };
}

fn pathExists(path: []const u8) bool {
    std.fs.cwd().access(path, .{}) catch return false;
    return true;
}
