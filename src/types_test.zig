const std = @import("std");
const skill_importer = @import("skill_importer");

test "agent entries derive aggregate enablement from enabled statuses" {
    try std.testing.expectEqual(
        skill_importer.AgentEnablement.both,
        (skill_importer.AgentEntries{
            .claude_code = .canonical_symlink,
            .codex = .skill_directory,
        }).enablement(),
    );
    try std.testing.expectEqual(
        skill_importer.AgentEnablement.neither,
        (skill_importer.AgentEntries{
            .claude_code = .missing,
            .codex = .broken_symlink,
        }).enablement(),
    );
}

test "error info carries operation payload and partial actions" {
    const allocator = std.testing.allocator;
    var error_info = skill_importer.ErrorInfo{
        .kind = .unsafe_agent_entry,
        .path = "/tmp/agent/helper",
        .reason = "entry is not a managed skill symlink",
    };
    defer error_info.deinit(allocator);

    try error_info.actions.append(allocator, .{
        .action = .create_directory,
        .agent = .codex,
        .path = "/tmp/agent",
    });

    try std.testing.expectEqual(skill_importer.ErrorKind.unsafe_agent_entry, error_info.kind);
    try std.testing.expectEqualStrings("/tmp/agent/helper", error_info.path.?);
    try std.testing.expectEqual(@as(usize, 1), error_info.actions.items.len);
    try std.testing.expectEqual(skill_importer.SkillActionKind.create_directory, error_info.actions.items[0].action);

    const ok_result = skill_importer.Result(void){ .ok = {} };
    try std.testing.expectEqual({}, ok_result.ok);
}
