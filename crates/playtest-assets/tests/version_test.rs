//! Integration tests for `playtest-assets` (`Spec 0012 §2`).
//!
//! Verifies workspace version synchronization, per-language embedded file lists,
//! absence of test/lockfile/cache/editor artifacts, and byte-for-byte parity with
//! canonical monorepo SDK files when running inside the repository checkout.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use playtest_assets::{
    SdkLanguage, VERSION, all_embedded_sdks, embedded_files, get_embedded_file, sdk_version,
};
use rstest::rstest;

const FORBIDDEN_FILENAMES: &[&str] = &[
    "Cargo.lock",
    "bun.lock",
    "pixi.lock",
    "go.sum",
    "biome.json",
    ".Rbuildignore",
    ".DS_Store",
];

fn manifest_languages() -> BTreeMap<String, (String, Vec<String>)> {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets_manifest.toml");
    let raw = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", manifest_path.display()));
    let parsed: toml::Value = toml::from_str(&raw)
        .unwrap_or_else(|err| panic!("invalid TOML in {}: {err}", manifest_path.display()));
    let langs = parsed
        .get("languages")
        .and_then(toml::Value::as_table)
        .expect("assets_manifest.toml must define [languages]");

    let mut out = BTreeMap::new();
    for (lang, entry) in langs {
        let source_dir = entry
            .get("source_dir")
            .and_then(toml::Value::as_str)
            .expect("language entry must define source_dir")
            .to_owned();
        let files = entry
            .get("files")
            .and_then(toml::Value::as_array)
            .expect("language entry must define files")
            .iter()
            .map(|val| {
                val.as_str()
                    .expect("file entry must be a string")
                    .to_owned()
            })
            .collect();
        out.insert(lang.clone(), (source_dir, files));
    }
    out
}

fn resolve_expected_source_root(lang: SdkLanguage, source_dir: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let staged_root = manifest_dir.join("staged").join(lang.as_str());
    if staged_root.is_dir() {
        return staged_root;
    }
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root must exist when staged/ is absent")
        .join(source_dir)
}

#[rstest]
fn test_version_matches_workspace() {
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    let bundles = all_embedded_sdks();
    assert_eq!(bundles.len(), SdkLanguage::ALL.len());
    for bundle in bundles {
        assert_eq!(bundle.version, VERSION);
        assert_eq!(sdk_version(bundle.language), VERSION);
        assert!(!bundle.files.is_empty());
    }
}

#[rstest]
#[case("python", SdkLanguage::Python)]
#[case("py", SdkLanguage::Python)]
#[case("typescript", SdkLanguage::TypeScript)]
#[case("ts", SdkLanguage::TypeScript)]
#[case("cpp", SdkLanguage::Cpp)]
#[case("c", SdkLanguage::C)]
#[case("csharp", SdkLanguage::CSharp)]
#[case("go", SdkLanguage::Go)]
#[case("java", SdkLanguage::Java)]
#[case("r", SdkLanguage::R)]
fn test_sdk_language_round_trip(#[case] alias: &str, #[case] expected: SdkLanguage) {
    let parsed = SdkLanguage::from_str(alias).expect("alias must parse");
    assert_eq!(parsed, expected);
    assert_eq!(
        SdkLanguage::from_str(expected.as_str()).expect("canonical key must parse"),
        expected
    );
}

#[rstest]
fn test_unknown_sdk_language_is_rejected() {
    let err = SdkLanguage::from_str("fortran").expect_err("unknown language must fail");
    assert!(err.to_string().contains("fortran"));
}

#[rstest]
fn test_manifest_and_embedded_files_match_sources_byte_for_byte() {
    let manifest = manifest_languages();
    let expected_langs: BTreeSet<String> = SdkLanguage::ALL
        .iter()
        .map(|lang| lang.as_str().to_owned())
        .collect();
    let actual_langs: BTreeSet<String> = manifest.keys().cloned().collect();
    assert_eq!(actual_langs, expected_langs);

    for lang in SdkLanguage::ALL {
        let (source_dir, expected_files) = manifest
            .get(lang.as_str())
            .expect("every SdkLanguage must exist in assets_manifest.toml");
        let embedded = embedded_files(lang);
        let embedded_paths: Vec<String> = embedded.iter().map(|f| f.path.to_owned()).collect();
        let mut sorted_expected = expected_files.clone();
        sorted_expected.sort();
        assert_eq!(
            embedded_paths, sorted_expected,
            "embedded file set mismatch for {lang}"
        );

        let source_root = resolve_expected_source_root(lang, source_dir);
        for file in &embedded {
            assert!(
                !file.path.contains("tests/")
                    && !file.path.contains("src/test/")
                    && !file.path.ends_with("_test.go")
                    && !file.path.contains(".pytest_cache"),
                "test or cache artifact must never be embedded: {}:{}",
                lang,
                file.path
            );
            for forbidden in FORBIDDEN_FILENAMES {
                assert!(
                    !file.path.ends_with(forbidden),
                    "forbidden lockfile or editor config embedded: {}:{}",
                    lang,
                    file.path
                );
            }

            let looked_up = get_embedded_file(lang, file.path)
                .unwrap_or_else(|| panic!("lookup failed for {}:{}", lang, file.path));
            assert_eq!(looked_up, *file);
            assert!(
                file.as_utf8().is_ok(),
                "published SDK source {}:{} must be valid UTF-8",
                lang,
                file.path
            );

            let disk_path = source_root.join(file.path);
            let disk_meta = fs::symlink_metadata(&disk_path)
                .unwrap_or_else(|err| panic!("missing source file {}: {err}", disk_path.display()));
            assert!(
                !disk_meta.file_type().is_symlink(),
                "source file must not be a symlink: {}",
                disk_path.display()
            );
            let disk_bytes = fs::read(&disk_path).unwrap_or_else(|err| {
                panic!("unreadable source file {}: {err}", disk_path.display())
            });
            assert_eq!(
                file.contents,
                disk_bytes.as_slice(),
                "embedded file {}:{} drifted from {}",
                lang,
                file.path,
                disk_path.display()
            );
        }
    }
}

#[rstest]
fn test_brotli_solid_round_trip_is_deterministic_across_embedded_sdks() {
    let mut raw_stream = Vec::new();
    for sdk in all_embedded_sdks() {
        for file in sdk.files {
            raw_stream.extend_from_slice(sdk.language.as_str().as_bytes());
            raw_stream.push(b'/');
            raw_stream.extend_from_slice(file.path.as_bytes());
            raw_stream.push(0);
            raw_stream.extend_from_slice(file.contents);
            raw_stream.push(0);
        }
    }
    assert!(!raw_stream.is_empty());

    let compress_once = |payload: &[u8]| -> Vec<u8> {
        let mut out = Vec::new();
        let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
        writer.write_all(payload).expect("compress payload");
        writer.flush().expect("flush brotli stream");
        drop(writer);
        out
    };

    let compressed_a = compress_once(&raw_stream);
    let compressed_b = compress_once(&raw_stream);
    assert_eq!(compressed_a, compressed_b);
    assert!(compressed_a.len() < raw_stream.len());

    let mut decompressed = Vec::new();
    let mut reader = brotli_decompressor::Decompressor::new(compressed_a.as_slice(), 4096);
    reader.read_to_end(&mut decompressed).expect("decode solid");
    assert_eq!(decompressed, raw_stream);
}

