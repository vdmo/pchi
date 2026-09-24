"""
PCHI Receiver CHOP for TouchDesigner

This CHOP receives PCHI messages from the PCHI Conductor and outputs
scene state as TouchDesigner channels.

Installation:
1. Place this file in your TouchDesigner scripts folder
2. Create a Python CHOP and set the "Custom" parameter to this script
3. Configure the PCHI Conductor host and port

Usage:
- The CHOP will output channels for each scene object's position
- Musical context (BPM, beat, kick, snare) are output as channels
- Equilibrium status is output as a channel
"""

import json
import socket
import threading
import time
from typing import Dict, Any, Optional
import TDFunctions as TDF


class PCHIReceiverCHOP:
    """Receive PCHI messages and output to TouchDesigner channels"""
    
    def __init__(self, ownerComp):
        self.ownerComp = ownerComp
        self.running = False
        self.socket = None
        self.thread = None
        self.state: Dict[str, Any] = {
            "objects": {},
            "musical_context": None,
            "equilibrium": 0.0,
            "coherence_gap": 0.0
        }
        
        # Get parameters
        self.host = self.ownerComp.par.Host.eval()
        self.port = self.ownerComp.par.Port.eval()
        self.update_rate = self.ownerComp.par.Updaterate.eval()
        
        # Start receiver
        self.start()
    
    def start(self):
        """Start the PCHI receiver thread"""
        if self.running:
            return
        
        self.running = True
        self.thread = threading.Thread(target=self.receive_loop, daemon=True)
        self.thread.start()
        print(f"PCHI Receiver started on {self.host}:{self.port}")
    
    def stop(self):
        """Stop the PCHI receiver thread"""
        self.running = False
        if self.socket:
            self.socket.close()
        if self.thread:
            self.thread.join(timeout=1.0)
        print("PCHI Receiver stopped")
    
    def receive_loop(self):
        """Main receive loop"""
        while self.running:
            try:
                # Create socket if needed
                if not self.socket:
                    self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    self.socket.bind((self.host, self.port))
                    self.socket.settimeout(1.0)
                
                # Receive data
                data, addr = self.socket.recvfrom(65535)
                message = json.loads(data.decode('utf-8'))
                
                # Update state
                self.update_state(message)
                
            except socket.timeout:
                continue
            except Exception as e:
                print(f"PCHI Receiver error: {e}")
                time.sleep(1.0)
                if self.socket:
                    self.socket.close()
                    self.socket = None
    
    def update_state(self, message: Dict[str, Any]):
        """Update internal state from PCHI message"""
        msg_type = message.get("type", "")
        
        if msg_type == "state_update":
            self.state.update(message.get("state", {}))
        elif msg_type == "pchi_message":
            # Handle individual PCHI message
            payload = message.get("payload", {})
            payload_type = payload.get("payload_type", "")
            
            if payload_type == "scene_update":
                data = payload.get("data", {})
                for obj in data.get("objects", []):
                    obj_id = obj.get("id", "")
                    transform = obj.get("transform", {})
                    position = transform.get("position", [0.0, 0.0, 0.0])
                    self.state["objects"][obj_id] = {
                        "position": position
                    }
            
            elif payload_type == "musical_context":
                data = payload.get("data", {})
                self.state["musical_context"] = {
                    "bpm": data.get("bpm", 0.0),
                    "beat": data.get("beat", 0),
                    "kick": data.get("kick", False),
                    "snare": data.get("snare", False),
                    "section": data.get("section", "")
                }
        
        elif msg_type == "equilibrium_change":
            self.state["equilibrium"] = message.get("residual", 0.0)
    
    def cook(self, output):
        """Called by TouchDesigner to update CHOP channels"""
        # Clear existing channels
        output.clear()
        
        # Add equilibrium channel
        eq_channel = output.appendChan("equilibrium")
        eq_channel[0] = self.state["equilibrium"]
        
        # Add coherence gap channel
        coh_channel = output.appendChan("coherence_gap")
        coh_channel[0] = self.state["coherence_gap"]
        
        # Add musical context channels
        if self.state["musical_context"]:
            ctx = self.state["musical_context"]
            
            bpm_channel = output.appendChan("bpm")
            bpm_channel[0] = ctx["bpm"]
            
            beat_channel = output.appendChan("beat")
            beat_channel[0] = ctx["beat"]
            
            kick_channel = output.appendChan("kick")
            kick_channel[0] = 1.0 if ctx["kick"] else 0.0
            
            snare_channel = output.appendChan("snare")
            snare_channel[0] = 1.0 if ctx["snare"] else 0.0
        
        # Add object position channels
        for obj_id, obj_data in self.state["objects"].items():
            position = obj_data.get("position", [0.0, 0.0, 0.0])
            
            # Create channels for each object
            x_channel = output.appendChan(f"{obj_id}_x")
            x_channel[0] = position[0]
            
            y_channel = output.appendChan(f"{obj_id}_y")
            y_channel[0] = position[1]
            
            z_channel = output.appendChan(f"{obj_id}_z")
            z_channel[0] = position[2]
        
        return output


def onCook(scriptOp):
    """TouchDesigner callback - called every frame"""
    if not hasattr(scriptOp, 'receiver'):
        scriptOp.receiver = PCHIReceiverCHOP(scriptOp)
    
    return scriptOp.receiver.cook(scriptOp)


def onOffToOn(scriptOp):
    """TouchDesigner callback - when CHOP is enabled"""
    if not hasattr(scriptOp, 'receiver'):
        scriptOp.receiver = PCHIReceiverCHOP(scriptOp)
    else:
        scriptOp.receiver.start()


def onOnToOff(scriptOp):
    """TouchDesigner callback - when CHOP is disabled"""
    if hasattr(scriptOp, 'receiver'):
        scriptOp.receiver.stop()


def onDelete(scriptOp):
    """TouchDesigner callback - when CHOP is deleted"""
    if hasattr(scriptOp, 'receiver'):
        scriptOp.receiver.stop()
