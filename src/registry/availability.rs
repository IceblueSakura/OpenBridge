//! Configuration-only startup availability derived from the compiled registry.
//!
//! The report groups trusted Provider families and downstream Public Models without retaining or
//! rendering credential values, pool IDs, Target IDs, Routes, endpoints, or live health. It uses
//! only the immutable startup registry and the redacted active-pool set; constructing or formatting
//! it never performs Provider egress.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::core::OperationKind;

use super::RuntimeRegistry;

/// Human-readable, configuration-only Provider and Public Model startup summary.
#[derive(Debug, Eq, PartialEq)]
pub struct ConfigurationAvailabilityReport {
    providers: AvailabilityLists,
    public_models: AvailabilityLists,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct AvailabilityLists {
    available: Vec<String>,
    unavailable: Vec<String>,
}

#[derive(Debug, Default)]
struct ProviderTargetCounts {
    total: usize,
    active_pool: usize,
    enabled: usize,
}

impl RuntimeRegistry {
    /// Builds a redacted startup report without contacting any Provider.
    pub fn configuration_availability(
        &self,
        active_pool_ids: &BTreeSet<String>,
    ) -> ConfigurationAvailabilityReport {
        // Aggregate every trusted Provider family before classifying Target eligibility.
        let mut provider_counts = self
            .provider_instances
            .values()
            .map(|instance| {
                (
                    instance.kind().slug().to_owned(),
                    ProviderTargetCounts::default(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for target in self.upstream_targets.values() {
            let counts = provider_counts
                .entry(target.kind().slug().to_owned())
                .or_default();
            counts.total += 1;
            counts.active_pool +=
                usize::from(active_pool_ids.contains(target.credential_pool_id()));
            counts.enabled += usize::from(target.enabled());
        }

        // Split Providers into deterministic available and unavailable display lists.
        let mut providers = AvailabilityLists::default();
        for (provider, counts) in provider_counts {
            if counts.enabled > 0 {
                providers.available.push(format!(
                    "{provider} ({}/{} targets)",
                    counts.enabled, counts.total
                ));
                continue;
            }
            let reason = if counts.total == 0 {
                "no registered target".to_owned()
            } else if counts.active_pool == 0 {
                format!("no active credential pool; 0/{} targets", counts.total)
            } else {
                format!("no enabled target; 0/{} targets", counts.total)
            };
            providers.unavailable.push(format!("{provider} ({reason})"));
        }

        // Classify every registered Public Model from its already compiled execution interfaces.
        let mut public_models = AvailabilityLists::default();
        for (model_id, public_model) in &self.public_models {
            let interfaces = [
                (OperationKind::ChatCompletions, "chat"),
                (OperationKind::Responses, "responses"),
                (OperationKind::EmbeddingsCreate, "embeddings"),
            ]
            .into_iter()
            .filter_map(|(operation, label)| {
                public_model
                    .execution_interface(operation)
                    .is_some()
                    .then_some(label)
            })
            .collect::<Vec<_>>();
            if public_model.is_available() {
                public_models
                    .available
                    .push(format!("{model_id} ({})", interfaces.join(", ")));
            } else {
                let reason = if interfaces.is_empty() {
                    "no executable route after configuration"
                } else {
                    "retired"
                };
                public_models
                    .unavailable
                    .push(format!("{model_id} ({reason})"));
            }
        }

        // Return an owned report so startup can render it after credential validation completes.
        ConfigurationAvailabilityReport {
            providers,
            public_models,
        }
    }
}

impl fmt::Display for ConfigurationAvailabilityReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Introduce the evidence boundary before rendering either entity table.
        writeln!(
            formatter,
            "Startup configuration availability (no network probe)"
        )?;

        // Render the Provider and Public Model lists as two independent dual-column tables.
        render_table(formatter, "Providers (configuration only)", &self.providers)?;
        writeln!(formatter)?;
        render_table(
            formatter,
            "Public models (configuration only)",
            &self.public_models,
        )
    }
}

/// Renders one deterministic available/unavailable ASCII table.
fn render_table(
    formatter: &mut fmt::Formatter<'_>,
    title: &str,
    lists: &AvailabilityLists,
) -> fmt::Result {
    // Size each column from its heading and content without depending on terminal state.
    let available_heading = format!("Available ({})", lists.available.len());
    let unavailable_heading = format!("Unavailable ({})", lists.unavailable.len());
    let available_width = column_width(&available_heading, &lists.available);
    let unavailable_width = column_width(&unavailable_heading, &lists.unavailable);

    // Write the title, headings, and stable ASCII divider.
    writeln!(formatter, "{title}")?;
    writeln!(
        formatter,
        "{available_heading:<available_width$} | {unavailable_heading}"
    )?;
    writeln!(
        formatter,
        "{}-+-{}",
        "-".repeat(available_width),
        "-".repeat(unavailable_width)
    )?;

    // Pair rows by index while preserving each independently sorted registry list.
    let row_count = lists.available.len().max(lists.unavailable.len());
    for index in 0..row_count {
        let available = lists.available.get(index).map_or("", String::as_str);
        let unavailable = lists.unavailable.get(index).map_or("", String::as_str);
        writeln!(formatter, "{available:<available_width$} | {unavailable}")?;
    }
    Ok(())
}

/// Returns the largest character width required by one table column.
fn column_width(heading: &str, entries: &[String]) -> usize {
    entries
        .iter()
        .map(|entry| entry.chars().count())
        .chain(std::iter::once(heading.chars().count()))
        .max()
        .expect("the heading always supplies one width")
}

#[cfg(test)]
mod tests {
    //! Verifies configuration classification, deterministic ordering, and redacted rendering.

    use std::collections::BTreeSet;

    use crate::{
        config::parse_bootstrap_config, providers::build_compiled_registry_with_active_pools,
    };

    #[test]
    fn production_report_uses_only_configuration_eligibility_and_redacted_names() {
        // Compile the production catalog with only the MiMo credential pool activated.
        let active_pool_ids = BTreeSet::from(["mimo-primary".to_owned()]);
        let bootstrap =
            parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
        let registry = build_compiled_registry_with_active_pools(bootstrap, &active_pool_ids)
            .expect("production registry should compile");

        // Verify both entity lists use deterministic configuration-only classification.
        let report = registry.configuration_availability(&active_pool_ids);
        assert_eq!(report.providers.available, ["mimo (6/6 targets)"]);
        assert!(
            report
                .providers
                .unavailable
                .iter()
                .any(|entry| entry.starts_with("openai (no active credential pool;"))
        );
        assert_eq!(
            report.public_models.available,
            [
                "mimo-v2.5 (chat, responses)",
                "mimo-v2.5-asr (chat)",
                "mimo-v2.5-pro (chat, responses)",
                "mimo-v2.5-tts (chat)",
                "mimo-v2.5-tts-voiceclone (chat)",
                "mimo-v2.5-tts-voicedesign (chat)",
            ]
        );
        assert!(
            report
                .public_models
                .unavailable
                .iter()
                .any(|entry| entry == "gpt-5.6-sol (no executable route after configuration)")
        );
        assert!(is_sorted(&report.providers.unavailable));
        assert!(is_sorted(&report.public_models.unavailable));

        // Render the report and keep internal topology and private configuration values absent.
        let rendered = report.to_string();
        for forbidden in [
            "mimo-primary",
            "openai-primary",
            "mimo-v2-5",
            "openai-main",
            "https://",
            "synthetic",
        ] {
            assert!(!rendered.contains(forbidden), "leaked '{forbidden}'");
        }
    }

    #[test]
    fn dual_list_renderer_keeps_empty_columns_and_counts_visible() {
        // Build isolated lists that exercise each table with one empty side.
        let report = super::ConfigurationAvailabilityReport {
            providers: super::AvailabilityLists {
                available: Vec::new(),
                unavailable: vec!["openai (no active credential pool; 0/2 targets)".to_owned()],
            },
            public_models: super::AvailabilityLists {
                available: vec!["mimo-v2.5 (chat, responses)".to_owned()],
                unavailable: Vec::new(),
            },
        };

        // Keep both headings, separators, and rows readable when either side has no entries.
        let rendered = report.to_string();
        assert!(rendered.contains("Available (0) | Unavailable (1)"));
        assert!(rendered.contains("Available (1)"));
        assert!(rendered.contains("Unavailable (0)"));
        assert!(rendered.contains(" | openai (no active credential pool; 0/2 targets)"));
        assert!(rendered.contains("mimo-v2.5 (chat, responses)"));
    }

    /// Returns whether display entries are already in deterministic lexical order.
    fn is_sorted(entries: &[String]) -> bool {
        entries.windows(2).all(|pair| pair[0] < pair[1])
    }
}
