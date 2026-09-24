# PCHI Schema v2.0

PCHI (Peachy) protocol schema with PIR mathematical invariants for scene state management.

## Overview

PCHI v2.0 extends the original PCHI protocol with Prime Integer Relations (PIR) invariants, enabling mathematically verified scene state management across distributed creative systems.

## Features

- **Standardized Schema**: Common message format for scene updates, control parameters, musical context, and artist tracking
- **PIR Invariants**: Mathematical equilibrium checks, coherence gap measures, and curvature signatures
- **Validation**: Built-in validation for message integrity and equilibrium compliance
- **Multiple Serializations**: JSON (human-readable) and FlatBuffers (high-performance binary)

## Message Types

- `scene_update`: Object transforms and custom data
- `control_parameter`: Parameter changes for target systems
- `musical_context`: BPM, beat, kick/snare detection, song sections
- `artist_tracking`: Real-time position and velocity tracking
- `heartbeat`: System health monitoring

## PIR Invariants

Every PCHI message includes mathematical invariants:

```json
{
  "pirInvariants": {
    "equilibriumCheck": {
      "residual": 0.0,
      "precision": 1e-12,
      "signSequence": [1, -1, -1, 1]
    },
    "coherenceGap": 0.0,
    "curvatureSignature": "0.0, 0.0, 0.0",
    "residual": 0.0
  }
}
```

## Usage

### Creating a Message

```rust
use pchi_schema::{PCHIMessage, MessageType, Payload, SceneUpdateData};

let message = PCHIMessage::new(
    MessageType::SceneUpdate,
    "resolume_bridge".to_string(),
    Payload::SceneUpdate(SceneUpdateData {
        objects: vec![/* ... */],
    }),
);

// Validate equilibrium
assert!(message.is_equilibrium());

// Full validation
message.validate()?;
```

### Validation

```rust
use pchi_schema::validation::{validate_message, ValidationConfig};

let config = ValidationConfig {
    max_residual: 1e-12,
    max_coherence_gap: 1.0,
    strict_timestamp: true,
};

validate_message_with_config(&message, &config)?;
```

## Schema Files

- `pchi-v2.0-schema.json`: JSON Schema for validation
- `pchi.fbs`: FlatBuffers schema for binary serialization

## Building

```bash
cd prime-market
cargo build -p pchi-schema
cargo test -p pchi-schema
```

## Integration

This crate is used by:
- **PCHI Conductor**: Central state management
- **PCHI Bridges**: Tool-specific adapters (Resolume, TouchDesigner, Unreal)
- **ONLY Engine**: Governance and equilibrium enforcement

## License

Part of the PIR / PrimeSwarm research monorepo.
