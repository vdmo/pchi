//! Transport layer for PCHI message communication

use crate::ConductorError;
use tokio::net::UdpSocket;
use tokio_tungstenite::tungstenite::protocol::Message;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use std::collections::HashMap;

/// Transport layer for sending/receiving PCHI messages
pub struct TransportLayer {
    /// UDP socket for receiving messages
    udp_socket: Option<UdpSocket>,
    /// WebSocket server
    ws_server: Option<tokio::net::TcpListener>,
    /// Connected WebSocket clients
    ws_clients: Arc<Mutex<HashMap<String, tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>>>,
    /// Bind address
    bind_addr: String,
    /// WebSocket address
    ws_addr: String,
}

impl TransportLayer {
    /// Create a new transport layer
    pub fn new() -> Self {
        Self {
            udp_socket: None,
            ws_server: None,
            ws_clients: Arc::new(Mutex::new(HashMap::new())),
            bind_addr: "127.0.0.1:0".to_string(),
            ws_addr: "127.0.0.1:8889".to_string(),
        }
    }
    
    /// Start the UDP transport layer
    pub async fn start_udp(&mut self, bind_addr: &str) -> Result<(), ConductorError> {
        self.bind_addr = bind_addr.to_string();
        
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| ConductorError::TransportError(format!("Failed to bind UDP: {}", e)))?;
        
        self.udp_socket = Some(socket);
        info!("UDP transport layer started on {}", bind_addr);
        
        Ok(())
    }
    
    /// Start the WebSocket transport layer
    pub async fn start_websocket(&mut self, bind_addr: &str) -> Result<(), ConductorError> {
        self.ws_addr = bind_addr.to_string();
        
        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|e| ConductorError::TransportError(format!("Failed to bind WebSocket: {}", e)))?;
        
        self.ws_server = Some(listener);
        info!("WebSocket transport layer started on {}", bind_addr);
        
        Ok(())
    }
    
    /// Start both UDP and WebSocket transport layers
    pub async fn start(&mut self, bind_addr: &str) -> Result<(), ConductorError> {
        self.start_udp(bind_addr).await?;
        let ws_addr = bind_addr.replace(":8888", ":8889");
        self.start_websocket(&ws_addr).await
    }
    
    /// Stop the transport layer
    pub async fn stop(&mut self) -> Result<(), ConductorError> {
        if let Some(socket) = self.udp_socket.take() {
            drop(socket);
            info!("UDP transport layer stopped");
        }
        if let Some(listener) = self.ws_server.take() {
            drop(listener);
            info!("WebSocket transport layer stopped");
        }
        self.ws_clients.lock().await.clear();
        Ok(())
    }
    
    /// Send a PCHI message via UDP to a target
    pub async fn send_udp(&self, target: &str, message: &[u8]) -> Result<(), ConductorError> {
        let socket = self.udp_socket.as_ref()
            .ok_or_else(|| ConductorError::TransportError("UDP transport not started".to_string()))?;
        
        socket.send_to(message, target)
            .await
            .map_err(|e| ConductorError::TransportError(format!("Failed to send UDP: {}", e)))?;
        
        Ok(())
    }
    
    /// Send a PCHI message to all WebSocket clients
    pub async fn broadcast_websocket(&self, message: &[u8]) -> Result<(), ConductorError> {
        let mut clients = self.ws_clients.lock().await;
        let mut failed_clients = Vec::new();
        
        for (id, client) in clients.iter_mut() {
            if let Err(e) = client.send(Message::Binary(message.to_vec())).await {
                tracing::warn!("Failed to send to WebSocket client {}: {}", id, e);
                failed_clients.push(id.clone());
            }
        }
        
        // Remove failed clients
        for id in failed_clients {
            clients.remove(&id);
        }
        
        Ok(())
    }
    
    /// Send a PCHI message to a target (UDP)
    pub async fn send(&self, target: &str, message: &[u8]) -> Result<(), ConductorError> {
        self.send_udp(target, message).await
    }
    
    /// Receive a PCHI message via UDP
    pub async fn receive(&self, buf: &mut [u8]) -> Result<(usize, std::net::SocketAddr), ConductorError> {
        let socket = self.udp_socket.as_ref()
            .ok_or_else(|| ConductorError::TransportError("UDP transport not started".to_string()))?;
        
        let (len, addr) = socket.recv_from(buf)
            .await
            .map_err(|e| ConductorError::TransportError(format!("Failed to receive UDP: {}", e)))?;
        
        Ok((len, addr))
    }
    
    /// Accept new WebSocket connections
    pub async fn accept_websocket(&self) -> Result<(String, tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>), ConductorError> {
        let listener = self.ws_server.as_ref()
            .ok_or_else(|| ConductorError::TransportError("WebSocket transport not started".to_string()))?;
        
        let (stream, addr) = listener.accept()
            .await
            .map_err(|e| ConductorError::TransportError(format!("Failed to accept WebSocket: {}", e)))?;
        
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| ConductorError::TransportError(format!("WebSocket handshake failed: {}", e)))?;
        
        let client_id = uuid::Uuid::new_v4().to_string();
        info!("WebSocket client connected: {} from {}", client_id, addr);
        
        Ok((client_id, ws_stream))
    }
    
    /// Add a WebSocket client
    pub async fn add_ws_client(&self, id: String, client: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>) {
        self.ws_clients.lock().await.insert(id, client);
    }
    
    /// Get WebSocket address
    pub fn ws_address(&self) -> &str {
        &self.ws_addr
    }
    
    /// Get number of connected WebSocket clients
    pub async fn ws_client_count(&self) -> usize {
        self.ws_clients.lock().await.len()
    }
}

impl Default for TransportLayer {
    fn default() -> Self {
        Self::new()
    }
}
