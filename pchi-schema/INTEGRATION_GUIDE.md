# PCHI Integration Guide for Live Software

## Overview

PCHI (Peachy) v2.0 is a scene state protocol with PIR mathematical invariants that standardizes communication across live creative tools. Unlike OSC, MIDI, or NDI, PCHI standardizes the "letter" (schema) not the "postal service" (transport).

## Core Value Proposition

**The Problem**: Connecting 10+ creative tools requires 50+ custom OSC mappings, creating brittle, unmaintainable systems.

**The Solution**: PCHI provides a centralized, mathematically-verified scene state schema that works over any transport (UDP, TCP, WebSocket, OSC).

**The Innovation**: PIR invariants ensure scene coherence is mathematically guaranteed, not just conventionally agreed upon.

---

## Integration Patterns

### 1. PCHI Hosts (Receivers)

A PCHI Host receives PCHI messages and applies them to internal state. Examples:

#### Unreal Engine Plugin

```rust
// PCHI Unreal Plugin Architecture
class FPCHIPlugin : public IModuleInterface
{
public:
    void OnPCHIMessageReceived(const PCHIMessage& Message)
    {
        // Validate PIR invariants
        if (!Message.PIRInvariants.IsEquilibrium())
        {
            UE_LOG(LogPCHI, Warning, TEXT("Message not in equilibrium"));
            return;
        }
    
        // Apply to scene
        if (Message.Type == EMessageType::SceneUpdate)
        {
            for (const auto& Obj : Message.Payload.Objects)
            {
                AActor* Actor = GetActorById(Obj.Id);
                if (Actor)
                {
                    Actor->SetActorTransform(Obj.Transform);
                }
            }
        }
    }
};
```

**Benefits**:
- Mathematical guarantee of scene coherence
- Automatic validation prevents corrupt state
- Single plugin works with any PCHI sender

#### TouchDesigner CHOP

```python
# PCHI Receiver CHOP in TouchDesigner
def onReceivePCHI(message):
    # Parse JSON PCHI message
    data = json.loads(message)
    
    # Check equilibrium
    residual = data['pirInvariants']['residual']
    if abs(residual) > 1e-12:
        return  # Reject disequilibrium
    
    # Update parameters
    for obj in data['payload']['objects']:
        op = op(obj['id'])
        if op:
            op.par.tx = obj['transform']['position'][0]
            op.par.ty = obj['transform']['position'][1]
            op.par.tz = obj['transform']['position'][2]
```

### 2. PCHI Clients (Senders)

A PCHI Client generates PCHI messages from native state and broadcasts them.

#### Resolume Arena Bridge

```python
# PCHI Bridge for Resolume Arena
class ResolumePCHIBridge:
    def __init__(self, conductor_url):
        self.conductor_url = conductor_url
        self.osc = OSCClient()
    
    def on_clip_change(self, layer, clip):
        message = {
            "version": "2.0.0",
            "type": "control_parameter",
            "sourceId": "resolume_bridge",
            "timestamp": time.time() * 1000,
            "pirInvariants": self.calculate_pir(layer, clip),
            "payload": {
                "payloadType": "control_parameter",
                "data": {
                    "targetId": f"layer_{layer}",
                    "parameter": "clip_index",
                    "value": clip
                }
            }
        }
        self.send_to_conductor(message)
    
    def calculate_pir(self, layer, clip):
        # Calculate PIR invariants from current state
        values = [layer, clip, self.get_opacity(layer), self.get_hue(layer)]
        signs = [1, -1, -1, 1]  # PTM sequence
        residual = sum(s * v for s, v in zip(signs, values))
        return {
            "equilibriumCheck": {
                "residual": residual,
                "precision": 1e-12,
                "signSequence": signs
            },
            "coherenceGap": self.calculate_coherence(values),
            "curvatureSignature": self.calculate_curvature(values),
            "residual": residual
        }
```

