//! Local project open/save flow and persisted launch preferences.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]

use super::*;

impl StrataPoc {
    pub(super) fn save_project_from_input(&mut self) {
        let path = PathBuf::from(self.project_path_input.trim());
        if path.as_os_str().is_empty() {
            "Choose a local project file first".clone_into(&mut self.status);
            return;
        }
        self.save_local_project_path(&path);
    }

    pub(super) fn save_local_project_path(&mut self, path: &Path) {
        let path = match absolutized_path(&normalized_project_path(path)) {
            Ok(path) => path,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        self.project_path_input = path.display().to_string();
        if !self.prepare_session_save() {
            let waiting_for_digest = self.session_attached
                && self
                    .analysis_source
                    .as_ref()
                    .is_some_and(|source| source.descriptor().content_digest.is_none());
            if waiting_for_digest {
                self.pending_project_save = Some(path);
                "Project save queued until the whole-source SHA-256 is sealed"
                    .clone_into(&mut self.status);
            }
            return;
        }
        match self.persist_local_project_now(&path) {
            Ok(()) => {
                self.pending_project_save = None;
                let message = format!(
                    "Saved local project {} · page, ranges, controls, camera, and signature pack pinned",
                    path.display()
                );
                self.status = match self.persist_project_preferences() {
                    Ok(()) => message,
                    Err(error) => format!("{message} · preferences not saved: {error}"),
                };
            }
            Err(error) => self.status = error,
        }
    }

    pub(super) fn complete_pending_project_save(&mut self) {
        let Some(path) = self.pending_project_save.take() else {
            return;
        };
        self.save_local_project_path(&path);
    }

    fn persist_local_project_now(&mut self, project_path: &Path) -> Result<(), String> {
        let source_path = self
            .loaded_source
            .as_ref()
            .map(|source| source.path.clone())
            .ok_or_else(|| {
                "A local project requires a local source; open a custom file first".to_owned()
            })?;
        let signature_pack = self
            .signature_catalog
            .as_ref()
            .map(|catalog| {
                let path = PathBuf::from(self.signature_pack_path_input.trim());
                if path.as_os_str().is_empty() {
                    return Err("loaded signature pack has no local locator".to_owned());
                }
                Ok((path, catalog.digest().to_owned()))
            })
            .transpose()?;
        let session_path = derived_session_path(project_path)?;
        self.persist_session_bundle(&session_path)?;
        let project = LocalProjectFile::new(
            project_path,
            &session_path,
            &source_path,
            signature_pack
                .as_ref()
                .map(|(path, digest)| (path.as_path(), digest.as_str())),
        )?;
        save_local_project(project_path, &project)?;
        self.session_path_input = session_path.display().to_string();
        self.project_path_input = project_path.display().to_string();
        Ok(())
    }

    pub(super) fn open_project_from_input(&mut self) {
        let path = PathBuf::from(self.project_path_input.trim());
        if path.as_os_str().is_empty() {
            "Choose a local project file first".clone_into(&mut self.status);
            return;
        }
        if let Err(error) = self.open_local_project_path(&path) {
            self.status = error;
        }
    }

    pub(super) fn open_local_project_path(&mut self, project_path: &Path) -> Result<(), String> {
        let project_path = absolutized_path(project_path)?;
        let project = load_local_project(&project_path)?;
        let session_path = project.resolved_session_bundle(&project_path);
        let source_path = project.resolved_source(&project_path);
        self.open_session_path(&session_path)?;

        if let Some(signature) = &project.signature_pack {
            let signature_path = project
                .resolved_signature_pack(&project_path)
                .ok_or_else(|| "project signature-pack locator disappeared".to_owned())?;
            self.load_signature_pack_path(&signature_path, Some(&signature.sha256))?;
        } else if self.signature_catalog.is_some() {
            self.clear_signature_pack();
        }

        self.project_path_input = project_path.display().to_string();
        self.path_input = source_path.display().to_string();
        self.reattach_session_path();
        let message = format!(
            "Opening local project {} · verifying source before restoring byte-dependent views…",
            project_path.display()
        );
        self.status = match self.persist_project_preferences() {
            Ok(()) => message,
            Err(error) => format!("{message} · preferences not saved: {error}"),
        };
        Ok(())
    }

    pub(super) fn browse_open_local_project(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open Strata local project")
            .add_filter("Strata local project", &["strata-project"]);
        if let Some(parent) = project_dialog_parent(&self.project_path_input) {
            dialog = dialog.set_directory(parent);
        }
        if let Some(path) = dialog.pick_file() {
            self.project_path_input = path.display().to_string();
            self.open_project_from_input();
        }
    }

    pub(super) fn browse_save_local_project(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save Strata local project")
            .add_filter("Strata local project", &["strata-project"])
            .set_file_name(default_project_file_name(self.loaded_source.as_ref()));
        if let Some(parent) = project_dialog_parent(&self.project_path_input) {
            dialog = dialog.set_directory(parent);
        }
        if let Some(path) = dialog.save_file() {
            self.save_local_project_path(&path);
        }
    }

    pub(super) fn show_project_preferences_window(&mut self, context: &egui::Context) {
        if !self.show_project_preferences {
            return;
        }
        let mut open = self.show_project_preferences;
        egui::Window::new("Project preferences")
            .id(egui::Id::new("strata-project-preferences"))
            .open(&mut open)
            .default_width(520.0)
            .resizable(false)
            .collapsible(false)
            .show(context, |ui| {
                self.show_local_project_preferences(ui);
                ui.separator();
                self.show_project_signature_preferences(ui);
                ui.separator();
                show_project_checkpoint_contents(ui);
            });
        self.show_project_preferences = open;
    }

    fn show_local_project_preferences(&mut self, ui: &mut egui::Ui) {
        project_preferences_heading(ui, "LOCAL PROJECT");
        ui.weak(
            "One-click local locator; its referenced session remains source-free and digest-gated.",
        );
        ui.add_sized(
            [ui.available_width(), RAIL_CONTROL_HEIGHT],
            egui::TextEdit::singleline(&mut self.project_path_input)
                .hint_text("analysis.strata-project"),
        );
        ui.columns(3, |columns| {
            if rail_action(&mut columns[0], "Open…") {
                self.browse_open_local_project();
            }
            if rail_action(&mut columns[1], "Open path") {
                self.open_project_from_input();
            }
            if rail_action(&mut columns[2], "Save as…") {
                self.browse_save_local_project();
            }
        });
        if rail_action_enabled(
            ui,
            !self.project_path_input.trim().is_empty() && self.loaded_source.is_some(),
            "Save current state to project path",
        ) {
            self.save_project_from_input();
        }
        let reopen_changed = ui
            .checkbox(
                &mut self.reopen_last_project,
                "Reopen this local project on next launch",
            )
            .changed();
        if self.project_path_input.trim().is_empty() {
            self.reopen_last_project = false;
        }
        if reopen_changed && let Err(error) = self.persist_project_preferences() {
            self.status = format!("Cannot save project preferences: {error}");
        }
    }

    fn show_project_signature_preferences(&mut self, ui: &mut egui::Ui) {
        project_preferences_heading(ui, "DEFAULT SIGNATURE KNOWLEDGE");
        ui.weak("Used for ordinary source opens; a project pins its own pack and digest.");
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
        if rail_action(ui, "Save these launch preferences")
            && let Err(error) = self.persist_project_preferences()
        {
            self.status = format!("Cannot save project preferences: {error}");
        }
    }

    pub(super) fn project_preferences(&self) -> ProjectPreferences {
        ProjectPreferences {
            version: LOCAL_PROJECT_VERSION,
            reopen_last_project: self.reopen_last_project,
            last_project_path: self.project_path_input.trim().to_owned(),
            default_signature_pack_path: self.signature_pack_path_input.trim().to_owned(),
        }
    }

    pub(super) fn persist_project_preferences(&self) -> Result<(), String> {
        save_project_preferences_file(&self.project_preferences_path, &self.project_preferences())
    }
}

fn project_preferences_heading(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label)
            .strong()
            .size(10.5)
            .color(UI_TEAL),
    );
}

