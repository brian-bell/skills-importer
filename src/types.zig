const std = @import("std");

pub fn Result(comptime Ok: type) type {
    return union(enum) {
        ok: Ok,
        err: ErrorInfo,
    };
}

pub const DiscoveryRoots = struct {
    canonical_root: []const u8,
    imports_root: []const u8,
    claude_code_root: []const u8,
    codex_root: []const u8,
};

pub const SkillSource = enum {
    canonical,
    imported,
    agent_only,
};

pub const SkillInventory = struct {
    skills: std.ArrayList(SkillEntry) = .empty,
    source_repositories: std.ArrayList(SourceRepositoryEntry) = .empty,

    pub fn deinit(self: *SkillInventory, allocator: std.mem.Allocator) void {
        for (self.skills.items) |*skill| skill.deinit(allocator);
        self.skills.deinit(allocator);
        for (self.source_repositories.items) |*entry| entry.deinit(allocator);
        self.source_repositories.deinit(allocator);
        self.* = undefined;
    }
};

pub const SkillEntry = struct {
    name: []const u8,
    description: ?[]const u8 = null,
    source: SkillSource,
    source_repository: ?ImportSourceRepository = null,
    promoted: bool = false,
    enablement: AgentEnablement = .neither,
    agent_entries: AgentEntries,
    analysis_skill_dir: ?[]const u8 = null,

    pub fn deinit(self: *SkillEntry, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
        if (self.description) |value| allocator.free(value);
        if (self.source_repository) |repository| {
            allocator.free(repository.repository);
            allocator.free(repository.skill_path);
        }
        if (self.analysis_skill_dir) |value| allocator.free(value);
        self.* = undefined;
    }
};

pub const SourceRepositoryEntry = struct {
    repository: []const u8,
    skills: std.ArrayList(SourceRepositorySkill) = .empty,

    pub fn deinit(self: *SourceRepositoryEntry, allocator: std.mem.Allocator) void {
        allocator.free(self.repository);
        for (self.skills.items) |*skill| skill.deinit(allocator);
        self.skills.deinit(allocator);
        self.* = undefined;
    }
};

pub const SourceRepositorySkill = struct {
    skill_name: []const u8,
    skill_path: []const u8,

    pub fn deinit(self: *SourceRepositorySkill, allocator: std.mem.Allocator) void {
        allocator.free(self.skill_name);
        allocator.free(self.skill_path);
        self.* = undefined;
    }
};

pub const AgentEntryStatus = enum {
    missing,
    skill_directory,
    canonical_symlink,
    imported_symlink,
    external_symlink,
    broken_symlink,

    pub fn isEnabled(self: AgentEntryStatus) bool {
        return switch (self) {
            .skill_directory,
            .canonical_symlink,
            .imported_symlink,
            .external_symlink,
            => true,
            .missing,
            .broken_symlink,
            => false,
        };
    }
};

pub const AgentEnablement = enum {
    neither,
    claude_code,
    codex,
    both,
};

pub const AgentEntries = struct {
    claude_code: AgentEntryStatus,
    codex: AgentEntryStatus,

    pub fn enablement(self: AgentEntries) AgentEnablement {
        const claude_enabled = self.claude_code.isEnabled();
        const codex_enabled = self.codex.isEnabled();
        if (claude_enabled and codex_enabled) return .both;
        if (claude_enabled) return .claude_code;
        if (codex_enabled) return .codex;
        return .neither;
    }
};

pub const SkillAgent = enum {
    claude_code,
    codex,
};

pub const ImportSourceType = enum {
    markdown,
    local_path,
    url,
    repository,
};

pub const ImportSourceRepository = struct {
    repository: []const u8,
    skill_path: []const u8,
};

pub const ImportManifest = struct {
    source_type: ImportSourceType,
    source_location: ?[]const u8 = null,
    source_repository: ?ImportSourceRepository = null,
    imported_at: u64,
    content_hash: []const u8,
    promoted: bool,

    pub fn deinit(self: *ImportManifest, allocator: std.mem.Allocator) void {
        if (self.source_location) |value| allocator.free(value);
        if (self.source_repository) |repository| {
            allocator.free(repository.repository);
            allocator.free(repository.skill_path);
        }
        allocator.free(self.content_hash);
        self.* = undefined;
    }
};

pub const ImportResult = struct {
    skill_name: []const u8,
    skill_path: []const u8,
    manifest_path: []const u8,
    manifest: ImportManifest,
    actions: std.ArrayList(ImportAction) = .empty,

    pub fn deinit(self: *ImportResult, allocator: std.mem.Allocator) void {
        allocator.free(self.skill_name);
        allocator.free(self.skill_path);
        allocator.free(self.manifest_path);
        self.manifest.deinit(allocator);
        for (self.actions.items) |*action| action.deinit(allocator);
        self.actions.deinit(allocator);
        self.* = undefined;
    }
};

pub const ImportActionKind = enum {
    create_directory,
    write_skill,
    copy_file,
    write_manifest,
};

pub const ImportAction = struct {
    action: ImportActionKind,
    path: []const u8,

    pub fn deinit(self: *ImportAction, allocator: std.mem.Allocator) void {
        allocator.free(self.path);
        self.* = undefined;
    }
};

pub const SkillActionKind = enum {
    create_directory,
    create_symlink,
    remove_symlink,
    copy_file,
    write_manifest,
    remove_directory,
    skip_unchanged,
};

pub const SkillAction = struct {
    action: SkillActionKind,
    agent: ?SkillAgent = null,
    path: []const u8,
    target: ?[]const u8 = null,
    source: ?[]const u8 = null,
};

pub const ErrorKind = enum {
    validation,
    invalid_source,
    fetch,
    repository_fetch,
    collision,
    unknown_skill,
    unsupported_skill_source,
    unsupported_skill_entry,
    unsafe_agent_entry,
    enabled_import,
    already_promoted,
    not_promoted,
    io,
    serialize,
};

pub const ErrorInfo = struct {
    kind: ErrorKind,
    owned_payloads: bool = false,
    name: ?[]const u8 = null,
    path: ?[]const u8 = null,
    field: ?[]const u8 = null,
    message: ?[]const u8 = null,
    reason: ?[]const u8 = null,
    url: ?[]const u8 = null,
    repository: ?[]const u8 = null,
    actions: std.ArrayList(SkillAction) = .empty,

    pub fn deinit(self: *ErrorInfo, allocator: std.mem.Allocator) void {
        if (self.owned_payloads) {
            if (self.name) |value| allocator.free(value);
            if (self.path) |value| allocator.free(value);
            if (self.field) |value| allocator.free(value);
            if (self.message) |value| allocator.free(value);
            if (self.reason) |value| allocator.free(value);
            if (self.url) |value| allocator.free(value);
            if (self.repository) |value| allocator.free(value);
        }
        self.actions.deinit(allocator);
        self.* = undefined;
    }
};
