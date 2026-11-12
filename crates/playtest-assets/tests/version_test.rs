//! Integration tests for `playtest-assets` (`Spec 0012 §2`, `ADR 0020`).
//!
//! Verifies workspace version synchronization, per-language embedded file lists,
//! absence of test/lockfile/cache/editor artifacts, byte-for-byte parity with
//! canonical monorepo SDK files, deterministic solid `.tar.br` reproduction,
//! runtime dependency boundaries (`brotli-decompressor` only, never `brotli`),
//! and archive error handling.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use playtest_assets::{
    SdkLanguage, VERSION, all_embedded_sdks, embedded_archive_bytes, embedded_files,
    get_embedded_file, sdk_version, verify_archive_bytes,
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

fn normalize_expected_bytes(lang: SdkLanguage, rel_path: &str, raw: Vec<u8>) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(&raw) else {
        return raw;
    };
    let lf = text.replace("\r\n", "\n");
    if lang != SdkLanguage::TypeScript || !rel_path.starts_with("dist/") {
        return lf.into_bytes();
    }
    let mut out = String::with_capacity(lf.len());
    for chunk in lf.split_inclusive('\n') {
        if let Some(rest) = chunk.strip_prefix("// ") {
            out.push_str("// ");
            out.push_str(&rest.replace('\\', "/"));
        } else {
            out.push_str(chunk);
        }
    }
    out.into_bytes()
}

fn compress_tar_brotli(tar_bytes: &[u8]) -> Vec<u8> {
    let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
    writer.write_all(tar_bytes).expect("compress tar");
    writer.flush().expect("flush brotli");
    writer.into_inner()
}

#[rstest]
fn test_version_matches_workspace() {
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    let bundles = all_embedded_sdks();
    assert_eq!(bundles.len(), SdkLanguage::ALL.len());
    for bundle in bundles {
        assert_eq!(bundle.version, VERSION);
        assert_eq!(sdk_version(bundle.language), VERSION);
        assert_eq!(bundle.language.source_dir(), bundle.language.as_str());
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
#[case("cs", SdkLanguage::CSharp)]
#[case("c#", SdkLanguage::CSharp)]
#[case("go", SdkLanguage::Go)]
#[case("golang", SdkLanguage::Go)]
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
    assert!(get_embedded_file(SdkLanguage::Python, "missing_file.py").is_none());

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
            let slash_prefixed = format!("/{}", file.path);
            assert_eq!(get_embedded_file(lang, &slash_prefixed), Some(*file));
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
            let normalized_disk = normalize_expected_bytes(lang, file.path, disk_bytes);
            assert_eq!(
                file.contents,
                normalized_disk.as_slice(),
                "embedded file {}:{} drifted from {}",
                lang,
                file.path,
                disk_path.display()
            );
        }
    }
}

#[rstest]
fn test_solid_brotli_archive_is_deterministic_and_runtime_excludes_encoder() {
    let mut sdks_lex = all_embedded_sdks();
    sdks_lex.sort_by(|lhs, rhs| lhs.language.as_str().cmp(rhs.language.as_str()));

    let mut total_files = 0usize;
    let mut builder = tar::Builder::new(Vec::new());
    for sdk in &sdks_lex {
        for file in &sdk.files {
            total_files += 1;
            let archive_path = format!("{}/{}", sdk.language.as_str(), file.path);
            let mut header = tar::Header::new_ustar();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(file.contents.len() as u64);
            header.set_mode(0o644);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_username("").expect("username");
            header.set_groupname("").expect("groupname");
            header.set_device_major(0).expect("dev major");
            header.set_device_minor(0).expect("dev minor");
            builder
                .append_data(&mut header, &archive_path, file.contents)
                .expect("append tar entry");
        }
    }
    let raw_tar = builder.into_inner().expect("finish tar");
    let repacked = compress_tar_brotli(&raw_tar);

    assert_eq!(
        embedded_archive_bytes(),
        repacked.as_slice(),
        "compile-time embedded_sdks.tar.br must match deterministic repack byte-for-byte"
    );
    assert!(
        embedded_archive_bytes().len() < raw_tar.len(),
        "solid .tar.br ({} B) must be smaller than raw tar ({} B)",
        embedded_archive_bytes().len(),
        raw_tar.len()
    );
    assert_eq!(
        verify_archive_bytes(embedded_archive_bytes()),
        Ok(total_files)
    );

    let cargo_toml_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let cargo_raw = fs::read_to_string(&cargo_toml_path).expect("read Cargo.toml");
    let parsed: toml::Value = toml::from_str(&cargo_raw).expect("parse Cargo.toml");
    let deps = parsed
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .expect("[dependencies] table");
    assert!(deps.contains_key("brotli-decompressor"));
    assert!(
        !deps.contains_key("brotli"),
        "brotli encoder must stay out of [dependencies] so playtest-cli never links it"
    );
    assert!(!deps.contains_key("include_dir"));

    let build_deps = parsed
        .get("build-dependencies")
        .and_then(toml::Value::as_table)
        .expect("[build-dependencies] table");
    assert!(build_deps.contains_key("brotli"));
}

#[rstest]
fn test_verify_archive_bytes_rejects_corrupt_and_malformed_archives() {
    let bad_br = verify_archive_bytes(b"not-a-valid-brotli-stream")
        .expect_err("corrupt brotli bytes must fail");
    assert!(bad_br.to_string().contains("brotli stream error"));

    let build_single_entry_tar = |path: &str, entry_type: tar::EntryType, body: &[u8]| -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(entry_type);
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();
        builder
            .append_data(&mut header, path, body)
            .expect("append test entry");
        let tar_bytes = builder.into_inner().expect("finish test tar");
        compress_tar_brotli(&tar_bytes)
    };

    let no_prefix = build_single_entry_tar("orphan.txt", tar::EntryType::Regular, b"hi");
    let err_prefix = verify_archive_bytes(&no_prefix).expect_err("missing prefix must fail");
    assert!(err_prefix.to_string().contains("missing `<lang>/` prefix"));

    let bad_lang = build_single_entry_tar("fortran/main.f90", tar::EntryType::Regular, b"end");
    let err_lang = verify_archive_bytes(&bad_lang).expect_err("unknown language must fail");
    assert!(err_lang.to_string().contains("fortran"));

    let dir_only = build_single_entry_tar("python/", tar::EntryType::Directory, b"");
    assert_eq!(verify_archive_bytes(&dir_only), Ok(0));
}
