//! Read-only `environment` command: name the configuration envelope a
//! result would be computed under, without running the analysis.
//!
//! Output is an identity record only: envelope source, workspace, packages,
//! targets, feature selection, toolchain facts, unknown inputs, and the
//! environment digest. It carries no safety, coverage, or applicability
//! claim: cfg evaluation and card filtering belong to later slices (#2318
//! PR2+). Unknown inputs stay visible; unknown never becomes inactive.

use crate::command::{EnvFeatureSelect, EnvOptions, Format};
use unsafe_review_core::{
    EnvDiscoverOptions, EnvironmentSource, FeatureSelection, discover_environment,
    render_environment_human,
};

pub(crate) fn run(options: &EnvOptions) -> Result<(), String> {
    let features = match &options.features {
        EnvFeatureSelect::Default => FeatureSelection::DefaultFeatures,
        EnvFeatureSelect::NoDefault => FeatureSelection::NoDefaultFeatures,
        EnvFeatureSelect::Explicit(selected) => FeatureSelection::Explicit(selected.clone()),
        EnvFeatureSelect::All => FeatureSelection::AllFeatures,
    };
    let env = discover_environment(
        &options.root,
        EnvironmentSource::ExplicitCli,
        &EnvDiscoverOptions {
            probe_toolchain: options.probe_toolchain,
            expand_members: options.expand_members,
            features,
        },
    )?;
    match options.format {
        Format::Human => {
            println!("{}", render_environment_human(&env));
        }
        Format::Json => {
            let environment = serde_json::to_value(&env)
                .map_err(|err| format!("serialize environment failed: {err}"))?;
            let combined = serde_json::json!({ "environment": environment });
            println!(
                "{}",
                serde_json::to_string_pretty(&combined)
                    .map_err(|err| format!("serialize environment output failed: {err}"))?
            );
        }
        _ => {
            return Err(
                "unsupported environment format; environment projects `human` and `json` only"
                    .to_string(),
            );
        }
    }
    Ok(())
}
