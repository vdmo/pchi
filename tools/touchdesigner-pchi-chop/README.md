# TouchDesigner PCHI CHOP

Receives PCHI messages from the PCHI Conductor and outputs scene state as TouchDesigner channels.

## Overview

This Python CHOP for TouchDesigner connects to the PCHI Conductor via UDP, receives PCHI messages, and outputs scene state as TouchDesigner channels for use in your TouchDesigner networks.

## Features

- **Real-time Scene State**: Object positions, transforms, and properties as CHOP channels
- **Musical Context**: BPM, beat, kick, snare as channels
- **Equilibrium Monitoring**: PIR equilibrium status as a channel
- **Automatic Channel Creation**: Dynamically creates channels based on scene objects

## Installation

1. Copy `pchi_receiver_chop.py` to your TouchDesigner scripts folder
2. In TouchDesigner, create a **Python CHOP**
3. Set the **Custom** parameter to point to the script
4. Configure parameters:
   - **Host**: PCHI Conductor host (default: 127.0.0.1)
   - **Port**: PCHI Conductor port (default: 8888)
   - **Update Rate**: Channel update rate in Hz (default: 60)

## Channel Output

The CHOP outputs the following channels:

### Equilibrium Channels
- `equilibrium` - Current PIR residual (0.0 = equilibrium)
- `coherence_gap` - Scene coherence measure

### Musical Context Channels
- `bpm` - Current BPM
- `beat` - Current beat number
- `kick` - Kick drum detection (1.0 = on, 0.0 = off)
- `snare` - Snare drum detection (1.0 = on, 0.0 = off)

### Object Channels
For each scene object `{object_id}`:
- `{object_id}_x` - X position
- `{object_id}_y` - Y position
- `{object_id}_z` - Z position

## Example Usage

1. Connect the PCHI Receiver CHOP to a **Geometry COMP**
2. Use the position channels to drive object transforms
3. Use musical context channels to trigger effects
4. Monitor equilibrium channel for system health

## Example Network

```
PCHI Receiver CHOP → Select CHOP → Geometry COMP → Render TOP
```

## PCHI Message Format

The CHOP expects PCHI messages in JSON format via UDP:

```json
{
  "type": "state_update",
  "state": {
    "objects": {
      "kraken_tentacle_1": {
        "transform": [1.0, 2.0, 3.0]
      }
    },
    "musical_context": {
      "bpm": 128.0,
      "beat": 16,
      "kick": true,
      "snare": false
    },
    "equilibrium": 0.000000000001
  }
}
```

## Integration with PCHI Conductor

The CHOP connects to the PCHI Conductor's UDP transport layer on port 8888. The conductor broadcasts state updates to all connected clients.

## Performance

- **Latency**: < 10ms for local connections
- **Update Rate**: Configurable up to 120 Hz
- **Channel Count**: Dynamically scales with scene complexity

## Troubleshooting

- **No channels appearing**: Check PCHI Conductor is running and sending messages
- **Channels not updating**: Verify host and port settings
- **High CPU usage**: Reduce update rate or simplify scene

## License

Part of the PIR / PrimeSwarm research monorepo.
