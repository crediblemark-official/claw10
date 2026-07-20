use clap::Subcommand;

#[derive(Subcommand, Clone, Debug)]
pub enum ServiceAction {
    /// Install and enable the systemd user service
    Install,
    /// Uninstall and disable the systemd user service
    Uninstall,
    /// Start the systemd user service
    Start,
    /// Stop the systemd user service
    Stop,
    /// Show status of the systemd user service
    Status,
}

pub fn handle_service_command(action: ServiceAction) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/rasyiqi".to_string());
    let systemd_dir = std::path::PathBuf::from(&home)
        .join(".config")
        .join("systemd")
        .join("user");
        
    let service_file_path = systemd_dir.join("claw10.service");

    match action {
        ServiceAction::Install => {
            println!("Menginstal Claw10 systemd user service...");
            if let Err(e) = std::fs::create_dir_all(&systemd_dir) {
                tracing::error!("Gagal membuat direktori systemd user: {e}");
                std::process::exit(1);
            }

            let current_exe = std::env::current_exe()
                .unwrap_or_else(|_| std::path::PathBuf::from("/home/rasyiqi/.local/bin/claw10"));
            
            let service_content = format!(
                "[Unit]\n\
                 Description=Claw10 OS API Server Daemon\n\
                 After=network.target\n\n\
                 [Service]\n\
                 ExecStart={} serve\n\
                 Restart=always\n\
                 RestartSec=5\n\
                 Environment=PATH=/usr/bin:/bin:{}/.local/bin\n\
                 WorkingDirectory={}\n\n\
                 [Install]\n\
                 WantedBy=default.target\n",
                current_exe.display(),
                home,
                home
            );

            if let Err(e) = std::fs::write(&service_file_path, service_content) {
                tracing::error!("Gagal menulis file service unit: {e}");
                std::process::exit(1);
            }

            println!("File service unit berhasil ditulis ke: {}", service_file_path.display());
            
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .stderr(std::process::Stdio::null())
                .status();
                
            let status = std::process::Command::new("systemctl")
                .args(["--user", "enable", "claw10"])
                .stderr(std::process::Stdio::null())
                .status();

            if status.map(|s| s.success()).unwrap_or(false) {
                println!("✓ Service Claw10 berhasil di-enable!");
                println!("Jalankan 'claw10 service start' untuk memulai service.");
            } else {
                eprintln!("✗ Gagal meng-enable service Claw10.");
            }
        }
        ServiceAction::Uninstall => {
            println!("Menghapus Claw10 systemd user service...");
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "--now", "claw10"])
                .stderr(std::process::Stdio::null())
                .status();
                
            if service_file_path.exists() {
                if let Err(e) = std::fs::remove_file(&service_file_path) {
                    tracing::error!("Gagal menghapus file service unit: {e}");
                } else {
                    println!("✓ File service unit berhasil dihapus.");
                }
            }
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .stderr(std::process::Stdio::null())
                .status();
            println!("✓ Service Claw10 berhasil di-uninstall.");
        }
        ServiceAction::Start => {
            println!("Memulai service Claw10...");
            let status = std::process::Command::new("systemctl")
                .args(["--user", "start", "claw10"])
                .stderr(std::process::Stdio::null())
                .status();
                
            if status.map(|s| s.success()).unwrap_or(false) {
                println!("✓ Service Claw10 berhasil dijalankan!");
            } else {
                eprintln!("✗ Gagal menjalankan service Claw10.");
            }
        }
        ServiceAction::Stop => {
            println!("Menghentikan service Claw10...");
            let status = std::process::Command::new("systemctl")
                .args(["--user", "stop", "claw10"])
                .stderr(std::process::Stdio::null())
                .status();
                
            if status.map(|s| s.success()).unwrap_or(false) {
                println!("✓ Service Claw10 berhasil dihentikan.");
            } else {
                eprintln!("✗ Gagal menghentikan service Claw10.");
            }
        }
        ServiceAction::Status => {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "status", "claw10"])
                .status();
        }
    }
}
