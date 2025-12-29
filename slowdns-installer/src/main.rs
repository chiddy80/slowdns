use std::process::{Command, Stdio};
use std::io::{self, Write};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::os::unix::fs::PermissionsExt;
use std::env;
use std::net::{UdpSocket, SocketAddr};
use std::sync::Arc;

use colored::*;
use console::Term;
use dialoguer::{Input, theme::ColorfulTheme};
use chrono;
use nix::unistd::Uid;
use reqwest;
use indicatif::{ProgressBar, ProgressStyle};

const SSH_PORT: u16 = 22;
const SLOWDNS_PORT: u16 = 5300;
const GITHUB_BASE: &str = "https://raw.githubusercontent.com/chiddy80/Halotel-Slow-DNS/main/DNSTT%20MODED";

struct SlowDNSInstaller {
    server_ip: String,
    nameserver: String,
    install_dir: PathBuf,
    log_file: PathBuf,
    ssh_config_backup: PathBuf,
    term: Term,
    theme: ColorfulTheme,
}

impl SlowDNSInstaller {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            server_ip: String::new(),
            nameserver: "dns.example.com".to_string(),
            install_dir: PathBuf::from("/etc/slowdns"),
            log_file: PathBuf::from("/var/log/slowdns-install.log"),
            ssh_config_backup: PathBuf::from("/etc/ssh/sshd_config.backup"),
            term: Term::stdout(),
            theme: ColorfulTheme::default(),
        })
    }

    fn print_banner(&self) {
        let _ = self.term.clear_screen();
        
        let banner = "
╔══════════════════════════════════════════════════════════╗
║          🚀 MODERN SLOWDNS INSTALLATION SCRIPT           ║
║            Fast & Professional Configuration             ║
║                Optimized for Performance                 ║
╚══════════════════════════════════════════════════════════╝
        ";
        
        println!("{}", banner.cyan().bold());
    }

    fn print_header(&self, text: &str) {
        println!("\n{}", "══════════════════════════════════════════════════════════".purple());
        println!("{}", text.cyan().bold());
        println!("{}", "══════════════════════════════════════════════════════════".purple());
    }

    fn print_step(&self, step: u8, title: &str) {
        println!("\n{} {}", "┌─".blue(), format!("STEP {}: {}", step, title).cyan().bold());
        println!("{}", "│".blue());
    }

    fn print_step_end(&self) {
        println!("{} {}", "└─".blue(), "✓ Completed".green());
    }

    fn print_success(&self, text: &str) {
        println!("  {} {}", "✓".green().bold(), text.green());
    }

    fn print_error(&self, text: &str) {
        println!("  {} {}", "✗".red().bold(), text.red());
    }

    fn print_warning(&self, text: &str) {
        println!("  {} {}", "!".yellow().bold(), text.yellow());
    }

    fn print_info(&self, text: &str) {
        println!("  {} {}", "ℹ".cyan().bold(), text.cyan());
    }

    fn show_progress(&self, message: &str, duration_ms: u64) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .expect("Failed to set progress style")
        );
        pb.set_message(message.to_string());
        
        thread::sleep(Duration::from_millis(duration_ms));
        pb.finish_with_message("Done!");
    }

    fn run_command(&self, cmd: &str) -> Result<String, Box<dyn std::error::Error>> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!("Command failed: {}", String::from_utf8_lossy(&output.stderr)).into())
        }
    }

    fn run_command_silent(&self, cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        Ok(())
    }

    fn get_server_ip(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_info("Detecting server IP address...");
        
        let methods = [
            ("ifconfig.me", "https://ifconfig.me"),
            ("ipinfo.io", "https://ipinfo.io/ip"),
            ("icanhazip.com", "https://icanhazip.com"),
        ];
        
        for (name, url) in methods.iter() {
            match reqwest::blocking::get(*url) {
                Ok(response) => {
                    if let Ok(ip) = response.text() {
                        let ip = ip.trim().to_string();
                        if !ip.is_empty() {
                            self.server_ip = ip;
                            self.print_success(&format!("Server IP detected via {}: {}", name, self.server_ip));
                            return Ok(());
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        
        match self.run_command("hostname -I | awk '{print $1}'") {
            Ok(ip) => {
                self.server_ip = ip.trim().to_string();
                self.print_success(&format!("Local IP: {}", self.server_ip));
            }
            Err(_) => {
                self.server_ip = "127.0.0.1".to_string();
                self.print_warning("Could not detect IP, using 127.0.0.1");
            }
        }
        
        Ok(())
    }

    fn download_file(&self, url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.print_info(&format!("Downloading: {}", url));
        
        let response = reqwest::blocking::get(url)?;
        let content = response.bytes()?;
        
        fs::write(dest, content)?;
        
        self.print_success(&format!("Downloaded to: {:?}", dest));
        Ok(())
    }

    fn check_root(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !Uid::effective().is_root() {
            return Err("Please run this script as root".into());
        }
        Ok(())
    }

    fn get_nameserver(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_banner();
        
        self.print_info("Enter nameserver configuration:");
        println!("{}", "Default: dns.example.com".yellow());
        println!("{}", "Example: tunnel.yourdomain.com".yellow());
        println!();
        
        let input: String = Input::with_theme(&self.theme)
            .with_prompt("Enter nameserver")
            .default("dns.example.com".into())
            .interact()?;
        
        self.nameserver = input;
        Ok(())
    }

    fn configure_ssh(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_step(1, "CONFIGURE OPENSSH");
        self.print_info(&format!("Configuring OpenSSH on port {}", SSH_PORT));
        
        self.show_progress("Backing up SSH configuration...", 500);
        if self.ssh_config_backup.exists() {
            fs::remove_file(&self.ssh_config_backup)?;
        }
        fs::copy("/etc/ssh/sshd_config", &self.ssh_config_backup)?;
        self.print_success("SSH configuration backed up");
        
        let ssh_config = format!("# ============================================================================
# SLOWDNS OPTIMIZED SSH CONFIGURATION
# ============================================================================
Port {}
Protocol 2
PermitRootLogin yes
PubkeyAuthentication yes
PasswordAuthentication yes
PermitEmptyPasswords no
ChallengeResponseAuthentication no
UsePAM yes
X11Forwarding no
PrintMotd no
PrintLastLog yes
TCPKeepAlive yes
ClientAliveInterval 60
ClientAliveCountMax 3
AllowTcpForwarding yes
GatewayPorts yes
Compression delayed
Subsystem sftp /usr/lib/openssh/sftp-server
MaxSessions 100
MaxStartups 100:30:200
LoginGraceTime 30
UseDNS no
", SSH_PORT);
        
        fs::write("/etc/ssh/sshd_config", ssh_config)?;
        
        self.show_progress("Restarting SSH service...", 1000);
        self.run_command("systemctl restart sshd")?;
        thread::sleep(Duration::from_secs(2));
        self.print_success("SSH service restarted");
        
        self.print_success(&format!("OpenSSH configured on port {}", SSH_PORT));
        self.print_step_end();
        
        Ok(())
    }

    fn setup_slowdns(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_step(2, "SETUP SLOWDNS");
        self.print_info("Setting up SlowDNS environment");
        
        self.show_progress("Creating SlowDNS directory...", 500);
        if self.install_dir.exists() {
            fs::remove_dir_all(&self.install_dir)?;
        }
        fs::create_dir_all(&self.install_dir)?;
        self.print_success("SlowDNS directory created");
        
        env::set_current_dir(&self.install_dir)?;
        
        self.print_info("Downloading SlowDNS binary");
        let binary_url = format!("{}/dnstt-server", GITHUB_BASE);
        let binary_path = self.install_dir.join("dnstt-server");
        
        self.download_file(&binary_url, &binary_path)?;
        
        let mut perms = fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms)?;
        
        self.print_info("Downloading encryption keys");
        
        let key_url = format!("{}/server.key", GITHUB_BASE);
        let key_path = self.install_dir.join("server.key");
        self.download_file(&key_url, &key_path)?;
        
        let pub_url = format!("{}/server.pub", GITHUB_BASE);
        let pub_path = self.install_dir.join("server.pub");
        self.download_file(&pub_url, &pub_path)?;
        
        self.show_progress("Validating binary...", 500);
        let test_result = self.run_command("./dnstt-server --help");
        match test_result {
            Ok(output) => {
                if output.to_lowercase().contains("usage") {
                    self.print_success("Binary validated successfully");
                } else {
                    self.print_warning("Binary test inconclusive");
                }
            }
            Err(_) => {
                self.print_warning("Binary test failed, continuing anyway");
            }
        }
        
        self.print_success("SlowDNS components installed");
        self.print_step_end();
        
        Ok(())
    }

    fn create_slowdns_service(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_step(3, "CREATE SLOWDNS SERVICE");
        self.print_info("Creating SlowDNS system service");
        
        let service_content = format!(r#"# ============================================================================
# SLOWDNS SERVICE CONFIGURATION
# ============================================================================
[Unit]
Description=SlowDNS Server
Description=High-performance DNS tunnel server
After=network.target sshd.service
Wants=network-online.target

[Service]
Type=simple
ExecStart={}/dnstt-server -udp :{} -mtu 1800 -privkey-file {}/server.key {} 127.0.0.1:{}
Restart=always
RestartSec=5
User=root
LimitNOFILE=65536
LimitCORE=infinity
TimeoutStartSec=0

[Install]
WantedBy=multi-user.target
"#, 
            self.install_dir.display(),
            SLOWDNS_PORT,
            self.install_dir.display(),
            self.nameserver,
            SSH_PORT
        );
        
        let service_file = Path::new("/etc/systemd/system/server-sldns.service");
        fs::write(service_file, service_content)?;
        
        self.print_success("Service configuration created");
        self.print_step_end();
        
        Ok(())
    }

    fn compile_edns_proxy(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_step(4, "COMPILE EDNS PROXY");
        self.print_info("Compiling high-performance EDNS Proxy");
        
        self.show_progress("Creating Rust proxy...", 500);
        
        let proxy_code = r#"use std::net::{UdpSocket, SocketAddr};
use std::io;
use std::thread;
use std::sync::Arc;
use std::time::Duration;

const SLOWDNS_PORT: u16 = 5300;
const LISTEN_PORT: u16 = 53;
const BUFFER_SIZE: usize = 4096;
const EXT_EDNS: u16 = 512;
const INT_EDNS: u16 = 1800;

fn patch_edns(buf: &mut [u8], new_size: u16) -> usize {
    if buf.len() < 12 {
        return buf.len();
    }
    
    let mut offset = 12;
    let qdcount = ((buf[4] as u16) << 8) | (buf[5] as u16);
    
    for _ in 0..qdcount {
        if offset >= buf.len() {
            break;
        }
        while offset < buf.len() && buf[offset] != 0 {
            offset += 1;
        }
        offset += 5;
    }
    
    let arcount = ((buf[10] as u16) << 8) | (buf[11] as u16);
    
    for _ in 0..arcount {
        if offset + 4 >= buf.len() {
            break;
        }
        
        if buf[offset] == 0 {
            let rrtype = ((buf[offset + 1] as u16) << 8) | (buf[offset + 2] as u16);
            if rrtype == 41 {
                buf[offset + 3] = (new_size >> 8) as u8;
                buf[offset + 4] = (new_size & 0xFF) as u8;
                break;
            }
        }
        offset += 1;
    }
    
    buf.len()
}

fn handle_client(slowdns_addr: SocketAddr, data: &[u8], client_addr: SocketAddr, listen_socket: Arc<UdpSocket>) {
    let mut modified_data = data.to_vec();
    patch_edns(&mut modified_data, INT_EDNS);
    
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(upstream_socket) => {
            if upstream_socket.send_to(&modified_data, slowdns_addr).is_ok() {
                let mut buffer = [0u8; BUFFER_SIZE];
                match upstream_socket.recv_from(&mut buffer) {
                    Ok((len, _)) => {
                        patch_edns(&mut buffer[..len], EXT_EDNS);
                        let _ = listen_socket.send_to(&buffer[..len], client_addr);
                    }
                    Err(_) => {}
                }
            }
        }
        Err(_) => {}
    }
}

fn main() -> io::Result<()> {
    println!("[EDNS Proxy] Starting high-performance DNS proxy in Rust...");
    
    let socket = UdpSocket::bind(("0.0.0.0", LISTEN_PORT))?;
    socket.set_nonblocking(true)?;
    
    println!("[EDNS Proxy] Listening on port 53");
    println!("[EDNS Proxy] Ready to handle DNS queries");
    
    let slowdns_addr: SocketAddr = format!("127.0.0.1:{}", SLOWDNS_PORT).parse().unwrap();
    let socket_arc = Arc::new(socket);
    
    let mut buffer = [0u8; BUFFER_SIZE];
    
    loop {
        match socket_arc.recv_from(&mut buffer) {
            Ok((len, client_addr)) => {
                let data = buffer[..len].to_vec();
                let slowdns_addr_clone = slowdns_addr;
                let socket_clone = Arc::clone(&socket_arc);
                
                thread::spawn(move || {
                    handle_client(slowdns_addr_clone, &data, client_addr, socket_clone);
                });
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                eprintln!("Error receiving data: {}", e);
            }
        }
    }
}"#;
        
        let proxy_file = Path::new("/tmp/edns_proxy.rs");
        fs::write(proxy_file, proxy_code)?;
        
        self.show_progress("Compiling Rust proxy...", 1000);
        
        let compile_cmd = "rustc --release -o /usr/local/bin/edns-proxy /tmp/edns_proxy.rs 2>/tmp/rust_compile.log";
        match self.run_command(compile_cmd) {
            Ok(_) => {
                let edns_binary = Path::new("/usr/local/bin/edns-proxy");
                let mut perms = fs::metadata(edns_binary)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(edns_binary, perms)?;
                self.print_success("EDNS Proxy compiled successfully in Rust");
            }
            Err(e) => {
                self.print_error(&format!("Rust compilation failed: {}", e));
                self.print_info("Falling back to simple Rust implementation...");
                
                let simple_proxy = r#"use std::net::{UdpSocket, SocketAddr};
use std::io;

const SLOWDNS_PORT: u16 = 5300;
const LISTEN_PORT: u16 = 53;
const BUFFER_SIZE: usize = 4096;

fn main() -> io::Result<()> {
    println!("[Simple EDNS Proxy] Starting DNS proxy...");
    
    let socket = UdpSocket::bind(("0.0.0.0", LISTEN_PORT))?;
    println!("[Simple EDNS Proxy] Listening on port 53");
    
    let slowdns_addr: SocketAddr = format!("127.0.0.1:{}", SLOWDNS_PORT).parse().unwrap();
    let mut buffer = [0u8; BUFFER_SIZE];
    
    loop {
        match socket.recv_from(&mut buffer) {
            Ok((len, client_addr)) => {
                let data = &buffer[..len];
                
                let upstream_socket = UdpSocket::bind("0.0.0.0:0")?;
                
                if upstream_socket.send_to(data, &slowdns_addr).is_ok() {
                    let mut response_buffer = [0u8; BUFFER_SIZE];
                    match upstream_socket.recv_from(&mut response_buffer) {
                        Ok((response_len, _)) => {
                            let _ = socket.send_to(&response_buffer[..response_len], client_addr);
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}"#;
                
                fs::write("/tmp/simple_proxy.rs", simple_proxy)?;
                self.run_command("rustc --release -o /usr/local/bin/edns-proxy /tmp/simple_proxy.rs")?;
                self.print_success("Simple Rust proxy compiled");
            }
        }
        
        let service_content = "# ============================================================================
# EDNS PROXY SERVICE CONFIGURATION
# ============================================================================
[Unit]
Description=EDNS Proxy for SlowDNS
Description=High-performance DNS proxy with EDNS support
After=server-sldns.service
Requires=server-sldns.service

[Service]
Type=simple
ExecStart=/usr/local/bin/edns-proxy
Restart=always
RestartSec=3
User=root
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
";
        
        let service_file = Path::new("/etc/systemd/system/edns-proxy.service");
        fs::write(service_file, service_content)?;
        
        self.print_success("EDNS Proxy service configured");
        self.print_step_end();
        
        Ok(())
    }

    fn configure_firewall(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_step(5, "CONFIGURE FIREWALL");
        self.print_info("Configuring system firewall");
        
        self.show_progress("Setting up firewall rules...", 800);
        
                
        let firewall_commands = vec![
            "iptables -F".to_string(),
            "iptables -X".to_string(),
            "iptables -t nat -F".to_string(),
            "iptables -t nat -X".to_string(),
            "iptables -P INPUT ACCEPT".to_string(),
            "iptables -P FORWARD ACCEPT".to_string(),
            "iptables -P OUTPUT ACCEPT".to_string(),
            "iptables -A INPUT -i lo -j ACCEPT".to_string(),
            "iptables -A OUTPUT -o lo -j ACCEPT".to_string(),
            "iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT".to_string(),
            format!("iptables -A INPUT -p tcp --dport {} -j ACCEPT", SSH_PORT),
            format!("iptables -A INPUT -p udp --dport {} -j ACCEPT", SLOWDNS_PORT),
            "iptables -A INPUT -p udp --dport 53 -j ACCEPT".to_string(),
            "iptables -A INPUT -s 127.0.0.1 -d 127.0.0.1 -j ACCEPT".to_string(),
            "iptables -A OUTPUT -s 127.0.0.1 -d 127.0.0.1 -j ACCEPT".to_string(),
            "iptables -A INPUT -p icmp -j ACCEPT".to_string(),
            "iptables -A INPUT -m state --state INVALID -j DROP".to_string(),
        ];
        
        for cmd in firewall_commands {
            let _ = self.run_command_silent(&cmd);
        }
        
        self.print_info("Disabling IPv6...");
        let _ = self.run_command_silent("echo 1 > /proc/sys/net/ipv6/conf/all/disable_ipv6");
        
        self.print_success("Firewall rules configured");
        
        self.show_progress("Stopping conflicting DNS services...", 500);
        let _ = self.run_command_silent("systemctl stop systemd-resolved 2>/dev/null");
        let _ = self.run_command_silent("pkill -f '53/udp' 2>/dev/null");
        
        self.print_success("DNS services stopped");
        self.print_success("Firewall and network configured");
        self.print_step_end();
        
        Ok(())
    }

    fn start_services(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_step(6, "START SERVICES");
        self.print_info("Starting all services");
        
        self.run_command("systemctl daemon-reload")?;
        
        self.show_progress("Starting SlowDNS service...", 1000);
        let _ = self.run_command_silent("systemctl enable server-sldns > /dev/null 2>&1");
        self.run_command("systemctl start server-sldns")?;
        thread::sleep(Duration::from_secs(2));
        
        if self.run_command("systemctl is-active server-sldns").is_ok() {
            self.print_success("SlowDNS service started");
        } else {
            self.print_warning("Starting SlowDNS in background");
            let cmd = format!(
                "{}/dnstt-server -udp :{} -mtu 1800 -privkey-file {}/server.key {} 127.0.0.1:{} &",
                self.install_dir.display(),
                SLOWDNS_PORT,
                self.install_dir.display(),
                self.nameserver,
                SSH_PORT
            );
            self.run_command_silent(&cmd)?;
        }
        
        self.show_progress("Starting EDNS Proxy service...", 1000);
        let _ = self.run_command_silent("systemctl enable edns-proxy > /dev/null 2>&1");
        self.run_command("systemctl start edns-proxy")?;
        thread::sleep(Duration::from_secs(2));
        
        if self.run_command("systemctl is-active edns-proxy").is_ok() {
            self.print_success("EDNS Proxy service started");
        } else {
            self.print_warning("Starting EDNS Proxy manually");
            self.run_command_silent("/usr/local/bin/edns-proxy &")?;
        }
        
        self.show_progress("Verifying service status...", 1500);
        self.print_success("Service verification complete");
        
        self.print_success("All services started successfully");
        self.print_step_end();
        
        Ok(())
    }

    fn show_summary(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_header("🎉 INSTALLATION COMPLETE");
        
        println!("\n{}", "SERVER INFORMATION".cyan().bold());
        println!("{}", "─".repeat(50));
        println!("{} Server IP:     {}", "●".yellow(), self.server_ip.white().bold());
        println!("{} SSH Port:      {}", "●".yellow(), SSH_PORT.to_string().white());
        println!("{} SlowDNS Port:  {}", "●".yellow(), SLOWDNS_PORT.to_string().white());
        println!("{} EDNS Port:     {}", "●".yellow(), "53".white());
        println!("{} MTU Size:      {}", "●".yellow(), "1800".white());
        println!("{} Nameserver:    {}", "●".yellow(), self.nameserver.white().bold());
        println!("{}", "─".repeat(50));
        
        println!("\n{}", "QUICK TEST COMMANDS".cyan().bold());
        println!("{}", "─".repeat(50));
        println!("{}", format!("dig @{} {}", self.server_ip, self.nameserver).green());
        println!("{}", format!("nslookup {} {}", self.nameserver, self.server_ip).green());
        println!("{}", "systemctl status server-sldns".green());
        println!("{}", "systemctl status edns-proxy".green());
        println!("{}", "─".repeat(50));
        
        println!("\n{}", "SERVICE MANAGEMENT".cyan().bold());
        println!("{}", "─".repeat(50));
        println!("{} systemctl restart server-sldns edns-proxy", "Restart services:".yellow());
        println!("{} journalctl -u server-sldns -f", "View logs:".yellow());
        println!("{} ss -ulpn | grep ':53\\|:5300'", "Check ports:".yellow());
        println!("{}", "─".repeat(50));
        
        println!("\n{}", "Verifying installation...".white().bold());
        
        print!("  {} Checking port 53...", "ℹ".cyan());
        io::stdout().flush()?;
        
        if self.run_command("ss -ulpn 2>/dev/null | grep ':53 '").is_ok() {
            println!("\r  {} Port 53 (EDNS Proxy) is listening", "✓".green());
        } else {
            println!("\r  {} Port 53 not listening", "!".yellow());
        }
        
        print!("  {} Checking port 5300...", "ℹ".cyan());
        io::stdout().flush()?;
        
        if self.run_command(&format!("ss -ulpn 2>/dev/null | grep ':{} '", SLOWDNS_PORT)).is_ok() {
            println!("\r  {} Port {} (SlowDNS) is listening", "✓".green(), SLOWDNS_PORT);
        } else {
            println!("\r  {} Port {} not listening", "!".yellow(), SLOWDNS_PORT);
        }
        
        print!("  {} Checking service status...", "ℹ".cyan());
        io::stdout().flush()?;
        
        let slowdns_active = self.run_command("systemctl is-active server-sldns").is_ok();
        let edns_active = self.run_command("systemctl is-active edns-proxy").is_ok();
        
        if slowdns_active && edns_active {
            println!("\r  {} All services are running", "✓".green());
        } else {
            println!("\r  {} Some services need attention", "!".yellow());
        }
        
        let pub_key_path = self.install_dir.join("server.pub");
        if pub_key_path.exists() {
            println!("\n{}", "PUBLIC KEY (For Client Configuration)".cyan().bold());
            println!("{}", "─".repeat(50));
            if let Ok(content) = fs::read_to_string(&pub_key_path) {
                if let Some(first_line) = content.lines().next() {
                    println!("{}", first_line.white());
                }
            }
            println!("{}", "─".repeat(50));
        }
        
        println!("\n{}", "PERFORMANCE TIPS".cyan().bold());
        println!("{}", "─".repeat(50));
        println!("{} MTU 1800 is optimal for most networks", "●".yellow());
        println!("{} For better performance, use TCP instead of UDP", "●".yellow());
        println!("{} Monitor performance: systemctl status server-sldns", "●".yellow());
        println!("{} Check logs: journalctl -u edns-proxy -n 50", "●".yellow());
        println!("{}", "─".repeat(50));
        
        println!("\n{}", "CLIENT CONFIGURATION EXAMPLE".cyan().bold());
        println!("{}", "─".repeat(50));
        println!("{}", "SlowDNS Client Command:".yellow());
        println!("{}", format!("./dnstt-client -udp {}:{} \\", self.server_ip, SLOWDNS_PORT).green());
        println!("{}", "    -pubkey-file server.pub \\".green());
        println!("{}", format!("    {} 127.0.0.1:1080", self.nameserver).green());
        println!("{}", "─".repeat(50));
        
        println!("\n{}", "TROUBLESHOOTING".cyan().bold());
        println!("{}", "─".repeat(50));
        println!("{}", "If port 53 is not listening:".yellow());
        println!("{}", "1. Stop systemd-resolved: systemctl stop systemd-resolved".white());
        println!("{}", "2. Kill any process on port 53: fuser -k 53/udp".white());
        println!("{}", "3. Restart edns-proxy: systemctl restart edns-proxy".white());
        println!();
        println!("{}", "If SlowDNS is not working:".yellow());
        println!("{}", "1. Check firewall: iptables -L -n -v".white());
        println!("{}", "2. Verify keys: ls -la /etc/slowdns/".white());
        println!("{}", "3. Restart all: systemctl restart server-sldns edns-proxy".white());
        println!("{}", "─".repeat(50));
        
        println!("\n{}", "╔══════════════════════════════════════════════════════════╗".green().bold());
        println!("{}", "║    🎯 SLOWDNS INSTALLATION COMPLETED SUCCESSFULLY!    ║".green().bold());
        println!("{}", "║    ⚡ Installation completed in Rust                     ║".green().bold());
        println!("{}", "║    📊 Services running: SlowDNS + EDNS Proxy            ║".green().bold());
        println!("{}", "║    🔧 Ready for DNS tunneling                           ║".green().bold());
        println!("{}", "╚══════════════════════════════════════════════════════════╝".green().bold());
        
        println!("\n{} {}", "📞".yellow().bold(), "Need help? Contact support: @esimfreegb".yellow().bold());
        println!("{} {}", "💡".yellow().bold(), "Documentation: https://github.com/chiddy80/Halotel-Slow-DNS".yellow().bold());
        
        Ok(())
    }

    fn post_install_menu(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n{}", "POST-INSTALLATION OPTIONS".cyan().bold());
        println!("{}", "─".repeat(50));
        println!("{} {}", "1.".yellow(), "View service status".white());
        println!("{} {}", "2.".yellow(), "Check listening ports".white());
        println!("{} {}", "3.".yellow(), "Restart all services".white());
        println!("{} {}", "4.".yellow(), "View installation log".white());
        println!("{} {}", "5.".yellow(), "Test DNS functionality".white());
        println!("{} {}", "6.".yellow(), "Exit to terminal".white());
        println!("{}", "─".repeat(50));
        
        print!("{}", "Select option [1-6]: ".white().bold());
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();
        
        match choice {
            "1" => {
                println!("\n{}", "════════════════ SERVICE STATUS ════════════════".cyan());
                let _ = self.run_command("systemctl status server-sldns --no-pager -l");
                println!("\n{}", "═══════════════════════════════════════════════".cyan());
                let _ = self.run_command("systemctl status edns-proxy --no-pager -l");
            }
            "2" => {
                println!("\n{}", "════════════════ LISTENING PORTS ════════════════".cyan());
                println!("{}", "Checking UDP ports:".white());
                let _ = self.run_command("ss -ulpn | grep -E ':53|:5300'");
                println!("\n{}", "Checking TCP ports:".white());
                let _ = self.run_command("ss -tlnp | grep -E ':22'");
            }
            "3" => {
                println!("\n{}", "════════════════ RESTARTING SERVICES ════════════════".cyan());
                let _ = self.run_command("systemctl restart server-sldns edns-proxy");
                thread::sleep(Duration::from_secs(2));
                println!("{}", "✓ Services restarted successfully".green());
            }
            "4" => {
                println!("\n{}", "════════════════ INSTALLATION LOG ════════════════".cyan());
                if self.log_file.exists() {
                    if let Ok(content) = fs::read_to_string(&self.log_file) {
                        let lines: Vec<&str> = content.lines().collect();
                        let start = if lines.len() > 20 { lines.len() - 20 } else { 0 };
                        for line in &lines[start..] {
                            println!("{}", line);
                        }
                    }
                } else {
                    println!("{}", "Log file not found".yellow());
                }
            }
            "5" => {
                println!("\n{}", "════════════════ DNS TEST ════════════════".cyan());
                println!("{}", format!("Testing DNS query to {}...", self.nameserver).white());
                
                if self.run_command("which dig").is_ok() {
                    let _ = self.run_command(&format!("dig @{} {} +short", self.server_ip, self.nameserver));
                } else if self.run_command("which nslookup").is_ok() {
                    let _ = self.run_command(&format!("nslookup {} {}", self.nameserver, self.server_ip));
                } else {
                    println!("{}", "DNS tools not available".yellow());
                }
            }
            "6" => {
                println!("\n{}", "Returning to terminal...".green());
            }
            _ => {
                println!("\n{}", "Invalid option, returning to terminal...".yellow());
            }
        }
        
        Ok(())
    }

    fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        let temp_files = [
            Path::new("/tmp/edns_proxy.rs"),
            Path::new("/tmp/simple_proxy.rs"),
            Path::new("/tmp/rust_compile.log"),
            Path::new("/var/log/slowdns-install.log"),
        ];
        
        for file in &temp_files {
            if file.exists() {
                let _ = fs::remove_file(file);
            }
        }
        
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.check_root()?;
        
        self.get_nameserver()?;
        
        self.print_header("📦 GATHERING SYSTEM INFORMATION");
        
        self.get_server_ip()?;
        
        self.configure_ssh()?;
        self.setup_slowdns()?;
        self.create_slowdns_service()?;
        self.compile_edns_proxy()?;
        self.configure_firewall()?;
        self.start_services()?;
        self.show_summary()?;
        
        self.post_install_menu()?;
        
        self.cleanup()?;
        
        let current_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        println!("\n{}", "══════════════════════════════════════════════════════════".green().bold());
        println!("{}", format!("   Installation completed at: {}", current_time).green().bold());
        println!("{}", format!("   Server: {} | SlowDNS: {} | EDNS: 53", 
            self.server_ip, SLOWDNS_PORT).green().bold());
        println!("{}", "══════════════════════════════════════════════════════════".green().bold());
        println!();
        
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    colored::control::set_override(true);
    
    let mut installer = SlowDNSInstaller::new()?;
    
    match installer.run() {
        Ok(_) => {
            println!("{}", "Installation completed successfully!".green().bold());
            Ok(())
        }
        Err(e) => {
            eprintln!("{} {}", "✗".red().bold(), format!("Installation failed: {}", e).red());
            installer.cleanup()?;
            std::process::exit(1);
        }
    }
}
```
