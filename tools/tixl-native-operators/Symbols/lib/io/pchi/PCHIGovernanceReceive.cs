using System.Net.WebSockets;
using System.Text;
using System.Threading;
using Newtonsoft.Json.Linq;

namespace Lib.io.pchi;

/// <summary>
/// Connects directly to a PCHI Conductor's governance-event WebSocket
/// (real wire protocol, no OSC intermediary) and exposes the latest live
/// verdict as graph values: GateState as a float code (1.0/0.5/0.0 =
/// ALLOW/ESCALATE/DENY — the same encoding
/// tools/tixl-pchi-bridge/tixl_pchi_bridge.py uses over OSC, chosen
/// because it's a single comparable number an artist can wire straight
/// into a Less/Greater node), plus ReceiptId and Tool as strings for
/// display/logging.
///
/// Requires vdmo/pchi#5 (the WS accept-loop fix) on the Conductor side —
/// without it, the Conductor's WebSocket is bound but never accepts a
/// connection, and this operator will sit at IsConnected=false forever.
///
/// Unlike OscInput, which shares connection pooling through
/// T3.IoServices.OscConnectionManager, this operator owns its connection
/// directly: one ClientWebSocket per operator instance, reconnecting on
/// failure. That infrastructure is TiXL's own internal machinery for a
/// protocol (OSC) many operators share one listening port for; a single
/// governance WebSocket per Conductor has no equivalent multi-operator
/// sharing need today.
/// </summary>
[Guid("d94e6a5f-1234-4abc-9def-0123456789ab")]
internal sealed class PCHIGovernanceReceive : Instance<PCHIGovernanceReceive>, IStatusProvider
{
    [Output(Guid = "e05f7b60-2345-4bcd-8ef0-123456789abc")]
    public readonly Slot<float> GateState = new();

    [Output(Guid = "f16a8c71-3456-4cde-9f01-23456789abcd")]
    public readonly Slot<string> ReceiptId = new();

    [Output(Guid = "0271934d-4567-4def-8f12-3456789abcde")]
    public readonly Slot<string> Tool = new();

    [Output(Guid = "1382a45e-5678-4ef0-9012-3456789abcd0")]
    public readonly Slot<bool> IsConnected = new();

    public PCHIGovernanceReceive()
    {
        GateState.UpdateAction += Update;
        ReceiptId.UpdateAction += Update;
        Tool.UpdateAction += Update;
        IsConnected.UpdateAction += Update;
    }

    private void Update(EvaluationContext context)
    {
        var host = PchiHost.GetValue(context);
        var port = PchiWsPort.GetValue(context);
        var target = $"ws://{host}:{port}";

        if (target != _connectedTarget)
        {
            StopListening();
            _connectedTarget = target;
            StartListening(target);
        }

        lock (_lock)
        {
            GateState.Value = _lastGateStateCode;
            ReceiptId.Value = _lastReceiptId ?? "";
            Tool.Value = _lastTool ?? "";
            IsConnected.Value = _isConnected;
        }
    }

    private void StartListening(string target)
    {
        _cts = new CancellationTokenSource();
        var token = _cts.Token;
        _listenTask = Task.Run(() => ListenLoop(target, token), token);
    }

    private void StopListening()
    {
        try
        {
            _cts?.Cancel();
            _listenTask?.Wait(TimeSpan.FromSeconds(1));
        }
        catch (Exception)
        {
            // Best-effort teardown on reconnect/dispose; a hung or already-
            // faulted socket must not block Update().
        }
        finally
        {
            _cts?.Dispose();
            _cts = null;
            _listenTask = null;
            lock (_lock)
            {
                _isConnected = false;
            }
        }
    }

    private async Task ListenLoop(string target, CancellationToken token)
    {
        var buffer = new byte[16 * 1024];
        while (!token.IsCancellationRequested)
        {
            try
            {
                using var socket = new ClientWebSocket();
                await socket.ConnectAsync(new Uri(target), token).ConfigureAwait(false);
                lock (_lock)
                {
                    _isConnected = true;
                }
                _lastErrorMessage = null;

                while (socket.State == WebSocketState.Open && !token.IsCancellationRequested)
                {
                    using var ms = new MemoryStream();
                    WebSocketReceiveResult result;
                    do
                    {
                        result = await socket.ReceiveAsync(new ArraySegment<byte>(buffer), token).ConfigureAwait(false);
                        if (result.MessageType == WebSocketMessageType.Close)
                            break;
                        ms.Write(buffer, 0, result.Count);
                    } while (!result.EndOfMessage);

                    if (result.MessageType == WebSocketMessageType.Close)
                        break;

                    HandleMessage(Encoding.UTF8.GetString(ms.ToArray()));
                }
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception e)
            {
                _lastErrorMessage = $"WebSocket error: {e.Message}";
            }

            lock (_lock)
            {
                _isConnected = false;
            }

            if (!token.IsCancellationRequested)
            {
                try
                {
                    await Task.Delay(TimeSpan.FromSeconds(2), token).ConfigureAwait(false);
                }
                catch (OperationCanceledException)
                {
                    break;
                }
            }
        }
    }

    private static readonly Dictionary<string, float> GateStateCodes = new()
    {
        ["ALLOW"] = 1.0f,
        ["ESCALATE"] = 0.5f,
        ["DENY"] = 0.0f,
    };

    private void HandleMessage(string raw)
    {
        JObject parsed;
        try
        {
            parsed = JObject.Parse(raw);
        }
        catch (Exception)
        {
            return; // not JSON we understand — ignore rather than fault the loop
        }

        var payload = parsed["payload"] as JObject;
        if (payload?["payloadType"]?.ToString() != "governance_event")
            return;

        var data = payload["data"] as JObject;
        var gateState = data?["gate_state"]?.ToString();
        if (gateState == null || !GateStateCodes.TryGetValue(gateState, out var code))
            return;

        lock (_lock)
        {
            _lastGateStateCode = code;
            _lastReceiptId = data?["receipt_id"]?.ToString();
            _lastTool = data?["tool"]?.ToString();
        }
    }

    protected override void Dispose(bool isDisposing)
    {
        if (isDisposing)
            StopListening();
    }

    public IStatusProvider.StatusLevel GetStatusLevel()
        => string.IsNullOrEmpty(_lastErrorMessage) ? IStatusProvider.StatusLevel.Success : IStatusProvider.StatusLevel.Warning;

    string IStatusProvider.GetStatusMessage() => _lastErrorMessage;
    private string _lastErrorMessage;

    private readonly object _lock = new();
    private float _lastGateStateCode = 1.0f; // ALLOW until a real event says otherwise
    private string _lastReceiptId;
    private string _lastTool;
    private bool _isConnected;
    private string _connectedTarget;
    private CancellationTokenSource _cts;
    private Task _listenTask;

    [Input(Guid = "249b1b6f-6789-4f01-9123-456789abcde1")]
    public readonly InputSlot<string> PchiHost = new() { Value = "127.0.0.1" };

    [Input(Guid = "35ac2c70-789a-4012-9234-56789abcde12")]
    public readonly InputSlot<int> PchiWsPort = new() { Value = 8889 };
}
