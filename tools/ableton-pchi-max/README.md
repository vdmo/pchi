# Ableton PCHI Max Device

Connects Ableton Live to the PCHI Conductor via Max for Live.

## Overview

This Max for Live device monitors Ableton Live's musical state (BPM, beat, clip changes) and sends PCHI messages to the PCHI Conductor with PIR mathematical invariants.

## Features

- **Musical Context Monitoring**: Tracks BPM, beat position, kick/snare detection
- **Clip Change Detection**: Sends PCHI messages when clips change
- **PIR Invariants**: Calculates equilibrium from musical state
- **Real-time Communication**: Sends messages via UDP to PCHI Conductor

## Installation

1. Copy `pchi_sender.maxpat` to your Max for Live devices folder
2. In Ableton Live, drag the device onto a MIDI track
3. Configure the PCHI Conductor host and port in the device

## Configuration

- **Host**: PCHI Conductor host (default: 127.0.0.1)
- **Port**: PCHI Conductor port (default: 8888)

## PCHI Message Format

The device sends PCHI messages in JSON format:

```json
{
  "version": "2.0.0",
  "type": "musical_context",
  "source_id": "ableton_pchi",
  "timestamp": 1692345678900.0,
  "pir_invariants": {
    "equilibrium_check": {
      "residual": 0.000000000001,
      "precision": 1e-12,
      "sign_sequence": [1, -1, -1, 1]
    },
    "coherence_gap": 0.0123,
    "curvature_signature": "0.000456, -0.000123, 0.000789",
    "residual": 0.000000000001
  },
  "payload": {
    "payload_type": "musical_context",
    "data": {
      "bpm": 128.0,
      "beat": 16,
      "kick": true,
      "snare": false,
      "section": "drop"
    }
  }
}
```

## PIR Invariants Calculation

The device calculates PIR invariants from Ableton's musical state:

- **Equilibrium Check**: Uses Prouhet-Thue-Morse sequence on BPM, beat, kick, snare values
- **Coherence Gap**: Measures variance in musical parameters
- **Curvature Signature**: Approximates second derivative of musical changes

## Integration with PCHI Conductor

The device sends PCHI messages to the PCHI Conductor's UDP transport layer on port 8888. The conductor validates the PIR invariants and broadcasts the musical context to other connected tools.

## Use Cases

- **Visuals Synchronization**: Sync visuals to musical beats and sections
- **Lighting Control**: Trigger lighting cues based on musical context
- **Generative Art**: Drive generative visuals with musical parameters
- **Live Performance**: Coordinate multiple tools with Ableton as the master clock

## Troubleshooting

- **No messages sent**: Check UDP send settings and PCHI Conductor is running
- **Incorrect BPM**: Verify Ableton Live's tempo is being read correctly
- **Connection refused**: Ensure PCHI Conductor is accessible on the network

## License

Part of the PIR / PrimeSwarm research monorepo.
