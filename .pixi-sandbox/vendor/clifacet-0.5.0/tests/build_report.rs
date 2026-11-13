//! The build-script report and what it becomes in the source (CLI-50, decision-19 item 4).
//!
//! Discovery is split in two so each half can be checked without the other: the build
//! script scans a folder into a [`Report`], and the report becomes the `mod` lines the
//! macro emits. This file drives both against real folders, because the things worth
//! getting right — which file is a command, which folder is a group, which is a helper —
//! are facts about a directory, not about a parser.

#![cfg(feature = "build")]

use std::path::Path;

use clifacet::build::{PROTOCOL, Report, Reports, VARIABLE, declarations};

/// A folder to scan, removed when the test ends.
struct Folder(std::path::PathBuf);

impl Folder {
    /// A folder with exactly these entries: a file per `name.rs`, a folder per `name/`.
    fn with(entries: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "clifacet-build-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a temporary folder");
        for entry in entries {
            let path = root.join(entry);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("a nested folder");
            }
            if entry.ends_with('/') {
                std::fs::create_dir_all(&path).expect("a folder entry");
            } else {
                std::fs::write(&path, "// a module\n").expect("a file entry");
            }
        }
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_is_a_command_and_a_folder_is_a_group() {
    let folder = Folder::with(&["list.rs", "show.rs", "db/", "mod.rs"]);

    let report = Report::of(folder.path()).expect("the folder reads");

    assert_eq!(report.commands, ["list", "show"]);
    assert_eq!(report.groups, ["db"]);
    assert!(report.helpers.is_empty(), "no helper was written");
}

#[test]
fn mod_rs_is_the_folder_itself_and_never_a_command() {
    let folder = Folder::with(&["mod.rs"]);

    let report = Report::of(folder.path()).expect("the folder reads");

    assert_eq!(
        report,
        Report {
            folder: folder.path().to_path_buf(),
            ..Report::default()
        }
    );
}

#[test]
fn a_leading_underscore_makes_a_helper_and_never_a_group() {
    let folder = Folder::with(&["list.rs", "_shared.rs", "_task/"]);

    let report = Report::of(folder.path()).expect("the folder reads");

    assert_eq!(report.commands, ["list"]);
    assert_eq!(
        report.helpers,
        ["_shared", "_task"],
        "keeping the underscore"
    );
    assert!(
        report.groups.is_empty(),
        "a `_` folder is a helper, so it is never a group"
    );
}

#[test]
fn the_order_is_stable_whatever_the_filesystem_says() {
    let folder = Folder::with(&["zebra.rs", "alpha.rs", "middle.rs"]);

    let report = Report::of(folder.path()).expect("the folder reads");

    assert_eq!(report.commands, ["alpha", "middle", "zebra"]);
    assert_eq!(
        report,
        Report::of(folder.path()).expect("the folder reads"),
        "two scans of one folder agree, so an unchanged crate does not rebuild"
    );
}

#[test]
fn what_is_not_a_module_is_skipped() {
    let folder = Folder::with(&["list.rs", "list.rs~", "notes.txt", "mod.rs.bak"]);

    let report = Report::of(folder.path()).expect("the folder reads");

    assert_eq!(report.commands, ["list"]);
}

#[test]
fn a_missing_folder_is_an_error_naming_it() {
    let error = Report::of("/nonexistent/clifacet/commands").expect_err("cannot read");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

// --- the report travels in an environment variable ---------------------------------

#[test]
fn a_report_round_trips_through_the_variable() {
    let report = Report {
        folder: std::path::PathBuf::from("src/commands"),
        commands: vec![String::from("list"), String::from("show")],
        groups: vec![String::from("db")],
        helpers: vec![String::from("_shared")],
        entries: Vec::new(),
    };
    let reports = Reports {
        folders: vec![report.clone()],
    };

    let decoded = Reports::decode(&reports.encode()).expect("a report this crate wrote");

    assert_eq!(decoded, reports);
    assert!(
        reports
            .encode()
            .starts_with(&format!("clifacet-commands/{PROTOCOL}")),
        "the value says which protocol it speaks: {}",
        reports.encode()
    );
}

#[test]
fn a_tree_of_folders_travels_in_one_value() {
    let parent = Report {
        folder: std::path::PathBuf::from("src/commands"),
        commands: vec![String::from("list")],
        groups: vec![String::from("db")],
        helpers: Vec::new(),
        entries: Vec::new(),
    };
    let group = Report {
        folder: std::path::PathBuf::from("src/commands/db"),
        commands: vec![String::from("migrate")],
        groups: Vec::new(),
        helpers: Vec::new(),
        entries: Vec::new(),
    };
    let reports = Reports {
        folders: vec![parent, group.clone()],
    };

    let decoded = Reports::decode(&reports.encode()).expect("a report this crate wrote");

    assert_eq!(
        decoded.of("src/commands/db"),
        Some(&group),
        "a group is a folder with its own report, found by its own path"
    );
    assert!(
        decoded.of("src/commands/nowhere").is_none(),
        "a folder the build script did not scan has no report"
    );
}

#[test]
fn a_value_from_another_protocol_is_refused_with_the_reason() {
    let error = Reports::decode("clifacet-commands/99;folder=src/commands")
        .expect_err("a newer protocol is not guessed at");

    assert!(error.contains("99"), "{error}");
    assert!(error.contains(VARIABLE), "{error}");
}

#[test]
fn a_value_that_is_not_a_report_is_refused() {
    let error = Reports::decode("just some text").expect_err("not a report");

    assert!(error.contains(VARIABLE), "{error}");
    assert!(error.contains("build.rs"), "it says who writes it: {error}");
}

#[test]
fn an_unknown_record_is_refused_rather_than_dropped() {
    let error = Reports::decode(&format!("clifacet-commands/{PROTOCOL};nonsense=1"))
        .expect_err("an unknown key means a protocol this crate does not know");

    assert!(error.contains("nonsense"), "{error}");
}

#[test]
fn a_command_before_its_folder_is_refused() {
    let error = Reports::decode(&format!("clifacet-commands/{PROTOCOL};command=list"))
        .expect_err("a report starts with the folder it describes");

    assert!(error.contains("folder"), "{error}");
}

// --- the report becomes source -------------------------------------------------------

#[test]
fn commands_and_groups_are_public_and_helpers_are_private() {
    let report = Report {
        folder: std::path::PathBuf::from("src/commands"),
        commands: vec![String::from("list")],
        groups: vec![String::from("db")],
        helpers: vec![String::from("_shared")],
        entries: Vec::new(),
    };

    let source = declarations(&report);

    assert_eq!(
        source, "pub mod list;\npub mod db;\nmod _shared;\n",
        "a helper is the folder's own business, so it is never re-exported"
    );
}

#[test]
fn an_empty_folder_declares_nothing() {
    let report = Report::default();

    assert_eq!(declarations(&report), "");
}
