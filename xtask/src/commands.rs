use crate::command_args;
use std::path::PathBuf;

pub(crate) enum XtaskCommand {
    Help,
    CheckPr,
    CheckDocs,
    CheckPolicy,
    CheckDocArtifacts,
    CheckDocsAutomation,
    CheckSpecStatus,
    CheckPublicSurfaces,
    CheckGoals,
    CheckPackageBoundary,
    CheckCiLanes,
    CheckSupportTiers,
    CheckFixtures,
    CheckCalibration,
    CheckDogfood,
    CheckFuzz,
    CheckAdvisoryArtifacts(PathBuf),
    CheckFirstPrArtifacts(PathBuf),
    CheckManualCandidateExamples,
    CheckFirstHour,
    DogfoodUsefulness,
    SyncCalibrationSnapshot,
    SourceDivergence,
}

impl XtaskCommand {
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        match args.get(1).map(|arg| arg.as_str()) {
            None | Some("help") | Some("--help") => Ok(Self::Help),
            Some("check-pr") => parse_no_extra(args, "check-pr", Self::CheckPr),
            Some("check-docs") => parse_no_extra(args, "check-docs", Self::CheckDocs),
            Some("check-policy") => parse_no_extra(args, "check-policy", Self::CheckPolicy),
            Some("check-doc-artifacts") => {
                parse_no_extra(args, "check-doc-artifacts", Self::CheckDocArtifacts)
            }
            Some("check-docs-automation") => {
                parse_no_extra(args, "check-docs-automation", Self::CheckDocsAutomation)
            }
            Some("check-spec-status") => {
                parse_no_extra(args, "check-spec-status", Self::CheckSpecStatus)
            }
            Some("check-public-surfaces") => {
                parse_no_extra(args, "check-public-surfaces", Self::CheckPublicSurfaces)
            }
            Some("check-goals") => parse_no_extra(args, "check-goals", Self::CheckGoals),
            Some("check-package-boundary") => {
                parse_no_extra(args, "check-package-boundary", Self::CheckPackageBoundary)
            }
            Some("check-ci-lanes") => parse_no_extra(args, "check-ci-lanes", Self::CheckCiLanes),
            Some("check-support-tiers") => {
                parse_no_extra(args, "check-support-tiers", Self::CheckSupportTiers)
            }
            Some("check-fixtures") => parse_no_extra(args, "check-fixtures", Self::CheckFixtures),
            Some("check-calibration") => {
                parse_no_extra(args, "check-calibration", Self::CheckCalibration)
            }
            Some("check-dogfood") => parse_no_extra(args, "check-dogfood", Self::CheckDogfood),
            Some("check-fuzz") => parse_no_extra(args, "check-fuzz", Self::CheckFuzz),
            Some("check-advisory-artifacts") => Ok(Self::CheckAdvisoryArtifacts(
                command_args::require_subcommand_dir_arg(args, "check-advisory-artifacts")?,
            )),
            Some("check-first-pr-artifacts") => Ok(Self::CheckFirstPrArtifacts(
                command_args::require_subcommand_dir_arg(args, "check-first-pr-artifacts")?,
            )),
            Some("check-manual-candidate-examples") => parse_no_extra(
                args,
                "check-manual-candidate-examples",
                Self::CheckManualCandidateExamples,
            ),
            Some("check-first-hour") => {
                parse_no_extra(args, "check-first-hour", Self::CheckFirstHour)
            }
            Some("dogfood-usefulness") => {
                parse_no_extra(args, "dogfood-usefulness", Self::DogfoodUsefulness)
            }
            Some("sync-calibration-snapshot") => parse_no_extra(
                args,
                "sync-calibration-snapshot",
                Self::SyncCalibrationSnapshot,
            ),
            Some("source-divergence") | Some("check-source-sync") => {
                parse_no_extra(args, "source-divergence", Self::SourceDivergence)
            }
            Some(other) => Err(format!("unknown xtask command `{other}`")),
        }
    }
}

fn parse_no_extra<T>(args: &[String], name: &str, value: T) -> Result<T, String> {
    command_args::require_no_extra_args(args, name)?;
    Ok(value)
}
