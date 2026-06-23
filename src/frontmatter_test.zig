const std = @import("std");
const skill_importer = @import("skill_importer");

test "strict frontmatter requires opening and closing delimiters" {
    const missing_open = skill_importer.parseSkillFrontmatter("name: demo\n---\n");
    try std.testing.expectEqual(skill_importer.ErrorKind.validation, missing_open.err.kind);
    try std.testing.expectEqualStrings("frontmatter", missing_open.err.field.?);
    try std.testing.expectEqualStrings("missing opening frontmatter delimiter", missing_open.err.message.?);

    const missing_close = skill_importer.parseSkillFrontmatter("---\nname: demo\n");
    try std.testing.expectEqual(skill_importer.ErrorKind.validation, missing_close.err.kind);
    try std.testing.expectEqualStrings("missing closing frontmatter delimiter", missing_close.err.message.?);
}

test "strict frontmatter reads quoted name and description" {
    const parsed = skill_importer.parseSkillFrontmatter("---\nname: \"demo\"\ndescription: 'useful'\n---\nbody\n");
    try std.testing.expectEqualStrings("demo", parsed.ok.name.?);
    try std.testing.expectEqualStrings("useful", parsed.ok.description.?);
}

test "lenient metadata returns null without usable name" {
    try std.testing.expectEqual(@as(?skill_importer.SkillMetadata, null), skill_importer.parseSkillMetadata("body"));
    try std.testing.expectEqual(@as(?skill_importer.SkillMetadata, null), skill_importer.parseSkillMetadata("---\ndescription: d\n---\n"));

    const metadata = skill_importer.parseSkillMetadata("---\nname: demo\n---\n").?;
    try std.testing.expectEqualStrings("demo", metadata.name);
    try std.testing.expectEqual(@as(?[]const u8, null), metadata.description);
}

test "skill names must be one safe path segment" {
    try std.testing.expectEqual({}, skill_importer.validateSkillName("demo").ok);
    try std.testing.expectEqual(skill_importer.ErrorKind.validation, skill_importer.validateSkillName("").err.kind);
    try std.testing.expectEqual(skill_importer.ErrorKind.validation, skill_importer.validateSkillName("../demo").err.kind);
    try std.testing.expectEqual(skill_importer.ErrorKind.validation, skill_importer.validateSkillName("nested/demo").err.kind);
}
