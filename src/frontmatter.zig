const std = @import("std");
const types = @import("types.zig");

pub const SkillMetadata = struct {
    name: []const u8,
    description: ?[]const u8 = null,
};

pub const RawSkillMetadata = struct {
    name: ?[]const u8 = null,
    description: ?[]const u8 = null,
};

pub fn parseSkillFrontmatter(contents: []const u8) types.Result(RawSkillMetadata) {
    var iterator = std.mem.splitScalar(u8, contents, '\n');
    const first = trimLineEnding(iterator.next() orelse "");
    if (!std.mem.eql(u8, first, "---")) {
        return rawValidationError("frontmatter", "missing opening frontmatter delimiter");
    }

    var raw = RawSkillMetadata{};
    var closed = false;
    while (iterator.next()) |line_with_ending| {
        const line = trimLineEnding(line_with_ending);
        if (std.mem.eql(u8, line, "---")) {
            closed = true;
            break;
        }
        if (std.mem.startsWith(u8, line, "name:")) {
            raw.name = cleanFrontmatterValue(line["name:".len..]);
        } else if (std.mem.startsWith(u8, line, "description:")) {
            raw.description = cleanFrontmatterValue(line["description:".len..]);
        }
    }

    if (!closed) {
        return rawValidationError("frontmatter", "missing closing frontmatter delimiter");
    }
    return .{ .ok = raw };
}

pub fn parseSkillMetadata(contents: []const u8) ?SkillMetadata {
    var iterator = std.mem.splitScalar(u8, contents, '\n');
    const first = trimLineEnding(iterator.next() orelse return null);
    if (!std.mem.eql(u8, first, "---")) return null;

    var name: ?[]const u8 = null;
    var description: ?[]const u8 = null;
    while (iterator.next()) |line_with_ending| {
        const line = trimLineEnding(line_with_ending);
        if (std.mem.eql(u8, line, "---")) break;
        if (std.mem.startsWith(u8, line, "name:")) {
            name = cleanFrontmatterValue(line["name:".len..]);
        } else if (std.mem.startsWith(u8, line, "description:")) {
            description = cleanFrontmatterValue(line["description:".len..]);
        }
    }

    return if (name) |value| .{ .name = value, .description = description } else null;
}

pub fn validateSkillName(name: []const u8) types.Result(void) {
    if (std.mem.trim(u8, name, " \t\r\n").len == 0) {
        return voidValidationError("name", "`name` cannot be empty");
    }
    if (std.mem.eql(u8, name, ".") or
        std.mem.eql(u8, name, "..") or
        std.mem.indexOfScalar(u8, name, '/') != null or
        std.mem.indexOfScalar(u8, name, '\\') != null)
    {
        return voidValidationError("name", "`name` must be a single directory-safe path segment");
    }
    return .{ .ok = {} };
}

pub fn cleanFrontmatterValue(value: []const u8) []const u8 {
    const trimmed = std.mem.trim(u8, value, " \t\r\n");
    if (trimmed.len >= 2) {
        if ((trimmed[0] == '"' and trimmed[trimmed.len - 1] == '"') or
            (trimmed[0] == '\'' and trimmed[trimmed.len - 1] == '\''))
        {
            return trimmed[1 .. trimmed.len - 1];
        }
    }
    return trimmed;
}

fn trimLineEnding(line: []const u8) []const u8 {
    return std.mem.trimRight(u8, line, "\r");
}

fn rawValidationError(field: []const u8, message: []const u8) types.Result(RawSkillMetadata) {
    return .{ .err = .{ .kind = .validation, .field = field, .message = message } };
}

fn voidValidationError(field: []const u8, message: []const u8) types.Result(void) {
    return .{ .err = .{ .kind = .validation, .field = field, .message = message } };
}
