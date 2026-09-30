# TiXL native PCHI operators

Path B of the PCHI/TiXL integration (see `tools/tixl-pchi-bridge` for Path
A, the OSC bridge). Real C# operators that speak PCHI's wire protocol
**directly** — raw JSON over UDP out, WebSocket in — with no OSC
intermediary and no external bridge process. TiXL has no separate plugin
DLL system; per TiXL's own docs ([Writing C# operators](https://github.com/tixl3d/tixl/blob/main/.help/docs/advanced/WritingCodeOps.md)),
"TiXL *is* the SDK" — operators are C# source next to the graph, compiled
and hot-reloaded by the running editor. These two files are that source.

## What's here

- `Symbols/lib/io/pchi/PCHISendControlParameter.cs` — an outbound operator.
  Inputs: `TargetId`, `Parameter`, `Value`, `SendTrigger`, `PchiHost`,
  `PchiPort`. Sends a real PCHI `control_parameter` message straight to the
  Conductor's UDP port.
- `Symbols/lib/io/pchi/PCHIGovernanceReceive.cs` — an inbound operator.
  Connects to the Conductor's governance-event WebSocket and exposes
  `GateState` (float: 1.0/0.5/0.0 = ALLOW/ESCALATE/DENY, same encoding the
  OSC bridge uses), `ReceiptId`, `Tool`, and `IsConnected` as live graph
  values. **Requires [vdmo/pchi#5](https://github.com/vdmo/pchi/pull/5)**
  on the Conductor — without it the governance WebSocket is bound but
  never accepts a connection, and this operator sits at
  `IsConnected=false` forever.
- `verify-build/Pchi.csproj` — the exact project used to actually compile
  these against TiXL's real `Core.dll`. See below.

## Verification status — read this before trusting the claim

**Genuinely compiled against TiXL's real `Core.dll` on Linux — this is
the strongest verification anything in this integration has had**, and
the reason: TiXL is fully open source
([tixl3d/tixl](https://github.com/tixl3d/tixl)), and while its `Editor`
needs Windows (DirectX 11 rendering, WinForms), `Core.csproj` — the
library operators actually compile against — builds successfully on
Linux with `-p:EnableWindowsTargeting=true` (a standard, well-supported
cross-compile flag; this is not a hack). That means these two operators
were checked by the real C# compiler against the real API
(`Instance<T>`, `Slot<T>`, `InputSlot<T>`, `EvaluationContext`,
`IStatusProvider`), not written blind and hoped-correct — the build
succeeded on the first attempt, 0 warnings, 0 errors.

**Not verified: running inside a live TiXL editor.** The editor itself —
the part that turns this `.cs` file into a node on a visual graph — needs
the Windows GUI application (DirectX 11), which doesn't run here. TiXL's
own workflow for adding a custom operator is to duplicate an existing one
in the editor ("Right click → Symbol Definition → Duplicate as new
Type"), which generates the accompanying `.t3`/`.t3ui` scene-graph
definition files alongside the `.cs` — those aren't something this
environment can produce; they're written by the editor itself. Getting
this into a real graph means:

1. Open TiXL (Windows, per its own [dev setup docs](https://github.com/tixl3d/tixl/blob/main/.help/docs/install/InstallDev.md)), create or open a project.
2. Duplicate an existing IO operator (e.g. `OscOutput` for the send side,
   `OscInput` for the receive side) as a new type — this scaffolds the
   `.t3`/`.t3ui` files TiXL needs.
3. Replace the generated `.cs` body with the corresponding file here (the
   `[Guid(...)]`/`[Input]`/`[Output]` attributes may need adjusting to
   match what the duplication step assigned — TiXL owns those once a
   symbol exists in the editor).

## Reproducing the compile verification

```bash
git clone git@github.com:tixl3d/tixl.git
mkdir -p tixl/Operators/Pchi
cp -r tools/tixl-native-operators/Symbols tixl/Operators/Pchi/Symbols
cp tools/tixl-native-operators/verify-build/Pchi.csproj tixl/Operators/Pchi/Pchi.csproj

# .NET 10 SDK required (TixlNetFrameworkVersion = net10.0-windows)
cd tixl
dotnet build Operators/Pchi/Pchi.csproj -p:EnableWindowsTargeting=true -p:SolutionDir="$(pwd)/"
```

## What this does not do

Same as the OSC bridge: this only carries the signal. Gating a DMX output
on `GateState` is downstream graph wiring the artist does themselves.
