use crate::service::{handle_service_command, ServiceAction};

pub async fn run_setup_wizard(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    handle_service_command(ServiceAction::Stop);

    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg("pkill -f 'claw10 serve' || fuser -k 3000/tcp")
        .output();
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let default_path = std::path::PathBuf::from(&home).join(".claw10").join("config.toml");
    let local_path = std::path::PathBuf::from("claw10.toml");

    let target_path = if local_path.exists() {
        local_path
    } else if default_path.exists() && !force {
        default_path
    } else {
        default_path
    };

    let mut wizard = claw10_tui::SetupWizard::new(target_path);
    wizard.run()?;
    Ok(())
}

pub async fn perform_uninstall(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !force {
        println!("Claw10 OS Uninstaller");
        println!("========================");
        println!("");
        println!("Perintah ini akan menghapus:");
        println!("  - Daemon Service (systemd)");
        println!("  - Folder Database & Konfigurasi (~/.claw10)");
        println!("  - File Eksekusi Binary (claw10)");
        println!("");
        print!("Apakah Anda yakin ingin menghapus Claw10 dari sistem? [y/N]: ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            if trimmed != "y" && trimmed != "yes" {
                println!("Uninstall dibatalkan.");
                std::process::exit(0);
            }
        } else {
            println!("Gagal membaca input. Uninstall dibatalkan.");
            std::process::exit(1);
        }
    }

    println!("\n[1/4] Menghentikan dan menghapus daemon service...");
    handle_service_command(ServiceAction::Stop);
    handle_service_command(ServiceAction::Uninstall);

    println!("[2/4] Menghapus folder konfigurasi dan database (~/.claw10)...");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/rasyiqi".to_string());
    let config_dir = std::path::PathBuf::from(&home).join(".claw10");
    if config_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&config_dir) {
            tracing::warn!("Gagal menghapus folder config: {e}");
        } else {
            println!("✓ Folder ~/.claw10 berhasil dihapus.");
        }
    } else {
        println!("✓ Folder config tidak ditemukan.");
    }

    println!("[3/4] Membersihkan entri PATH dari file konfigurasi shell...");
    let shell_name = std::env::var("SHELL")
        .map(|s| {
            std::path::Path::new(&s)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| "bash".to_string())
        })
        .unwrap_or_else(|_| "bash".to_string());
    
    let rc_files = match shell_name.as_str() {
        "bash" => vec![std::path::PathBuf::from(&home).join(".bashrc")],
        "zsh" => vec![std::path::PathBuf::from(&home).join(".zshrc")],
        "fish" => vec![std::path::PathBuf::from(&home).join(".config/fish/config.fish")],
        _ => vec![
            std::path::PathBuf::from(&home).join(".bashrc"),
            std::path::PathBuf::from(&home).join(".zshrc"),
        ],
    };

    let install_dir = std::path::PathBuf::from(&home).join(".local/bin");
    let cargo_dir = std::path::PathBuf::from(&home).join(".cargo/bin");

    for rc in rc_files {
        if rc.exists() {
            if let Ok(content) = std::fs::read_to_string(&rc) {
                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                let original_len = lines.len();
                
                // Hapus baris PATH claw10
                lines.retain(|line| {
                    !line.contains("export PATH=") || (!line.contains(".local/bin") && !line.contains(".cargo/bin") && !line.contains("claw10"))
                });

                if lines.len() != original_len {
                    if let Err(e) = std::fs::write(&rc, lines.join("\n") + "\n") {
                        tracing::warn!("Gagal membersihkan PATH di {}: {e}", rc.display());
                    } else {
                        println!("✓ Entri PATH dibersihkan dari {}", rc.display());
                    }
                }
            }
        }
    }

    println!("[4/4] Menghapus file binary claw10...");
    let exe_paths = vec![
        install_dir.join("claw10"),
        cargo_dir.join("claw10"),
    ];

    for path in exe_paths {
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            println!("✓ File binary {} berhasil dihapus.", path.display());
        }
    }

    // Hapus binary saat ini yang sedang dieksekusi jika berbeda dari path di atas
    if let Ok(current_exe) = std::env::current_exe() {
        if current_exe.exists() {
            let _ = std::fs::remove_file(&current_exe);
        }
    }

    // Pembersihan sisa logs dan folder /tmp/claw10
    println!("\n[Tambahan] Membersihkan berkas log dan folder temporary...");
    let tmp_dir = std::path::PathBuf::from("/tmp/claw10");
    if tmp_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        println!("✓ Folder temporary /tmp/claw10 berhasil dibersihkan.");
    }

    let logs_dir = std::path::PathBuf::from(&home).join("logs");
    if logs_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&logs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    if filename.starts_with("claw10") {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        println!("✓ Berkas log claw10 di ~/logs/ berhasil dibersihkan.");
    }

    println!("\n🎉 Claw10 OS berhasil di-uninstall seutuhnya dari sistem Anda.");
    Ok(())
}
