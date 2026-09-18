# Compilar RIVAREN para Android (Xiaomi 14C / Mali-G52)

Requisitos:
```bash
rustup target add aarch64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/27.0.12077973
```

Compilar la librería nativa:
```bash
cargo xtask apk           # genera target/android/arm64-v8a/librivaren.so
```

Empaquetar el APK (dos opciones):
1. `cargo apk build` (cargo-apk) con `[package.metadata.android]` en `crates/app/Cargo.toml`.
2. Gradle + `libmain.so` renombrando `librivaren.so` y un `MainActivity` que cargue
   la librería con `System.loadLibrary("rivaren")` (winit expone
   `android_main` cuando se compila con `--features android`).

Ajustes recomendados en móvil (ya aplicados por defecto):
- Tier `Mobile`: escala de render 0.65, sombras 2 cascadas, GI 2 rayos.
- Perfil `mobile` (opt-level="s", LTO thin) vía `cargo xtask build-all --mobile`.
- Resolución dinámica (`FrameBudget::adapt_resolution`) para mantener 60 FPS.

Notas:
- El renderer usa `Limits::downlevel_defaults()` (Vulkan 1.1 / GLES3), compatible
  con Mali-G52 MC2.
- Los toques se mapean a clics y el HUD usa objetivos táctiles ≥ 48 px.
