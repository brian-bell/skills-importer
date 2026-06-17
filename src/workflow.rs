use std::{fmt, io};

use crate::{
    DeleteImportRequest, DisableSkillRequest, DiscoveryRoots, EnableSkillRequest, ImportError,
    ImportLocalPathRequest, ImportMarkdownRequest, ImportRepositoryRequest, ImportResult,
    ImportUrlRequest, PromoteSkillOptions, PromoteSkillRequest, RepositoryImportResult, SkillAgent,
    SkillInventory, SkillOperationFailure, SkillOperationResult, SkillRepositoryProvider,
    SkillUrlFetcher, UnpromoteSkillRequest, delete_unpromoted_import, disable_skill,
    discover_skills, enable_skill, import_local_path_skill, import_markdown_skill,
    import_repository_skill, import_url_skill, promote_imported_skill_with_launcher,
    promotion_pr::PromotionPrLauncher, unpromote_imported_skill,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationRequest<'request> {
    List,
    ImportMarkdown(ImportMarkdownRequest<'request>),
    ImportLocalPath(ImportLocalPathRequest<'request>),
    ImportUrl(ImportUrlRequest<'request>),
    ImportRepository(ImportRepositoryRequest<'request>),
    Enable {
        skill_name: &'request str,
        agents: &'request [SkillAgent],
    },
    Disable {
        skill_name: &'request str,
        agents: &'request [SkillAgent],
    },
    Promote {
        request: PromoteSkillRequest<'request>,
        options: PromoteSkillOptions<'request>,
    },
    Unpromote(UnpromoteSkillRequest<'request>),
    Delete(DeleteImportRequest<'request>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationOutcome {
    Inventory(SkillInventory),
    Import(ImportResult),
    RepositoryImport(RepositoryImportResult),
    SkillOperation(SkillOperationResult),
}

#[derive(Debug)]
pub enum OperationError {
    Discovery(io::Error),
    Import(ImportError),
    SkillOperation(SkillOperationFailure),
}

pub fn execute(
    roots: &DiscoveryRoots,
    request: OperationRequest<'_>,
    url_fetcher: &impl SkillUrlFetcher,
    repository_provider: &impl SkillRepositoryProvider,
    promotion_launcher: &impl PromotionPrLauncher,
) -> Result<OperationOutcome, OperationError> {
    match request {
        OperationRequest::List => discover_skills(roots)
            .map(OperationOutcome::Inventory)
            .map_err(OperationError::Discovery),
        OperationRequest::ImportMarkdown(request) => import_markdown_skill(roots, request)
            .map(OperationOutcome::Import)
            .map_err(OperationError::Import),
        OperationRequest::ImportLocalPath(request) => import_local_path_skill(roots, request)
            .map(OperationOutcome::Import)
            .map_err(OperationError::Import),
        OperationRequest::ImportUrl(request) => import_url_skill(roots, request, url_fetcher)
            .map(OperationOutcome::Import)
            .map_err(OperationError::Import),
        OperationRequest::ImportRepository(request) => {
            import_repository_skill(roots, request, repository_provider)
                .map(OperationOutcome::RepositoryImport)
                .map_err(OperationError::Import)
        }
        OperationRequest::Enable { skill_name, agents } => {
            enable_skill(roots, EnableSkillRequest { skill_name, agents })
                .map(OperationOutcome::SkillOperation)
                .map_err(OperationError::SkillOperation)
        }
        OperationRequest::Disable { skill_name, agents } => {
            disable_skill(roots, DisableSkillRequest { skill_name, agents })
                .map(OperationOutcome::SkillOperation)
                .map_err(OperationError::SkillOperation)
        }
        OperationRequest::Promote { request, options } => {
            promote_imported_skill_with_launcher(roots, request, options, promotion_launcher)
                .map(OperationOutcome::SkillOperation)
                .map_err(OperationError::SkillOperation)
        }
        OperationRequest::Unpromote(request) => unpromote_imported_skill(roots, request)
            .map(OperationOutcome::SkillOperation)
            .map_err(OperationError::SkillOperation),
        OperationRequest::Delete(request) => delete_unpromoted_import(roots, request)
            .map(OperationOutcome::SkillOperation)
            .map_err(OperationError::SkillOperation),
    }
}

impl fmt::Display for OperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovery(error) => write!(formatter, "{error}"),
            Self::Import(error) => write!(formatter, "{error}"),
            Self::SkillOperation(failure) => write!(formatter, "{}", failure.error),
        }
    }
}

impl std::error::Error for OperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Discovery(error) => Some(error),
            Self::Import(error) => Some(error),
            Self::SkillOperation(failure) => Some(&failure.error),
        }
    }
}
