fn main() {
    // On macOS, pre-compile the Swift audio helper so end users
    // don't need Xcode Command Line Tools installed.
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        use std::path::Path;

        let swift_src = Path::new("resources/audio_recorder.swift");
        let out_bin = Path::new("resources/audio_recorder_bin");

        println!("cargo:rerun-if-changed=resources/audio_recorder.swift");

        if swift_src.exists() {
            let status = Command::new("xcrun")
                .args([
                    "swiftc",
                    "-parse-as-library",
                    "-O",
                    "-o",
                    out_bin.to_str().unwrap(),
                    swift_src.to_str().unwrap(),
                    "-framework", "Foundation",
                    "-framework", "AVFoundation",
                    "-framework", "CoreMedia",
                    "-framework", "ScreenCaptureKit",
                ])
                .status()
                .expect("Failed to run xcrun swiftc — ensure Xcode Command Line Tools are installed on the build machine");

            assert!(status.success(), "Swift audio helper compilation failed");

            // Ensure the binary is executable
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(out_bin) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(out_bin, perms);
            }

            println!("cargo:warning=Built audio_recorder_bin from Swift source");
        }
    }

    tauri_build::build()
}
