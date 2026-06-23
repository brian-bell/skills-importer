const std = @import("std");
const types = @import("types.zig");

const ParsedImportManifest = struct {
    source_type: types.ImportSourceType,
    source_location: ?[]const u8 = null,
    source_repository: ?types.ImportSourceRepository = null,
    imported_at: u64,
    content_hash: []const u8,
    promoted: bool,
};

pub fn readImportManifest(allocator: std.mem.Allocator, path: []const u8) !types.ImportManifest {
    const bytes = try std.fs.cwd().readFileAlloc(allocator, path, 1024 * 1024);
    defer allocator.free(bytes);

    var parsed = try std.json.parseFromSlice(ParsedImportManifest, allocator, bytes, .{
        .ignore_unknown_fields = true,
    });
    defer parsed.deinit();

    return .{
        .source_type = parsed.value.source_type,
        .source_location = if (parsed.value.source_location) |value| try allocator.dupe(u8, value) else null,
        .source_repository = if (parsed.value.source_repository) |repository| .{
            .repository = try allocator.dupe(u8, repository.repository),
            .skill_path = try allocator.dupe(u8, repository.skill_path),
        } else null,
        .imported_at = parsed.value.imported_at,
        .content_hash = try allocator.dupe(u8, parsed.value.content_hash),
        .promoted = parsed.value.promoted,
    };
}

pub fn writeImportManifest(allocator: std.mem.Allocator, path: []const u8, value: types.ImportManifest) !void {
    const json = try manifestJsonAlloc(allocator, value);
    defer allocator.free(json);
    try std.fs.cwd().writeFile(.{ .sub_path = path, .data = json });
}

pub fn manifestJsonAlloc(allocator: std.mem.Allocator, value: types.ImportManifest) ![]const u8 {
    var out: std.Io.Writer.Allocating = .init(allocator);
    defer out.deinit();
    var stringify: std.json.Stringify = .{
        .writer = &out.writer,
        .options = .{ .whitespace = .indent_2 },
    };

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

    return try allocator.dupe(u8, out.written());
}

fn importSourceTypeString(source_type: types.ImportSourceType) []const u8 {
    return switch (source_type) {
        .markdown => "markdown",
        .local_path => "local_path",
        .url => "url",
        .repository => "repository",
    };
}
