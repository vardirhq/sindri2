//! Registering steps, and what the chain does with a version it cannot reach.

use serde_json::json;

use crate::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneJsonError, SceneMigrationError, SceneMigrator,
};

use super::support::{legacy_document, rename_label_to_name};

#[test]
fn current_documents_pass_through_untouched() {
    let migrator = SceneMigrator::new();
    assert!(migrator.is_empty());
    let document = json!({ "format_version": SCENE_FORMAT_VERSION, "entities": [] });
    assert_eq!(migrator.migrate(document.clone()).unwrap(), document);
}

#[test]
fn registered_steps_upgrade_older_documents() {
    let mut migrator = SceneMigrator::builtin();
    migrator.register(0, 1, rename_label_to_name).unwrap();

    let document = SceneDocument::from_json_migrated(&legacy_document(), &migrator).unwrap();
    assert_eq!(document.format_version, SCENE_FORMAT_VERSION);
    assert_eq!(document.entities[0].name.as_deref(), Some("Player"));
}

#[test]
fn unmigrated_versions_report_the_missing_step() {
    let migrator = SceneMigrator::new();
    let error = SceneDocument::from_json_migrated(&legacy_document(), &migrator).unwrap_err();
    assert!(matches!(
        error,
        SceneJsonError::Migration(SceneMigrationError::NoRegisteredStep {
            from_version: 0,
            supported: SCENE_FORMAT_VERSION,
        })
    ));
}

#[test]
fn newer_documents_are_rejected_rather_than_guessed_at() {
    let migrator = SceneMigrator::new();
    let document = json!({ "format_version": SCENE_FORMAT_VERSION + 1, "entities": [] });
    assert_eq!(
        migrator.migrate(document),
        Err(SceneMigrationError::FromTheFuture {
            found: SCENE_FORMAT_VERSION + 1,
            supported: SCENE_FORMAT_VERSION,
        })
    );
}

#[test]
fn registration_rejects_loops_duplicates_and_overshoot() {
    let mut migrator = SceneMigrator::new();
    assert_eq!(
        migrator.register(1, 1, rename_label_to_name),
        Err(SceneMigrationError::NonProgressingStep {
            from_version: 1,
            to_version: 1,
        })
    );
    assert_eq!(
        migrator.register(1, SCENE_FORMAT_VERSION + 1, rename_label_to_name),
        Err(SceneMigrationError::StepBeyondSupportedVersion {
            to_version: SCENE_FORMAT_VERSION + 1,
            supported: SCENE_FORMAT_VERSION,
        })
    );
    migrator.register(0, 1, rename_label_to_name).unwrap();
    assert_eq!(
        migrator.register(0, 1, rename_label_to_name),
        Err(SceneMigrationError::DuplicateStep { from_version: 0 })
    );
}

#[test]
fn documents_without_a_version_are_rejected() {
    let migrator = SceneMigrator::new();
    assert_eq!(
        migrator.migrate(json!({ "entities": [] })),
        Err(SceneMigrationError::MissingFormatVersion)
    );
    assert_eq!(
        migrator.migrate(json!(["not", "a", "document"])),
        Err(SceneMigrationError::NotADocument)
    );
}

#[test]
fn a_failing_step_surfaces_its_reason() {
    let mut migrator = SceneMigrator::new();
    migrator.register(0, 1, rename_label_to_name).unwrap();
    let error = migrator
        .migrate(json!({ "format_version": 0 }))
        .unwrap_err();
    assert!(matches!(
        error,
        SceneMigrationError::StepFailed {
            from_version: 0,
            ..
        }
    ));
}
