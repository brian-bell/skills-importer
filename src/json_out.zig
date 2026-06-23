const std = @import("std");
const types = @import("types.zig");

pub fn inventoryJsonAlloc(allocator: std.mem.Allocator, inventory: types.SkillInventory) ![]const u8 {
    var out: std.Io.Writer.Allocating = .init(allocator);
    defer out.deinit();
    try writeInventoryJson(&out.writer, inventory);
    return try allocator.dupe(u8, out.written());
}

pub fn writeInventoryJson(writer: *std.Io.Writer, inventory: types.SkillInventory) !void {
    var stringify: std.json.Stringify = .{
        .writer = writer,
        .options = .{ .whitespace = .indent_2 },
    };
    try stringify.beginObject();
    try stringify.objectField("skills");
    try stringify.beginArray();
    for (inventory.skills.items) |skill| {
        try writeSkill(&stringify, skill);
    }
    try stringify.endArray();
    try stringify.objectField("source_repositories");
    try stringify.beginArray();
    for (inventory.source_repositories.items) |repository| {
        try stringify.beginObject();
        try stringify.objectField("repository");
        try stringify.write(repository.repository);
        try stringify.objectField("skills");
        try stringify.beginArray();
        for (repository.skills.items) |skill| {
            try stringify.beginObject();
            try stringify.objectField("skill_name");
            try stringify.write(skill.skill_name);
            try stringify.objectField("skill_path");
            try stringify.write(skill.skill_path);
            try stringify.endObject();
        }
        try stringify.endArray();
        try stringify.endObject();
    }
    try stringify.endArray();
    try stringify.endObject();
}

pub fn importResultJsonAlloc(allocator: std.mem.Allocator, result: types.ImportResult) ![]const u8 {
    var out: std.Io.Writer.Allocating = .init(allocator);
    defer out.deinit();
    try writeImportResultJson(&out.writer, result);
    return try allocator.dupe(u8, out.written());
}

pub fn writeImportResultJson(writer: *std.Io.Writer, result: types.ImportResult) !void {
    var stringify: std.json.Stringify = .{
        .writer = writer,
        .options = .{ .whitespace = .indent_2 },
    };
    try stringify.beginObject();
    try stringify.objectField("skill_name");
    try stringify.write(result.skill_name);
    try stringify.objectField("skill_path");
    try stringify.write(result.skill_path);
    try stringify.objectField("manifest_path");
    try stringify.write(result.manifest_path);
    try stringify.objectField("manifest");
    try writeManifest(&stringify, result.manifest);
    try stringify.objectField("actions");
    try stringify.beginArray();
    for (result.actions.items) |action| {
        try stringify.beginObject();
        try stringify.objectField("action");
        try stringify.write(importActionKindString(action.action));
        try stringify.objectField("path");
        try stringify.write(action.path);
        try stringify.endObject();
    }
    try stringify.endArray();
    try stringify.endObject();
}

fn writeSkill(stringify: *std.json.Stringify, skill: types.SkillEntry) !void {
    try stringify.beginObject();
    try stringify.objectField("name");
    try stringify.write(skill.name);
    if (skill.description) |description| {
        try stringify.objectField("description");
        try stringify.write(description);
    }
    try stringify.objectField("source");
    try stringify.write(skillSourceString(skill.source));
    if (skill.source_repository) |repository| {
        try stringify.objectField("source_repository");
        try stringify.beginObject();
        try stringify.objectField("repository");
        try stringify.write(repository.repository);
        try stringify.objectField("skill_path");
        try stringify.write(repository.skill_path);
        try stringify.endObject();
    }
    try stringify.objectField("promoted");
    try stringify.write(skill.promoted);
    try stringify.objectField("enablement");
    try stringify.beginObject();
    try stringify.objectField("claude_code");
    try stringify.write(skill.agent_entries.claude_code.isEnabled());
    try stringify.objectField("codex");
    try stringify.write(skill.agent_entries.codex.isEnabled());
    try stringify.endObject();
    try stringify.objectField("agent_entries");
    try stringify.beginObject();
    try stringify.objectField("claude_code");
    try stringify.write(agentEntryStatusString(skill.agent_entries.claude_code));
    try stringify.objectField("codex");
    try stringify.write(agentEntryStatusString(skill.agent_entries.codex));
    try stringify.endObject();
    try stringify.endObject();
}

fn writeManifest(stringify: *std.json.Stringify, value: types.ImportManifest) !void {
    try stringify.beginObject();
    try stringify.objectField("source_type");
    try stringify.write(importSourceTypeString(value.source_type));
    try stringify.objectField("source_location");
    if (value.source_location) |location| {
        try stringify.write(location);
    } else {
        try stringify.write(null);
    }
    if (value.source_repository) |repository| {
        try stringify.objectField("source_repository");
        try stringify.beginObject();
        try stringify.objectField("repository");
        try stringify.write(repository.repository);
        try stringify.objectField("skill_path");
        try stringify.write(repository.skill_path);
        try stringify.endObject();
    }
    try stringify.objectField("imported_at");
    try stringify.write(value.imported_at);
    try stringify.objectField("content_hash");
    try stringify.write(value.content_hash);
    try stringify.objectField("promoted");
    try stringify.write(value.promoted);
    try stringify.endObject();
}

fn skillSourceString(source: types.SkillSource) []const u8 {
    return switch (source) {
        .canonical => "canonical",
        .imported => "imported",
        .agent_only => "agent_only",
    };
}

fn importSourceTypeString(source_type: types.ImportSourceType) []const u8 {
    return switch (source_type) {
        .markdown => "markdown",
        .local_path => "local_path",
        .url => "url",
        .repository => "repository",
    };
}

fn importActionKindString(kind: types.ImportActionKind) []const u8 {
    return switch (kind) {
        .create_directory => "create_directory",
        .write_skill => "write_skill",
        .copy_file => "copy_file",
        .write_manifest => "write_manifest",
    };
}

fn agentEntryStatusString(status: types.AgentEntryStatus) []const u8 {
    return switch (status) {
        .missing => "missing",
        .skill_directory => "skill_directory",
        .canonical_symlink => "canonical_symlink",
        .imported_symlink => "imported_symlink",
        .external_symlink => "external_symlink",
        .broken_symlink => "broken_symlink",
    };
}