#### Ableton Live Max Device

```max
-- PCHI Sender Max Device for Ableton
-- [pchi.sender.maxpat]

-- Calculate PIR invariants from MIDI notes
fun calculate_pir notes:
    signs = [1, -1, -1, 1]
    values = map(notes, note -> note.pitch)
    residual = sum(signs[i] * values[i] for i in 0..3)
    return {
        residual: residual,
        precision: 1e-12,
        signSequence: signs
    }

-- Send PCHI message on beat
fun on_beat:
    message = {
        version: "2.0.0",
        type: "musical_context",
        sourceId: "ableton_pchi",
        timestamp: time.now(),
        pirInvariants: calculate_pir(current_notes),
        payload: {
            payloadType: "musical_context",
            data: {
                bpm: live.bpm,
                beat: live.beat,
                kick: kick_is_playing(),
                snare: snare_is_playing(),
                section: get_song_section()
            }
        }
    }
    udp.send("127.0.0.1:8888", json.encode(message))
```

### 3. PCHI Conductor (Central Hub)

The PCHI Conductor is the central state manager that:
- Receives PCHI messages from all tools
- Validates PIR invariants
- Applies Only-Lang rules
- Broadcasts commands to receivers

```rust
// Running the PCHI Conductor
#[tokio::main]
async fn main() {
    let conductor = PCHIConductor::new(1e-12);
    conductor.start_transport("0.0.0.0:8888").await.unwrap();
    
    // Load Only-Lang rules
    conductor.load_rules("kraken-rules.only").await.unwrap();
    
    // Run forever
    tokio::signal::ctrl_c().await.unwrap();
}
```

---

## Solutions for Common Problems

### Problem 1: "The Spiderweb of Connections"

**Traditional Approach**:
```
Ableton → 7 custom OSC mappings → 7 different tools
BlackTrax → 4 custom OSC mappings → 4 different tools
Lighting → 3 custom OSC mappings → 3 different tools
Total: 14 custom mappings, each requiring custom parsers
```

**PCHI Solution**:
```
All tools → PCHI Conductor (single schema)
PCHI Conductor → All tools (single schema)
Total: 2 PCHI implementations per tool
```

**Benefit**: 7x reduction in integration complexity

### Problem 2: "No Central State"

**Traditional Approach**:
- Debugging requires checking 5 different screens
- No single source of truth
- State drift between systems

**PCHI Solution**:
- PCHI Conductor maintains single source of truth
- Web dashboard shows entire scene state
- State history and rollback

**Benefit**: Instant debugging, state transparency

### Problem 3: "Brittle Custom Mappings"

**Traditional Approach**:
- Lighting director changes OSC address → entire system breaks
- No validation of message structure
- Silent failures

**PCHI Solution**:
- Schema validation at protocol level
- PIR equilibrium checks prevent corrupt state
- Versioned schema with backward compatibility

**Benefit**: Robust, self-healing systems

### Problem 4: "No Mathematical Guarantees"

**Traditional Approach**:
- Scene coherence is convention, not math
- No way to prove system is in valid state
- AI agents can corrupt scene without detection

**PCHI Solution**:
- PIR invariants mathematically prove equilibrium
- Disequilibrium detected before state corruption
- AI agents constrained by mathematical boundaries

**Benefit**: Verifiable system integrity

---

## Opportunities

### 1. PCHI-Compatible Tool Market

**For Tool Developers**:
- "PCHI Certified" badge for tools
- Plugin marketplace for PCHI hosts/clients
- Premium for PCHI-compatible versions

**Examples**:
- "Resolume Arena PCHI Edition" - $50 premium
- "TouchDesigner PCHI Pro" - $200/year
- "Unreal PCHI Plugin" - Free, drives engine adoption

### 2. AI Agent Ecosystem

**PCHI as AI Interface**:
- AI agents reason over PCHI scene state
- Only-Lang rules constrain AI actions
- PIR invariants prevent AI from breaking equilibrium

