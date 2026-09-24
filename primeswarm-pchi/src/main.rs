//! PCHI Conductor - Main server with web dashboard

use primeswarm_pchi::{PCHIConductor, WebServerState, create_router};
use pchi_schema::{PCHIMessage, MessageType, Payload, ControlParameterData};
use tracing_subscriber;
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
    
    // Create web server state (only needs scene state arc)
    let web_state = WebServerState {
        scene_state: conductor.get_state_arc(),
    };
    
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
    
    println!("PCHI Conductor running. Press Ctrl+C to stop.");
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    println!("Shutting down...");
    
    // Stop background tasks
    web_handle.abort();
    
    // Stop transport
    conductor.stop_transport().await?;
    
    println!("PCHI Conductor stopped");
    
    Ok(())
}
