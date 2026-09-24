#!/usr/bin/env python3
"""
Resolume PCHI Bridge
Connects Resolume Arena to PCHI Conductor via OSC

This bridge listens to OSC messages from Resolume, converts them to PCHI messages
with PIR invariants, and sends them to the PCHI Conductor.
"""

import argparse
import json
import socket
import time
import threading
from typing import Dict, Any, Optional
from dataclasses import dataclass
from pythonosc import udp_client
from pythonosc.dispatcher import Dispatcher
from pythonosc.osc_server import BlockingOSCUDPServer
import numpy as np


@dataclass
class PCHIMessage:
    """PCHI v2.0 message structure"""
    version: str = "2.0.0"
    message_type: str = "control_parameter"
    source_id: str = "resolume_bridge"
    timestamp: float = 0.0
    pir_invariants: Dict[str, Any] = None
    payload: Dict[str, Any] = None

    def __post_init__(self):
        if self.timestamp == 0.0:
            self.timestamp = time.time() * 1000
        if self.pir_invariants is None:
            self.pir_invariants = {}
        if self.payload is None:
            self.payload = {}


class PIRCalculator:
    """Calculate PIR invariants from Resolume state"""
    
    def __init__(self, equilibrium_threshold: float = 1e-12):
        self.threshold = equilibrium_threshold
        self.state_history: Dict[str, list] = {}
    
    def calculate_equilibrium(self, values: list) -> Dict[str, Any]:
        """Calculate PIR equilibrium using Prouhet-Thue-Morse sequence"""
        if len(values) == 0:
            return {"residual": 0.0, "precision": self.threshold, "sign_sequence": []}
        
        # PTM sign sequence
        signs = [1 if bin(i).count('1') % 2 == 0 else -1 for i in range(len(values))]
        residual = sum(s * v for s, v in zip(signs, values))
        
        return {
            "residual": residual,
            "precision": self.threshold,
            "sign_sequence": signs
        }
    
    def calculate_coherence_gap(self, values: list) -> float:
        """Calculate coherence gap as variance from mean"""
        if len(values) < 2:
            return 0.0
        
        values_array = np.array(values)
        return float(np.var(values_array))
    
    def calculate_curvature_signature(self, values: list) -> str:
        """Calculate curvature signature as second derivative approximation"""
        if len(values) < 3:
            return "0.0, 0.0, 0.0"
        
        # Simple second derivative
        curvature = []
        for i in range(1, len(values) - 1):
            d2 = values[i+1] - 2*values[i] + values[i-1]
            curvature.append(d2)
        
        return ", ".join(f"{c:.6f}" for c in curvature[-3:])


