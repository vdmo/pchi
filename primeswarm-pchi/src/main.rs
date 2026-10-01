//! PCHI Conductor - Main server with web dashboard

use primeswarm_pchi::{PCHIConductor, WebServerState, create_router};
use pchi_schema::{PCHIMessage, MessageType, Payload, ControlParameterData};
use clap::Parser;
use std::sync::Arc;

/// PCHI Conductor — central state management with PIR mathematical
/// governance. All flags also read from an env var of the same name
/// (upper-cased), so a container/service deployment can configure this
/// without touching the invocation.
#[derive(Parser)]
#[command(about = "PCHI Conductor - central state management with PIR mathematical governance")]
struct Cli {
    /// Path to a .only-pchi rule file. Omit to use the bundled default
    /// rule set (pchi-schema/examples/kraken-tentacle.only-pchi) — the
    /// same rules every fresh checkout has always run, unchanged.
    #[arg(long, env = "PCHI_RULES_FILE")]
    rules: Option<String>,

    /// Where to periodically snapshot scene state, so a restart resumes
    /// instead of starting blank. Always enabled, same as the governance
    /// log's own default-local-file behavior.
    #[arg(long, env = "PCHI_STATE_FILE", default_value = "pchi_scene_state.json")]
    state_file: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Create PCHI Conductor. Loading rules from a file at startup — rather
    // than only ever running the rule set compiled into the binary — is
    // what makes a single `primeswarm-pchi` build usable across different
    // venues/shows: each one points at its own .only-pchi file instead of
    // needing a recompile to change a safety threshold.
    let mut conductor = match &cli.rules {
        Some(path) => {
            println!("Loading rules from {}", path);
            PCHIConductor::with_rules_file(
                1e-12,
                path,
                "pchi_governance_key.hex",
                "pchi_governance_log.jsonl",
            )
            .unwrap_or_else(|e| {
                eprintln!("Failed to load rules from {}: {}", path, e);
                std::process::exit(1);
            })
        }
        None => PCHIConductor::new(1e-12),
    }
    .with_state_file(&cli.state_file);

    println!("PCHI Conductor started");
    
    // Start transport layer (UDP + WebSocket)
    conductor.start_transport("127.0.0.1:8888").await?;
    println!("Transport layer started on UDP 127.0.0.1:8888");
    println!("WebSocket server started on ws://127.0.0.1:8889");
    
    // Create web server state. PCHI_DASHBOARD_PASSWORD, when set, requires
    // HTTP Basic Auth (username "operator") on the dashboard, its /ws feed,
    // and /governance/export — unset by default, same self-hosted-trusted-
    // machine posture this conductor has always had.
    let dashboard_password = std::env::var("PCHI_DASHBOARD_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty());
    if dashboard_password.is_some() {
        println!("Dashboard auth enabled (HTTP Basic, username: operator)");
    } else {
        println!("Dashboard auth disabled — set PCHI_DASHBOARD_PASSWORD to require credentials");
    }
    let web_state = WebServerState {
        scene_state: conductor.get_state_arc(),
        governance: Some(conductor.get_governance_arc()),
        dashboard_password,
    };
    println!(
        "Governance log verifying_key: {}",
        conductor.get_governance_arc().verifying_key_hex()
    );
    
    // Create web server router
    let app = create_router(web_state);
    
    // Spawn web server in background
    let web_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
        println!("Web dashboard available at http://127.0.0.1:3000");
        axum::serve(listener, app).await.unwrap();
    });
    
    // Create a sample control parameter message
    let message = PCHIMessage::new(
        MessageType::ControlParameter,
        "resolume_bridge".to_string(),
        Payload::ControlParameter(ControlParameterData {
            target_id: "resolume_layer_1".to_string(),
            parameter: "opacity".to_string(),
            value: serde_json::json!(0.85),
        }),
    );
    
    // Process the message
    match conductor.process_message(message).await {
        Ok(_) => println!("Message processed successfully"),
        Err(e) => println!("Error processing message: {}", e),
    }

    // Get current state
    let state = conductor.get_state().await;
    println!("Current equilibrium status: {:?}", state.equilibrium_status);

    // Feed real incoming UDP traffic into process_message. Until this ran,
    // start_transport only opened the socket — nothing a bridge sent to it
    // was ever validated or governed. Needs Arc<PCHIConductor> since the
    // loop runs in its own task alongside the web server.
    let conductor = Arc::new(conductor);
    let udp_handle = tokio::spawn(conductor.clone().run_udp_receive_loop());
    println!("Governing incoming UDP traffic on 127.0.0.1:8888");

    // Accept WebSocket clients on 8889 so broadcast_websocket (governance
    // events) actually reaches someone. Until this ran, the socket was
    // bound but nothing ever accepted a connection.
    let ws_handle = tokio::spawn(conductor.clone().run_ws_accept_loop());
    println!("Accepting governance-event WebSocket clients on ws://127.0.0.1:8889");

    // Periodically snapshot scene state so a restart resumes instead of
    // starting blank. A no-op loop if persistence somehow ended up
    // disabled, so always safe to spawn.
    let snapshot_handle = tokio::spawn(
        conductor.clone().run_state_snapshot_loop(std::time::Duration::from_secs(2)),
    );
    println!("Persisting scene state to {} every 2s", cli.state_file);

    println!("PCHI Conductor running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    println!("Shutting down...");

    // One last snapshot before tearing down, so a planned restart loses
    // nothing regardless of where in the snapshot loop's interval this
    // landed.
    conductor.snapshot_state_now().await;

    // Stop background tasks (dropping the process closes the sockets;
    // stop_transport needs &mut self, which the Arc above gave up)
    web_handle.abort();
    udp_handle.abort();
    ws_handle.abort();
    snapshot_handle.abort();

    println!("PCHI Conductor stopped");

    Ok(())
}
