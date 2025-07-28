use clap::{Arg, ArgMatches, Command};
use doomsday_rs::config::{ClientConfig, ClientTarget};
use doomsday_rs::duration::DurationParser;
use doomsday_rs::types::{AuthRequest, CacheItem};
use reqwest::Client;
use serde_json;
use std::collections::HashMap;
use tabled::{Table, Tabled, settings::{Style, Width}};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Command::new("doomsday")
        .version(doomsday_rs::version::VERSION)
        .about("Doomsday certificate monitoring CLI")
        .subcommand(
            Command::new("target")
                .about("Set target doomsday server")
                .arg(Arg::new("name").required(true).help("Target name"))
                .arg(Arg::new("address").required(true).help("Server address"))
                .arg(Arg::new("skip-verify").long("skip-verify").action(clap::ArgAction::SetTrue).help("Skip TLS verification"))
        )
        .subcommand(
            Command::new("targets")
                .about("List configured targets")
        )
        .subcommand(
            Command::new("auth")
                .about("Authenticate with server")
                .arg(Arg::new("username").short('u').long("username").help("Username"))
                .arg(Arg::new("password").short('p').long("password").help("Password"))
        )
        .subcommand(
            Command::new("list")
                .about("List certificates")
                .arg(Arg::new("beyond").long("beyond").help("Show certificates expiring beyond duration"))
                .arg(Arg::new("within").long("within").help("Show certificates expiring within duration"))
        )
        .subcommand(
            Command::new("dashboard")
                .about("Show certificate dashboard")
        )
        .subcommand(
            Command::new("refresh")
                .about("Refresh certificate cache")
                .arg(Arg::new("backends").long("backends").help("Comma-separated list of backends to refresh"))
        )
        .subcommand(
            Command::new("info")
                .about("Show server information")
        )
        .subcommand(
            Command::new("scheduler")
                .about("Show scheduler information")
        );
    
    let matches = app.get_matches();
    
    match matches.subcommand() {
        Some(("target", sub_matches)) => handle_target(sub_matches).await,
        Some(("targets", _)) => handle_targets().await,
        Some(("auth", sub_matches)) => handle_auth(sub_matches).await,
        Some(("list", sub_matches)) => handle_list(sub_matches).await,
        Some(("dashboard", _)) => handle_dashboard().await,
        Some(("refresh", sub_matches)) => handle_refresh(sub_matches).await,
        Some(("info", _)) => handle_info().await,
        Some(("scheduler", _)) => handle_scheduler().await,
        _ => {
            println!("Use --help for usage information");
            Ok(())
        }
    }
}

async fn handle_target(matches: &ArgMatches) -> anyhow::Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    let address = matches.get_one::<String>("address").unwrap();
    let skip_verify = matches.get_flag("skip-verify");
    
    let mut config = ClientConfig::load()?;
    
    let target = ClientTarget {
        name: name.clone(),
        address: address.clone(),
        skip_verify,
        token: None,
        token_expires: None,
    };
    
    config.targets.insert(name.clone(), target);
    config.current_target = Some(name.clone());
    config.save()?;
    
    println!("✅ Target '{}' configured and set as current", name);
    Ok(())
}

async fn handle_targets() -> anyhow::Result<()> {
    let config = ClientConfig::load()?;
    
    if config.targets.is_empty() {
        println!("No targets configured. Use 'doomsday target' to add one.");
        return Ok(());
    }
    
    #[derive(Tabled)]
    struct TargetRow {
        #[tabled(rename = "Name")]
        name: String,
        #[tabled(rename = "Address")]
        address: String,
        #[tabled(rename = "Current")]
        current: String,
        #[tabled(rename = "Skip Verify")]
        skip_verify: String,
    }
    
    let mut rows = Vec::new();
    for (name, target) in &config.targets {
        let current = if config.current_target.as_ref() == Some(name) {
            "✓".to_string()
        } else {
            "".to_string()
        };
        
        let skip_verify = if target.skip_verify { "✓".to_string() } else { "".to_string() };
        
        rows.push(TargetRow {
            name: name.clone(),
            address: target.address.clone(),
            current,
            skip_verify,
        });
    }
    
    let mut table = Table::new(rows);
    table.with(Style::rounded()).with(Width::wrap(120));
    println!("{}", table);
    Ok(())
}