class ResolumePCHIBridge:
    """Bridge between Resolume Arena and PCHI Conductor"""
    
    def __init__(
        self,
        resolume_listen_port: int = 7000,
        pchi_conductor_host: str = "127.0.0.1",
        pchi_conductor_port: int = 8888,
        equilibrium_threshold: float = 1e-12
    ):
        self.resolume_listen_port = resolume_listen_port
        self.pchi_conductor_host = pchi_conductor_host
        self.pchi_conductor_port = pchi_conductor_port
        
        self.pir_calculator = PIRCalculator(equilibrium_threshold)
        self.state: Dict[str, Any] = {}
        self.running = False
        
        # OSC client for sending to PCHI Conductor
        self.osc_client = udp_client.SimpleUDPClient(
            pchi_conductor_host, 
            pchi_conductor_port
        )
        
        print(f"Resolume PCHI Bridge initialized")
        print(f"  Listening for Resolume OSC on port {resolume_listen_port}")
        print(f"  Sending PCHI to {pchi_conductor_host}:{pchi_conductor_port}")
    
    def handle_osc_message(self, address: str, *args):
        """Handle incoming OSC message from Resolume"""
        try:
            # Parse Resolume OSC address
            # Format: /composition/layers/{layer_id}/video/{parameter}
            parts = address.split('/')
            
            if len(parts) < 4:
                return
            
            layer_id = parts[3] if len(parts) > 3 else "unknown"
            parameter = parts[4] if len(parts) > 4 else "unknown"
            value = args[0] if args else 0.0
            
            # Update state
            state_key = f"{layer_id}_{parameter}"
            self.state[state_key] = value
            
            # Create PCHI message
            pchi_message = self.create_pchi_message(
                layer_id=layer_id,
                parameter=parameter,
                value=value
            )
            
            # Send to PCHI Conductor
            self.send_pchi_message(pchi_message)
            
            print(f"[OSC] {address} = {value} -> PCHI sent")
            
        except Exception as e:
            print(f"Error handling OSC message: {e}")
    
    def create_pchi_message(
        self,
        layer_id: str,
        parameter: str,
        value: float
    ) -> PCHIMessage:
        """Create PCHI message from Resolume state"""
        
        # Get recent values for PIR calculation
        values = list(self.state.values())[-4:] if self.state else [value]
        
        # Calculate PIR invariants
        equilibrium = self.pir_calculator.calculate_equilibrium(values)
        coherence_gap = self.pir_calculator.calculate_coherence_gap(values)
        curvature = self.pir_calculator.calculate_curvature_signature(values)
        
        pir_invariants = {
            "equilibrium_check": equilibrium,
            "coherence_gap": coherence_gap,
            "curvature_signature": curvature,
            "residual": equilibrium["residual"]
        }
        
        payload = {
            "payload_type": "control_parameter",
            "data": {
                "target_id": f"resolume_layer_{layer_id}",
                "parameter": parameter,
                "value": value
            }
        }
        
        return PCHIMessage(
            message_type="control_parameter",
            source_id="resolume_bridge",
            pir_invariants=pir_invariants,
            payload=payload
        )
    
    def send_pchi_message(self, message: PCHIMessage):
        """Send PCHI message to conductor via OSC"""
        try:
            # Convert to JSON and send as OSC string
            message_json = json.dumps(message.__dict__)
            self.osc_client.send_message("/pchi/message", message_json)
        except Exception as e:
            print(f"Error sending PCHI message: {e}")
    
    def send_heartbeat(self):
        """Send periodic heartbeat to PCHI Conductor"""
        while self.running:
            try:
                heartbeat = PCHIMessage(
                    message_type="heartbeat",
                    source_id="resolume_bridge"
                )
                self.send_pchi_message(heartbeat)
                time.sleep(5)  # Heartbeat every 5 seconds
            except Exception as e:
                print(f"Error sending heartbeat: {e}")
                time.sleep(1)
    
    def start(self):
        """Start the bridge"""
        print("Starting Resolume PCHI Bridge...")
        self.running = True
        
        # Setup OSC dispatcher
        dispatcher = Dispatcher()
        dispatcher.map("/composition/*/*/*", self.handle_osc_message)
        dispatcher.map("/composition/*/*/*/*", self.handle_osc_message)
        
        # Start heartbeat thread
        heartbeat_thread = threading.Thread(target=self.send_heartbeat, daemon=True)
        heartbeat_thread.start()
        
        # Start OSC server
        try:
            server = BlockingOSCUDPServer(
                ("127.0.0.1", self.resolume_listen_port),
                dispatcher
            )
            print(f"OSC server listening on port {self.resolume_listen_port}")
            server.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down...")
            self.running = False
        except Exception as e:
            print(f"Error starting OSC server: {e}")
            self.running = False


def main():
    parser = argparse.ArgumentParser(
        description="Resolume PCHI Bridge - Connect Resolume Arena to PCHI Conductor"
    )
    parser.add_argument(
        "--resolume-port",
        type=int,
        default=7000,
        help="Port to listen for Resolume OSC messages (default: 7000)"
    )
    parser.add_argument(
        "--pchi-host",
        type=str,
        default="127.0.0.1",
        help="PCHI Conductor host (default: 127.0.0.1)"
    )
    parser.add_argument(
        "--pchi-port",
        type=int,
        default=8888,
        help="PCHI Conductor port (default: 8888)"
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=1e-12,
        help="PIR equilibrium threshold (default: 1e-12)"
    )
    
    args = parser.parse_args()
    
    bridge = ResolumePCHIBridge(
        resolume_listen_port=args.resolume_port,
        pchi_conductor_host=args.pchi_host,
        pchi_conductor_port=args.pchi_port,
        equilibrium_threshold=args.threshold
    )
    
    bridge.start()


if __name__ == "__main__":
    main()
