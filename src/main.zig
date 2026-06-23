const std = @import("std");
const skill_importer = @import("skill_importer");

pub fn main() !void {
    const allocator = std.heap.page_allocator;
    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    var stdout_buffer: [4096]u8 = undefined;
    var stdout_writer = std.fs.File.stdout().writer(&stdout_buffer);
    const stdout = &stdout_writer.interface;
    defer stdout.flush() catch {};

    var stderr_buffer: [4096]u8 = undefined;
    var stderr_writer = std.fs.File.stderr().writer(&stderr_buffer);
    const stderr = &stderr_writer.interface;
    defer stderr.flush() catch {};

    if (args.len < 2) {
        try stderr.writeAll("skill-importer: missing command\n");
        std.process.exit(2);
    }

    const command = args[1];
    if (std.mem.eql(u8, command, "list")) {
        try requireJson(args);
        var inventory = try skill_importer.discoverSkills(allocator, parseRoots(args));
        defer inventory.deinit(allocator);
        try skill_importer.writeInventoryJson(stdout, inventory);
        try stdout.writeByte('\n');
        return;
    }

    if (std.mem.eql(u8, command, "import")) {
        if (args.len < 3 or !std.mem.eql(u8, args[2], "markdown")) {
            try stderr.writeAll("skill-importer: only `import markdown` is implemented\n");
            std.process.exit(2);
        }
        try requireJson(args);
        const markdown = try std.fs.File.stdin().readToEndAlloc(allocator, 1024 * 1024);
        defer allocator.free(markdown);
        var result = try skill_importer.importMarkdown(allocator, parseRoots(args), markdown, optionValue(args, "--source-location"));
        switch (result) {
            .ok => |*import_result| {
                defer import_result.deinit(allocator);
                try skill_importer.writeImportResultJson(stdout, import_result.*);
                try stdout.writeByte('\n');
            },
            .err => |*error_info| {
                defer error_info.deinit(allocator);
                try stderr.print("skill-importer: {s}\n", .{error_info.message orelse "import failed"});
                std.process.exit(1);
            },
        }
        return;
    }

    if (std.mem.eql(u8, command, "tui")) {
        try stderr.writeAll("skill-importer: tui not yet implemented\n");
        std.process.exit(1);
    }

    try stderr.print("skill-importer: unknown command `{s}`\n", .{command});
    std.process.exit(2);
}

fn parseRoots(args: []const []const u8) skill_importer.DiscoveryRoots {
    return .{
        .canonical_root = optionValue(args, "--canonical-root") orelse ".skill-importer/dev/agent-skills/third-party",
        .imports_root = optionValue(args, "--imports-root") orelse ".skill-importer/dev/v2/imports",
        .claude_code_root = optionValue(args, "--claude-code-root") orelse ".skill-importer/dev/claude",
        .codex_root = optionValue(args, "--codex-root") orelse ".skill-importer/dev/codex",
    };
}

fn optionValue(args: []const []const u8, name: []const u8) ?[]const u8 {
    var index: usize = 0;
    while (index + 1 < args.len) : (index += 1) {
        if (std.mem.eql(u8, args[index], name)) return args[index + 1];
    }
    return null;
}

fn requireJson(args: []const []const u8) !void {
    for (args) |arg| {
        if (std.mem.eql(u8, arg, "--json")) return;
    }
    var stderr_buffer: [1024]u8 = undefined;
    var stderr_writer = std.fs.File.stderr().writer(&stderr_buffer);
    const stderr = &stderr_writer.interface;
    try stderr.writeAll("skill-importer: --json is required for this command\n");
    try stderr.flush();
    std.process.exit(2);
}
