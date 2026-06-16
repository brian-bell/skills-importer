mod executor;
mod planner;

use crate::{
    DisableSkillRequest, DiscoveryRoots, EnableSkillRequest, SkillOperationFailure,
    SkillOperationResult,
};

pub(crate) fn enable_skill(
    roots: &DiscoveryRoots,
    request: EnableSkillRequest<'_>,
) -> Result<SkillOperationResult, SkillOperationFailure> {
    let plan = planner::plan_enable(roots, request.skill_name, request.agents)?;
    executor::execute_enable(plan)
}

pub(crate) fn disable_skill(
    roots: &DiscoveryRoots,
    request: DisableSkillRequest<'_>,
) -> Result<SkillOperationResult, SkillOperationFailure> {
    let plan = planner::plan_disable(roots, request.skill_name, request.agents)?;
    executor::execute_disable(plan)
}