async fn handle_auth(matches: &ArgMatches) -> anyhow::Result<()> {
    let mut config = ClientConfig::load()?;
    let target = config.current_target()
        .ok_or_else(|| anyhow::anyhow!("No target configured. Use 'doomsday target' first."))?
        .clone();
    
    let username = if let Some(username) = matches.get_one::<String>("username") {
        username.clone()
    } else {
        print!("Username: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };
    
    let password = if let Some(password) = matches.get_one::<String>("password") {
        password.clone()
    } else {
        rpassword::prompt_password("Password: ")?
    };
    
    let client = create_client(target.skip_verify);
    let auth_request = AuthRequest { username, password };
    
    let response = client
        .post(&format!("{}/v1/auth", target.address))
        .json(&auth_request)
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Authentication failed"));
    }
    
    let auth_response: doomsday_rs::types::AuthResponse = response.json().await?;
    
    // Update target with token
    if let Some(target_mut) = config.targets.get_mut(&target.name) {
        target_mut.token = Some(auth_response.token);
        target_mut.token_expires = Some(auth_response.expires_at);
    }
    
    config.save()?;
    
    println!("✅ Authentication successful");
    Ok(())
}

async fn handle_list(matches: &ArgMatches) -> anyhow::Result<()> {
    let config = ClientConfig::load()?;
    let target = config.current_target()
        .ok_or_else(|| anyhow::anyhow!("No target configured"))?;
    
    let client = create_client(target.skip_verify);
    let mut url = format!("{}/v1/cache", target.address);
    
    let mut params = vec![];
    if let Some(beyond) = matches.get_one::<String>("beyond") {
        params.push(format!("beyond={}", beyond));
    }
    if let Some(within) = matches.get_one::<String>("within") {
        params.push(format!("within={}", within));
    }
    
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }
    
    let mut request = client.get(&url);
    if let Some(token) = &target.token {
        request = request.header("X-Doomsday-Token", token);
    }
    
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to fetch certificates: {}", response.status()));
    }
    
    let certificates: Vec<CacheItem> = response.json().await?;
    
    if certificates.is_empty() {
        println!("No certificates found");
        return Ok(());
    }
    
    #[derive(Tabled)]
    struct CertRow {
        #[tabled(rename = "Subject")]
        subject: String,
        #[tabled(rename = "Expires")]
        expires: String,
        #[tabled(rename = "Time Until")]
        time_until: String,
        #[tabled(rename = "Paths")]
        paths: String,
    }
    
    let mut rows = Vec::new();
    for cert in certificates {
        let expires = cert.not_after.format("%Y-%m-%d %H:%M UTC").to_string();
        let time_until = DurationParser::format_human(
            DurationParser::until_expiry(cert.not_after)
        );
        let paths = cert.paths.len().to_string();
        
        rows.push(CertRow {
            subject: cert.subject,
            expires,
            time_until,
            paths,
        });
    }
    
    let mut table = Table::new(rows);
    table.with(Style::rounded()).with(Width::wrap(120));
    println!("{}", table);
    Ok(())
}

