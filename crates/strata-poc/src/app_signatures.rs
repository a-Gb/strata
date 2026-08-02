//! External signature-pack loading, scan coordination, and evidence presentation.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]

use super::*;

impl StrataPoc {
    pub(super) fn browse_signature_pack(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open UFSC signature knowledge pack")
            .add_filter("UFSC JSON", &["json"])
            .pick_file()
        {
            self.signature_pack_path_input = path.display().to_string();
            self.load_signature_pack();
        }
    }

    pub(super) fn load_signature_pack(&mut self) {
        let input = self.signature_pack_path_input.trim();
        if input.is_empty() {
            "Choose a UFSC JSON signature pack first".clone_into(&mut self.status);
            return;
        }
        let path = PathBuf::from(input);
        if let Err(error) = self.load_signature_pack_path(&path, None) {
            self.status = error;
        }
    }

    pub(super) fn load_signature_pack_path(
        &mut self,
        path: &Path,
        expected_sha256: Option<&str>,
    ) -> Result<(), String> {
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                return Err(format!(
                    "Cannot inspect signature pack {}: {error}",
                    path.display()
                ));
            }
        };
        let maximum = u64::try_from(strata_analysis::signatures::MAX_SIGNATURE_PACK_BYTES)
            .unwrap_or(u64::MAX);
        if metadata.len() > maximum {
            return Err(format!(
                "Signature pack is {} bytes; bounded maximum is {maximum}",
                metadata.len()
            ));
        }
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Err(format!(
                    "Cannot read signature pack {}: {error}",
                    path.display()
                ));
            }
        };
        let catalog = match SignatureCatalog::from_ufsc_json(&bytes) {
            Ok(catalog) => Arc::new(catalog),
            Err(error) => {
                return Err(format!(
                    "Cannot import signature pack {}: {error}",
                    path.display()
                ));
            }
        };
        if let Some(expected) = expected_sha256
            && !catalog.digest().eq_ignore_ascii_case(expected)
        {
            return Err(format!(
                "Signature pack changed · expected {}… · got {}…",
                digest_prefix(expected),
                digest_prefix(catalog.digest())
            ));
        }
        let stats = catalog.stats();
        self.signature_pack_status = format!(
            "{} {} · {} accepted / {} skipped · SHA-256 {}…",
            catalog.name(),
            catalog.version(),
            stats.accepted_rules,
            stats.skipped_records(),
            digest_prefix(catalog.digest())
        );
        self.signature_pack_path_input = path.display().to_string();
        self.signature_catalog = Some(catalog);
        self.projection_sample_key = None;
        self.projection_field_key = None;
        self.recompute_discovery();
        self.rebuild_workspace_models();
        self.invalidate_texture();
        self.status = format!(
            "Loaded signature knowledge pack read-only: {}",
            path.display()
        );
        Ok(())
    }

    pub(super) fn clear_signature_pack(&mut self) {
        self.signature_catalog = None;
        self.projection_sample_key = None;
        self.projection_field_key = None;
        "Built-in five-signature fallback active".clone_into(&mut self.signature_pack_status);
        "No external knowledge pack loaded".clone_into(&mut self.signature_scan_status);
        self.recompute_discovery();
        self.rebuild_workspace_models();
        self.invalidate_texture();
        "External signature knowledge cleared".clone_into(&mut self.status);
    }

    pub(super) fn external_signature_leads(
        &self,
    ) -> Result<Option<(Vec<WorkbenchLead>, String)>, strata_core::DomainError> {
        let Some(catalog) = self.signature_catalog.as_ref() else {
            return Ok(None);
        };
        if self
            .loaded_source
            .as_ref()
            .is_some_and(|source| source.sampled_overview)
        {
            return Ok(Some((
                Vec::new(),
                "Catalog scan paused: sampled overview bytes are not a contiguous exact source range"
                    .to_owned(),
            )));
        }
        let report = catalog.scan(
            self.source_bytes(),
            SignatureScanConfig {
                max_matches: 32,
                ..SignatureScanConfig::default()
            },
        )?;
        let inspected = report.embedded_inspected_range.len();
        let status = format!(
            "{} catalog match(es) · embedded search 0x0..0x{inspected:x}{}",
            report.matches.len(),
            if report.truncated {
                " · BOUNDED/TRUNCATED"
            } else {
                ""
            }
        );
        Ok(Some((catalog_signature_leads(&report), status)))
    }

    pub(super) fn show_signature_pack_inspector(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("SIGNATURE KNOWLEDGE")
                .strong()
                .size(10.5)
                .color(UI_TEAL),
        );
        ui.add_sized(
            [ui.available_width(), RAIL_CONTROL_HEIGHT],
            egui::TextEdit::singleline(&mut self.signature_pack_path_input)
                .hint_text("UFSC JSON knowledge pack…"),
        );
        ui.columns(3, |columns| {
            if rail_action(&mut columns[0], "Browse…") {
                self.browse_signature_pack();
            }
            if rail_action(&mut columns[1], "Load pack") {
                self.load_signature_pack();
            }
            if rail_action_enabled(&mut columns[2], self.signature_catalog.is_some(), "Clear") {
                self.clear_signature_pack();
            }
        });
        ui.small(&self.signature_pack_status);
        ui.monospace(&self.signature_scan_status);
        ui.weak("Catalog metadata is candidate evidence; colour never asserts a parsed file type.");
    }

    pub(super) fn signature_evidence_for_projection(
        &self,
        offsets: [usize; 3],
        analysis_range: [usize; 2],
    ) -> Option<&SignatureMatchEvidence> {
        self.discovery_findings.iter().find_map(|finding| {
            let WorkbenchEvidence::CatalogSignature(evidence) = &finding.evidence else {
                return None;
            };
            finding
                .source_ranges
                .iter()
                .any(|range| {
                    let contributor_matches = offsets.iter().any(|offset| {
                        u64::try_from(*offset).is_ok_and(|offset| range.contains(offset))
                    });
                    let analysis_overlaps = u64::try_from(analysis_range[0])
                        .ok()
                        .zip(u64::try_from(analysis_range[1]).ok())
                        .is_some_and(|(start, end)| range.start < end && start < range.end);
                    contributor_matches || analysis_overlaps
                })
                .then_some(evidence)
        })
    }

    pub(super) fn visible_signature_evidence(
        &self,
        point: &ScreenProjection,
    ) -> Option<&SignatureMatchEvidence> {
        self.projection_composition
            .overlays
            .signatures
            .then(|| {
                self.signature_evidence_for_projection(point.source_offsets, point.analysis_range)
            })
            .flatten()
    }
}