fn show_project_checkpoint_contents(ui: &mut egui::Ui) {
    project_preferences_heading(ui, "CHECKPOINT CONTENTS");
    ui.monospace("page + workbench mode + exact byte ranges + evidence state");
    ui.monospace("projection A/B + geometry + channels + sampling + camera");
    ui.monospace("structure / grammar / interleave / resonance controls");
    ui.add_space(4.0);
    ui.colored_label(
        UI_AMBER,
        "Local project files contain filesystem paths. Share the .strata-session bundle instead.",
    );
}

fn project_dialog_parent(input: &str) -> Option<PathBuf> {
    Path::new(input.trim())
        .parent()
        .filter(|parent| parent.is_dir())
        .map(Path::to_owned)
}

fn default_project_file_name(source: Option<&LoadedSource>) -> String {
    let stem = source
        .and_then(|source| source.path.file_stem())
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("strata-investigation");
    format!("{stem}{LOCAL_PROJECT_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_persisted_preferences_fail_closed() {
        let preferences = ProjectPreferences {
            reopen_last_project: true,
            last_project_path: String::new(),
            ..ProjectPreferences::default()
        };
        assert!(preferences.validate().is_err());
    }

    #[test]
    fn project_filename_uses_stable_suffix() {
        assert_eq!(
            default_project_file_name(None),
            "strata-investigation.strata-project"
        );
    }
}
