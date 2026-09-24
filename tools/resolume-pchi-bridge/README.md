# Resolume PCHI Bridge

Connects Resolume Arena to the PCHI Conductor via OSC.

## Overview

This bridge listens to OSC messages from Resolume Arena, converts them to PCHI messages with PIR mathematical invariants, and forwards them to the PCHI Conductor.

## Features

- **OSC to PCHI Conversion**: Automatically converts Resolume OSC messages to PCHI format
- **PIR Invariants**: Calculates equilibrium, coherence gap, and curvature signature using Prouhet-Thue-Morse sequences
- **Real-time Processing**: Handles live OSC streams with minimal latency
- **Heartbeat**: Sends periodic heartbeats to maintain connection with PCHI Conductor

## Installation

```bash
pip install python-osc numpy
```

## Usage

```bash
python resolume_pchi_bridge.py \
    --resolume-port 7000 \
    --pchi-host 127.0.0.1 \
    --pchi-port 8888 \
    --threshold 1e-12
```

## Configuration in Resolume Arena

1. Open Resolume Arena Preferences → OSC
2. Enable OSC Output
3. Set Output Address to `127.0.0.1`
4. Set Output Port to `7000` (or your configured port)
5. Enable OSC Input (for receiving commands from PCHI Conductor)

## OSC Address Mapping

Resolume OSC addresses are mapped to PCHI control parameters:

```
/composition/layers/{layer_id}/video/{parameter} → resolume_layer_{layer_id}.{parameter}
```

Common parameters:
- `opacity` - Layer opacity (0.0 - 1.0)
- `playbackspeed` - Playback speed
- `position` - Clip position
- `blendmode` - Blend mode

## PIR Invariants

The bridge calculates PIR invariants from the current state:

- **Equilibrium Check**: Uses Prouhet-Thue-Morse sign sequence to verify mathematical balance
- **Coherence Gap**: Measures variance from mean to detect state drift
- **Curvature Signature**: Approximates second derivative for smoothness

## Example PCHI Message

```json
{
  "version": "2.0.0",
  "type": "control_parameter",
  "source_id": "resolume_bridge",
  "timestamp": 1692345678900.0,
  "pir_invariants": {
    "equilibrium_check": {
      "residual": 0.000000000001,
      "precision": 1e-12,
      "sign_sequence": [1, -1, -1, 1]
    },
    "coherence_gap": 0.0234,
    "curvature_signature": "0.001234, -0.000567, 0.000890",
    "residual": 0.000000000001
  },
  "payload": {
    "payload_type": "control_parameter",
    "data": {
      "target_id": "resolume_layer_1",
      "parameter": "opacity",
      "value": 0.85
    }
  }
}
```

## Integration with PCHI Conductor

The bridge sends PCHI messages to the PCHI Conductor via OSC on port 8888. The conductor validates the PIR invariants and applies Only-Lang rules before broadcasting to other connected tools.

## Troubleshooting

- **No messages received**: Check Resolume OSC output settings
- **Connection refused**: Ensure PCHI Conductor is running on the specified host/port
- **High residual values**: May indicate disequilibrium in the visual state

## License

Part of the PIR / PrimeSwarm research monorepo.