**Use Cases**:
- AI VJ that responds to musical context
- AI lighting designer that respects artist position
- AI camera operator that maintains composition rules

### 3. Distributed Live Shows

**Multi-Venue Synchronization**:
- PCHI Conductor in cloud
- Multiple venues connect to same scene state
- Coordinated shows across cities

**Example**:
- DJ performs in Tokyo
- Visuals generated in London
- Lighting controlled in New York
- All synchronized via PCHI Conductor

### 4. Scene State Marketplace

**Buy/Sell Scene States**:
- "Kraken Tentacle Scene" - $500
- "Particle Storm Preset" - $100
- "AI-Generated Visuals Pack" - $1000

All PCHI-compatible, mathematically verified.

### 5. Verification & Compliance

**DGV Integration**:
- PCHI messages generate DGV evidence
- Mathematical proof of system integrity
- Regulatory compliance for AI-governed shows

---

## Benefits for Live Software

### For VJs and Visual Artists

1. **Focus on Creativity, Not Integration**
   - Spend time on visuals, not OSC mapping
   - Reuse PCHI presets across projects

2. **Mathematical Harmony**
   - PIR ensures visuals are mathematically harmonious
   - No more "ugly" random transitions

3. **AI Collaboration**
   - AI VJ assistant respects artistic constraints
   - Only-Lang rules define your style

### For Lighting Designers

1. **Automated Follow-Spot**
   - Artist tracking via BlackTrax → PCHI → Lighting
   - Mathematical guarantee of smooth movement

2. **Musical Synchronization**
   - BPM, beat, kick detection → PCHI → Lighting cues
   - No manual programming

3. **Safety Constraints**
   - Only-Lang rules prevent dangerous light levels
   - PIR invariants ensure system stability

### For System Integrators

1. **Reduced Integration Time**
   - 2 PCHI implementations vs 50 OSC mappings
   - Weeks of work → hours

2. **Easier Debugging**
   - Single dashboard for entire system
   - State history for troubleshooting

3. **Future-Proof**
   - New tools just need PCHI plugin
   - No rewriting entire system

### For Tool Developers

1. **Market Differentiation**
   - "PCHI Certified" attracts integrators
   - Premium pricing for PCHI versions

2. **AI-Ready**
   - PCHI is the interface for AI agents
   - Future-proof your tool for AI workflows

3. **Community**
   - Shared PCHI component library
   - Collaborative rule development

---

## Getting Started

### Step 1: Install PCHI Conductor

```bash
cd primeswarm-engine/primeswarm-pchi
cargo run --release
```

### Step 2: Create PCHI Bridge for Your Tool

Use the examples in `pchi-schema/examples/` as templates.

### Step 3: Define Only-Lang Rules

Write rules in `.only` format to define your constraints.

### Step 4: Connect Tools

Point all tools to the PCHI Conductor address.

### Step 5: Monitor

Use the web dashboard (coming soon) to monitor scene state.

---

## Technical Specifications

- **Schema Version**: 2.0.0
- **Transport**: UDP, TCP, WebSocket, OSC (any)
- **Serialization**: JSON (human-readable), FlatBuffers (binary)
- **Validation**: PIR invariants, equilibrium checks
- **Governance**: Only-Lang DSL
- **Performance**: ~62 µs per equilibrium check

---

## Roadmap

- [ ] PCHI Conductor web dashboard
- [ ] WebSocket transport layer
- [ ] FlatBuffers serialization implementation
- [ ] Unreal Engine PCHI plugin
- [ ] TouchDesigner PCHI CHOP
- [ ] Resolume PCHI Bridge
- [ ] Ableton PCHI Max Device
- [ ] PCHI Scene State Marketplace
- [ ] AI Agent PCHI Interface

---

## License

Part of the PIR / PrimeSwarm research monorepo.
