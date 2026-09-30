//! PCHI Conductor - Main server with web dashboard

use primeswarm_pchi::{PCHIConductor, WebServerState, create_router};
use pchi_schema::{PCHIMessage, MessageType, Payload, ControlParameterData};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create PCHI Conductor
    let mut conductor = PCHIConductor::new(1e-12);
    
    println!("PCHI Conductor started");
    
    // Start transport layer (UDP + WebSocket)
    conductor.start_transport("127.0.0.1:8888").await?;
    println!("Transport layer started on UDP 127.0.0.1:8888");
    println!("WebSocket server started on ws://127.0.0.1:8889");
    
    // Create web server state
    let web_state = WebServerState {
        scene_state: conductor.get_state_arc(),
        governance: Some(conductor.get_governance_arc()),
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

    println!("PCHI Conductor running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    println!("Shutting down...");

    // Stop background tasks (dropping the process closes the sockets;
    // stop_transport needs &mut self, which the Arc above gave up)
    web_handle.abort();
    udp_handle.abort();
    ws_handle.abort();

    println!("PCHI Conductor stopped");

    Ok(())
}