pub(super) fn signature_projection_offsets(findings: &[WorkbenchLead]) -> Vec<usize> {
    findings
        .iter()
        .filter(|finding| finding.kind == WorkbenchLeadKind::CatalogSignature)
        .flat_map(|finding| finding.source_ranges.iter())
        .filter_map(|range| usize::try_from(range.start).ok())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn show_signature_match_evidence(ui: &mut egui::Ui, evidence: &SignatureMatchEvidence) {
    ui.separator();
    ui.label(
        egui::RichText::new("CATALOG EVIDENCE")
            .strong()
            .size(10.5)
            .color(UI_TEAL),
    );
    ui.monospace(&evidence.pattern_hex);
    let mode = match evidence.mode {
        strata_analysis::signatures::SignatureMatchMode::DeclaredOffset => "DECLARED OFFSET AGREES",
        strata_analysis::signatures::SignatureMatchMode::EmbeddedSearch => {
            "EMBEDDED SEARCH · OFFSET RELAXED"
        }
    };
    ui.colored_label(
        if evidence.mode == strata_analysis::signatures::SignatureMatchMode::DeclaredOffset {
            UI_TEAL
        } else {
            UI_AMBER
        },
        mode,
    );
    for candidate in evidence.candidates.iter().take(6) {
        ui.label(egui::RichText::new(&candidate.label).strong());
        if !candidate.categories.is_empty() {
            ui.weak(candidate.categories.join(" · "));
        }
        for source in candidate.sources.iter().take(3) {
            ui.small(format!("{} · {}", source.name, source.retrieved_at));
        }
    }
    if evidence.candidates.len() > 6 {
        ui.weak(format!(
            "+ {} competing catalog interpretations",
            evidence.candidates.len().saturating_sub(6)
        ));
    }
    ui.small(format!(
        "{} {} · SHA-256 {}…",
        evidence.catalog_name,
        evidence.catalog_version,
        digest_prefix(&evidence.catalog_digest)
    ));
}

pub(super) fn signature_category_color(evidence: &SignatureMatchEvidence) -> egui::Color32 {
    let categories = evidence
        .candidates
        .iter()
        .flat_map(|candidate| candidate.categories.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    if categories
        .iter()
        .any(|category| category.contains("executable") || category.contains("firmware"))
    {
        egui::Color32::from_rgb(188, 105, 105)
    } else if categories
        .iter()
        .any(|category| category.contains("archive") || category.contains("compress"))
    {
        egui::Color32::from_rgb(190, 145, 82)
    } else if categories.iter().any(|category| category.contains("image")) {
        egui::Color32::from_rgb(151, 127, 188)
    } else if categories
        .iter()
        .any(|category| category.contains("audio") || category.contains("video"))
    {
        egui::Color32::from_rgb(102, 151, 190)
    } else if categories
        .iter()
        .any(|category| category.contains("document") || category.contains("text"))
    {
        egui::Color32::from_rgb(94, 168, 139)
    } else {
        egui::Color32::from_rgb(154, 166, 174)
    }
}
