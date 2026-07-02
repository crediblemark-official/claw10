/// Helper untuk membandingkan semver string (misal: "0.10.0" > "0.2.0")
pub fn is_newer(remote: &str, local: &str) -> bool {
    let remote_parts: Vec<u32> = remote.split('.').filter_map(|s| s.trim().parse::<u32>().ok()).collect();
    let local_parts: Vec<u32> = local.split('.').filter_map(|s| s.trim().parse::<u32>().ok()).collect();

    for i in 0..std::cmp::max(remote_parts.len(), local_parts.len()) {
        let r = remote_parts.get(i).copied().unwrap_or(0);
        let l = local_parts.get(i).copied().unwrap_or(0);
        if r > l {
            return true;
        } else if r < l {
            return false;
        }
    }
    false
}

/// Memeriksa versi terbaru dari GitHub dan melakukan update otomatis jika tersedia.
pub async fn check_and_perform_update(is_auto: bool) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()?;

    let url = "https://raw.githubusercontent.com/crediblemark-official/claw10/master/VERSION";
    
    let res = match client.get(url).header("User-Agent", "claw10-cli").send().await {
        Ok(r) => r,
        Err(e) => {
            if !is_auto {
                return Err(format!("Gagal menghubungi server update: {e}").into());
            }
            return Ok(());
        }
    };

    if !res.status().is_success() {
        if !is_auto {
            return Err(format!("Server update mengembalikan status error: {}", res.status()).into());
        }
        return Ok(());
    }

    let remote_version = res.text().await?.trim().to_string();
    let local_version = env!("CARGO_PKG_VERSION");

    if is_newer(&remote_version, local_version) {
        if !is_auto {
            println!("\n📢 Mendeteksi versi baru Claw10 OS: v{remote_version} (Versi lokal Anda: v{local_version})");
            println!("Menjalankan pembaruan otomatis...");
        }

        let install_script_url = "https://raw.githubusercontent.com/crediblemark-official/claw10/master/install.sh";
        let script_res = client.get(install_script_url).header("User-Agent", "claw10-cli").send().await?;
        
        if !script_res.status().is_success() {
            if !is_auto {
                return Err(format!("Gagal mengunduh installer script: {}", script_res.status()).into());
            }
            return Ok(());
        }

        let script_content = script_res.text().await?;
        let tmp_script_path = std::env::temp_dir().join("claw10_install.sh");
        std::fs::write(&tmp_script_path, script_content)?;

        // Set permission execute
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp_script_path, std::fs::Permissions::from_mode(0o755))?;
        }

        if !is_auto {
            println!("Menjalankan skrip instalasi untuk memperbarui biner...");
        }

        let mut cmd = std::process::Command::new("sh");
        cmd.arg(tmp_script_path.to_str().unwrap());
        if is_auto {
            cmd.stdout(std::process::Stdio::null())
               .stderr(std::process::Stdio::null());
        }

        let status = cmd.status();

        match status {
            Ok(s) if s.success() => {
                if !is_auto {
                    println!("✓ Claw10 OS berhasil diperbarui ke v{remote_version}!");
                }
            }
            _ => {
                if !is_auto {
                    eprintln!("✗ Gagal menjalankan pembaruan otomatis.");
                }
            }
        }
    } else if !is_auto {
        println!("✓ Claw10 OS Anda sudah menggunakan versi terbaru (v{local_version}).");
    }

    Ok(())
}

/// Hanya melakukan pengecekan versi tanpa melakukan proses update/install
pub async fn check_version_only() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()?;

    let url = "https://raw.githubusercontent.com/crediblemark-official/claw10/master/VERSION";
    
    let res = client.get(url)
        .header("User-Agent", "claw10-cli")
        .send()
        .await
        .map_err(|e| format!("Gagal menghubungi server update: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("Server update mengembalikan status error: {}", res.status()).into());
    }

    let remote_version = res.text().await?.trim().to_string();
    let local_version = env!("CARGO_PKG_VERSION");

    if is_newer(&remote_version, local_version) {
        println!("📢 Versi baru Claw10 OS telah tersedia: v{remote_version}");
        println!("Versi lokal Anda saat ini: v{local_version}");
        println!("Jalankan 'claw10 update' untuk memperbarui secara otomatis.");
    } else {
        println!("✓ Claw10 OS Anda sudah menggunakan versi terbaru (v{local_version}).");
    }

    Ok(())
}
