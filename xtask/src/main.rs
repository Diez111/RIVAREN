//! `cargo xtask`: compilador unificado RIVAREN.
//! Compila UNA VEZ para los 3 OS en paralelo (todos los hilos CPU),
//! con flags óptimos por target, sccache y reporte unificado.
//!
//! Uso:
//!   cargo xtask build-all [--release|--dist|--mobile] [--targets linux,windows,mac]
//!   cargo xtask check-all
//!   cargo xtask bench
//!   cargo xtask package   # .tar.gz / .zip por plataforma en dist/

use anyhow::{Context, Result};
use rayon::prelude::*;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
struct Target {
    name: &'static str,   // cargo --target
    label: &'static str,  // linux / windows / mac
    extra_rustflags: &'static str,
}

const TARGETS: &[Target] = &[
    Target { name: "x86_64-unknown-linux-gnu", label: "linux", extra_rustflags: "-C target-cpu=x86-64-v2" },
    Target { name: "x86_64-pc-windows-gnu", label: "windows", extra_rustflags: "-C target-cpu=x86-64-v2" },
    Target { name: "x86_64-apple-darwin", label: "mac-intel", extra_rustflags: "-C target-cpu=x86-64-v2" },
    Target { name: "aarch64-apple-darwin", label: "mac-silicon", extra_rustflags: "" },
];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(2).collect(); // skip "xtask" + subcmd
    let sub = std::env::args().nth(1).unwrap_or_else(|| "help".into());
    match sub.as_str() {
        "build-all" => build_all(&args),
        "check-all" => check_all(),
        "bench" => bench(),
        "package" => package(),
        "apk" => apk(),
        "doctor" => doctor(),
        _ => {
            println!("RIVAREN xtask — compilador unificado");
            println!("  cargo xtask build-all [--release|--dist|--mobile]");
            println!("  cargo xtask check-all");
            println!("  cargo xtask bench");
            println!("  cargo xtask package");
            println!("  cargo xtask apk       # APK Android (requiere NDK)");
            println!("  cargo xtask doctor    # comprueba toolchains");
            Ok(())
        }
    }
}

fn profile_from(args: &[String]) -> (&str, Vec<String>) {
    if args.iter().any(|a| a == "--dist") {
        ("dist", vec!["--profile".into(), "dist".into()])
    } else if args.iter().any(|a| a == "--mobile") {
        ("mobile", vec!["--profile".into(), "mobile".into()])
    } else {
        ("release", vec!["--release".into()])
    }
}

fn build_all(args: &[String]) -> Result<()> {
    let (profile, prof_args) = profile_from(args);
    println!("◆ RIVAREN build-all [{profile}] — {} targets en paralelo ({} hilos)",
        TARGETS.len(), rayon::current_num_threads());
    let t0 = Instant::now();
    // sccache si existe (compilación única compartida entre targets).
    if Command::new("sccache").arg("--version").output().is_ok() {
        println!("  sccache: activado");
    } else {
        println!("  sccache: no encontrado (recomendado: cargo install sccache)");
    }
    let results: Vec<(String, Result<()>)> = TARGETS
        .par_iter()
        .map(|t| {
            let mut cmd = Command::new("cargo");
            cmd.arg("build").arg("-p").arg("rivaren-app");
            cmd.args(&prof_args);
            cmd.arg("--target").arg(t.name);
            // RUSTFLAGS unificados + extra por target.
            let base = std::env::var("RUSTFLAGS").unwrap_or_default();
            cmd.env("RUSTFLAGS", format!("{base} {} -C embed-bitcode=yes", t.extra_rustflags));
            let out = cmd.output().with_context(|| format!("spawn cargo {}", t.name));
            let res = match out {
                Ok(o) if o.status.success() => Ok(()),
                Ok(o) => Err(anyhow::anyhow!(
                    "target {} falló:\n{}", t.name,
                    String::from_utf8_lossy(&o.stderr).chars().take(3000).collect::<String>()
                )),
                Err(e) => Err(e),
            };
            (t.label.to_string(), res)
        })
        .collect();
    let mut failed = false;
    for (label, r) in &results {
        match r {
            Ok(()) => println!("  ✓ {label} OK"),
            Err(e) => {
                failed = true;
                println!("  ✗ {label} FALLO: {e:#}");
            }
        }
    }
    println!("  tiempo total: {:.1}s", t0.elapsed().as_secs_f32());
    if failed {
        anyhow::bail!("uno o más targets fallaron");
    }
    println!("◆ Binarios en target/<triple>/{profile}/rivaren*");
    Ok(())
}

fn check_all() -> Result<()> {
    println!("◆ check-all: los 4 targets (rápido, sin codegen)");
    TARGETS.par_iter().try_for_each(|t| {
        let st = Command::new("cargo")
            .arg("check").arg("--workspace").arg("--all-targets")
            .arg("--target").arg(t.name)
            .status()
            .with_context(|| format!("check {}", t.name))?;
        if !st.success() {
            anyhow::bail!("check falló para {}", t.name);
        }
        Ok(())
    })?;
    println!("  ✓ todos los targets pasan check");
    Ok(())
}

fn bench() -> Result<()> {
    let st = Command::new("cargo")
        .arg("test").arg("--release").arg("-p").arg("rivaren-world")
        .arg("-p").arg("rivaren-meshing").arg("-p").arg("rivaren-compression")
        .status()?;
    if !st.success() {
        anyhow::bail!("bench/tests fallaron");
    }
    Ok(())
}

/// Empaqueta un APK para Android (requiere Android SDK/NDK + cargo-ndk).
fn apk() -> Result<()> {
    let ndk = std::env::var("ANDROID_NDK_HOME").ok();
    if ndk.is_none() {
        anyhow::bail!(
            "ANDROID_NDK_HOME no definido. Instala el NDK y exporta la variable.\n             Pasos:\n             1) rustup target add aarch64-linux-android\n             2) cargo install cargo-ndk\n             3) export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/<version>\n             4) cargo xtask apk"
        );
    }
    let st = Command::new("cargo")
        .arg("ndk")
        .args(["-t", "arm64-v8a", "-o", "target/android", "build"])
        .arg("-p")
        .arg("rivaren-app")
        .arg("--release")
        .status();
    match st {
        Ok(s) if s.success() => {
            println!("✓ lib rivaren.so en target/android/arm64-v8a");
            println!("  Empaqueta con `cargo apk build` o Gradle usando el .so generado.");
            Ok(())
        }
        Ok(_) => anyhow::bail!("cargo ndk falló"),
        Err(e) => anyhow::bail!("cargo ndk no instalado: {e}"),
    }
}

fn doctor() -> Result<()> {
    println!("◆ doctor: toolchains y targets");
    let out = Command::new("rustup").args(["target", "list", "--installed"]).output()?;
    println!("targets instalados:\n{}", String::from_utf8_lossy(&out.stdout));
    for (name, var) in [
        ("Android NDK", "ANDROID_NDK_HOME"),
        ("macOS SDK (cross)", "SDKROOT"),
    ] {
        println!(
            "{name}: {}",
            std::env::var(var).unwrap_or_else(|_| "(no definido)".into())
        );
    }
    Ok(())
}

fn package() -> Result<()> {
    println!("◆ package: comprimiendo binarios dist/ (ver scripts/package.sh)");
    let st = Command::new("sh").arg("scripts/package.sh").status()?;
    if !st.success() {
        anyhow::bail!("package.sh falló");
    }
    Ok(())
}