async fn handle_dashboard() -> anyhow::Result<()> {
    let config = ClientConfig::load()?;
    let target = config.current_target()
        .ok_or_else(|| anyhow::anyhow!("No target configured"))?;
    
    let client = create_client(target.skip_verify);
    let mut request = client.get(&format!("{}/v1/cache", target.address));
    
    if let Some(token) = &target.token {
        request = request.header("X-Doomsday-Token", token);
    }
    
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to fetch certificates: {}", response.status()));
    }
    
    let certificates: Vec<CacheItem> = response.json().await?;
    
    let now = chrono::Utc::now();
    let mut expired = 0;
    let mut expiring_soon = 0;
    let mut ok = 0;
    
    for cert in &certificates {
        let days_until_expiry = (cert.not_after - now).num_days();
        
        if days_until_expiry < 0 {
            expired += 1;
        } else if days_until_expiry <= 30 {
            expiring_soon += 1;
        } else {
            ok += 1;
        }
    }
    
    println!("🔒 Doomsday Certificate Dashboard");
    println!("═══════════════════════════════════");
    println!();
    println!("⚠️  Expired:        {} certificates", expired);
    println!("⏰ Expiring Soon:   {} certificates (within 30 days)", expiring_soon);
    println!("✅ OK:              {} certificates", ok);
    println!("📊 Total:           {} certificates", certificates.len());
    
    Ok(())
}

async fn handle_refresh(matches: &ArgMatches) -> anyhow::Result<()> {
    let config = ClientConfig::load()?;
    let target = config.current_target()
        .ok_or_else(|| anyhow::anyhow!("No target configured"))?;
    
    let client = create_client(target.skip_verify);
    
    let refresh_request = if let Some(backends_str) = matches.get_one::<String>("backends") {
        let backends: Vec<String> = backends_str.split(',').map(|s| s.trim().to_string()).collect();
        doomsday_rs::types::RefreshRequest { backends: Some(backends) }
    } else {
        doomsday_rs::types::RefreshRequest { backends: None }
    };
    
    let mut request = client
        .post(&format!("{}/v1/cache/refresh", target.address))
        .json(&refresh_request);
    
    if let Some(token) = &target.token {
        request = request.header("X-Doomsday-Token", token);
    }
    
    println!("🔄 Refreshing certificate cache...");
    
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to refresh cache: {}", response.status()));
    }
    
    let stats: doomsday_rs::types::PopulateStats = response.json().await?;
    
    println!("✅ Refresh complete");
    println!("   Certificates: {}", stats.num_certs);
    println!("   Paths:        {}", stats.num_paths);
    println!("   Duration:     {}ms", stats.duration_ms);
    
    Ok(())
}

async fn handle_info() -> anyhow::Result<()> {
    let config = ClientConfig::load()?;
    let target = config.current_target()
        .ok_or_else(|| anyhow::anyhow!("No target configured"))?;
    
    let client = create_client(target.skip_verify);
    let response = client.get(&format!("{}/v1/info", target.address)).send().await?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to get server info: {}", response.status()));
    }
    
    let info: doomsday_rs::types::InfoResponse = response.json().await?;
    
    println!("🔒 Doomsday Server Information");
    println!("════════════════════════════════");
    println!("Version:          {}", info.version);
    println!("Authentication:   {}", if info.auth_required { "Required" } else { "Not Required" });
    println!("Target:           {} ({})", target.name, target.address);
    
    Ok(())
}

async fn handle_scheduler() -> anyhow::Result<()> {
    let config = ClientConfig::load()?;
    let target = config.current_target()
        .ok_or_else(|| anyhow::anyhow!("No target configured"))?;
    
    let client = create_client(target.skip_verify);
    let mut request = client.get(&format!("{}/v1/scheduler", target.address));
    
    if let Some(token) = &target.token {
        request = request.header("X-Doomsday-Token", token);
    }
    
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to get scheduler info: {}", response.status()));
    }
    
    let info: doomsday_rs::types::SchedulerInfo = response.json().await?;
    
    println!("⚙️  Scheduler Information");
    println!("════════════════════════");
    println!("Workers:        {}", info.workers);
    println!("Pending Tasks:  {}", info.pending_tasks);
    println!("Running Tasks:  {}", info.running_tasks);
    
    Ok(())
}

fn create_client(skip_verify: bool) -> Client {
    let mut client_builder = reqwest::Client::builder();
    
    if skip_verify {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    
    client_builder.build().unwrap()
}