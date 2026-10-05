//! Builds the rules plugins for `wasm32-unknown-unknown` and places the
//! modules in `OUT_DIR`: the official NATO rules, which the kernel embeds, and
//! an example third-party plugin used by the kernel's tests.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Plugin packages built for the kernel: the bundled official rules, and an
/// example third-party plugin used by the kernel's tests.
const PLUGINS: [(&str, &str); 2] = [
    ("ooaw-nato", "ooaw_nato.wasm"),
    (
        "ooaw-example-night-fighting",
        "ooaw_example_night_fighting.wasm",
    ),
];
const WASM_TARGET: &str = "wasm32-unknown-unknown";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("ooaw-core lives in <workspace>/crates/ooaw-core")
        .to_path_buf();
    for input in [
        "Cargo.toml",
        "Cargo.lock",
        "crates/plugins/nato-official",
        "crates/plugins/examples",
        "crates/ooaw-plugin-api",
        "crates/ooaw-plugin-sdk",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(input).display());
    }

    // A separate target directory: the outer build holds the lock on the
    // workspace's own, and the plugin is always built optimised.
    let target_dir = root.join("target").join("wasm-plugins");
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command
        .current_dir(&root)
        .args(["build", "--release", "--lib", "--target", WASM_TARGET]);
    for (package, _) in PLUGINS {
        command.args(["-p", package]);
    }
    command.arg("--target-dir").arg(&target_dir);
    // Flags and wrappers meant for the outer, native build must not leak in.
    for variable in [
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_PRIMARY_PACKAGE",
    ] {
        command.env_remove(variable);
    }
    let status = command
        .status()
        .expect("failed to run cargo to build the rules plugins");
    if !status.success() {
        panic!(
            "building the rules plugins for {WASM_TARGET} failed; if the target is \
             missing, install it with `rustup target add {WASM_TARGET}`"
        );
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("set by Cargo"));
    for (_, file) in PLUGINS {
        let built = target_dir.join(WASM_TARGET).join("release").join(file);
        std::fs::copy(&built, out_dir.join(file))
            .unwrap_or_else(|error| panic!("cannot copy {}: {error}", built.display()));
    }
}
