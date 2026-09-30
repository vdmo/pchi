using System.Net.Sockets;
using System.Text;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

namespace Lib.io.pchi;

/// <summary>
/// Sends a control-parameter update directly to a PCHI Conductor as raw
/// JSON over UDP — the real wire format
/// (pchi-schema/src/lib.rs::PCHIMessage), not OSC. This is the native
/// alternative to routing through tools/tixl-pchi-bridge: no external
/// process, no OSC round-trip, this operator speaks the protocol
/// directly.
///
/// Field-name lesson learned the hard way while building the Resolume and
/// TiXL OSC bridges (vdmo/pchi#3): PCHIMessage.message_type serializes
/// under the wire name "type" (#[serde(rename = "type")] in the Rust
/// schema), and Payload is a tagged enum keyed "payloadType"/"data"
/// (#[serde(tag = "payloadType", content = "data")]) — not the Rust
/// field's own snake_case name. BuildWireJson() below is the one place
/// that mapping happens, matching the Python bridges' to_wire_dict().
/// </summary>
[Guid("6d6aa537-f7de-452f-ade6-b063694f0cee")]
internal sealed class PCHISendControlParameter : Instance<PCHISendControlParameter>, IStatusProvider
{
    [Output(Guid = "d11a663b-c0cd-49fc-ad9f-c7ce132b5335")]
    public readonly Slot<Command> Result = new();

    public PCHISendControlParameter()
    {
        Result.UpdateAction += Update;
    }

    private void Update(EvaluationContext context)
    {
        var shouldSend = SendTrigger.GetValue(context);
        if (!shouldSend)
            return;

        var host = PchiHost.GetValue(context);
        var port = PchiPort.GetValue(context);
        var targetId = TargetId.GetValue(context);
        var parameter = Parameter.GetValue(context);
        var value = Value.GetValue(context);

        if (string.IsNullOrWhiteSpace(targetId) || string.IsNullOrWhiteSpace(parameter))
        {
            _lastErrorMessage = "TargetId and Parameter must both be set";
            return;
        }

        if (port is <= 0 or > 65535)
        {
            _lastErrorMessage = "PchiPort must be in the 1-65535 range";
            return;
        }

        try
        {
            var json = BuildWireJson(targetId, parameter, value);
            var bytes = Encoding.UTF8.GetBytes(json);

            _client ??= new UdpClient();
            _client.Send(bytes, bytes.Length, host, port);
            _lastErrorMessage = null;
        }
        catch (Exception e)
        {
            _lastErrorMessage = $"Failed to send PCHI message: {e.Message}";
        }
    }

    /// <summary>
    /// The full PCHIMessage envelope, with a complete (zero-residual)
    /// pir_invariants block — the schema has no #[serde(default)] on any
    /// PIRInvariants field, so an incomplete or omitted block is rejected
    /// outright, not defaulted. A single control-parameter value has no
    /// genuine multi-value equilibrium claim to make in the first place
    /// (see vdmo/pchi#4) — this reports an honest zero, not a fabricated
    /// one, and (post vdmo/pchi#4) it's informational only for this
    /// message type, not a pass/fail gate.
    /// </summary>
    private static string BuildWireJson(string targetId, string parameter, float value)
    {
        var message = new JObject
        {
            ["version"] = "2.0.0",
            ["type"] = "control_parameter",
            ["source_id"] = "tixl_native_operator",
            ["timestamp"] = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
            ["pir_invariants"] = new JObject
            {
                ["equilibrium_check"] = new JObject
                {
                    ["residual"] = 0.0,
                    ["precision"] = 1e-12,
                    ["sign_sequence"] = new JArray(),
                },
                ["coherence_gap"] = 0.0,
                ["curvature_signature"] = "0.0, 0.0, 0.0",
                ["residual"] = 0.0,
            },
            ["payload"] = new JObject
            {
                ["payloadType"] = "control_parameter",
                ["data"] = new JObject
                {
                    ["target_id"] = targetId,
                    ["parameter"] = parameter,
                    ["value"] = value,
                },
            },
        };
        return message.ToString(Formatting.None);
    }

    protected override void Dispose(bool isDisposing)
    {
        if (isDisposing)
            _client?.Dispose();
    }

    private UdpClient _client;

    public IStatusProvider.StatusLevel GetStatusLevel()
        => string.IsNullOrEmpty(_lastErrorMessage) ? IStatusProvider.StatusLevel.Success : IStatusProvider.StatusLevel.Warning;

    string IStatusProvider.GetStatusMessage() => _lastErrorMessage;
    private string _lastErrorMessage;

    [Input(Guid = "7f4ed76b-ae25-41d5-b0d8-b4af2c7aa971")]
    public readonly InputSlot<bool> SendTrigger = new();

    [Input(Guid = "71739432-4939-4070-828f-25a58ef2239e")]
    public readonly InputSlot<string> PchiHost = new() { Value = "127.0.0.1" };

    [Input(Guid = "3127a0de-730f-4585-a2ee-ec70520117d9")]
    public readonly InputSlot<int> PchiPort = new() { Value = 8888 };

    [Input(Guid = "782ad5a0-ffd4-4d58-a303-57233a65a6b7")]
    public readonly InputSlot<string> TargetId = new();

    [Input(Guid = "8a1c2e3f-4b5d-6e7f-8091-a2b3c4d5e6f7")]
    public readonly InputSlot<string> Parameter = new();

    [Input(Guid = "9b2d3f4a-5c6e-7f80-91a2-b3c4d5e6f708")]
    public readonly InputSlot<float> Value = new();
}
