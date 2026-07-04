// AI copilot agent loop (GLM / OpenAI-compatible function calling).
//
// Runs client-side because tools execute against the live document (the WASM
// worker lives in the browser). Each turn: build a chat request (system + tools
// + a fresh document snapshot) → POST to the server proxy (which talks to GLM)
// → run any tool_calls (destructive ones gated on user approval) → feed the
// results back as role:"tool" messages → repeat until the model stops. History
// is the running OpenAI-style messages array (without the system message).

import { AI_TOOLS, executeTool, requiresApproval, documentSnapshot } from './aiTools';

const MODEL = 'glm-4-plus';
const MAX_ITERS = 12; // tool round-trips per user turn

export const SYSTEM_PROMPT = `You are the CAD copilot inside rCAD, a mechanical-CAD app. You build and edit 3D models by calling the provided tools.

Environment:
- Units are millimetres. The world is Y-up (Y is vertical/height; X-Z is the ground plane).
- Primitive params: box=[width,height,depth], cylinder=[radius,height], sphere=[radius], cone=[bottomRadius,topRadius,height].
- New parts are created at the origin. Use move_feature to place them; Y+ moves up.

How to work:
- Read the [current document] block (or call list_features) to see existing parts, their ids, dimensions and bounding boxes before editing.
- Refer to parts by their id. Select the part you are actively changing so the user can see it.
- After each change, trust the tool result (it returns the new bounding box) to verify the size/placement.
- For an organic / freeform shape that is not a simple primitive (a gear, a bracket, a toy, a figurine), use generate_mesh with a short text prompt to synthesise it, then place it.
- For a bolt use make_bolt; for a gear use make_gear (teeth + module). For any turned/lathed shape (vase, bottle, cup, knob, pillar, wheel, nozzle) use make_revolve with a {y,radius} profile — it builds a single clean solid of revolution, far better than stacking primitives.

Important limitations:
- boolean_op (union/subtract/intersect) uses a weak kernel and OFTEN produces wrong or empty geometry. Prefer building ASSEMBLIES by positioning separate parts over using subtract. If you must use a boolean, check the returned bounding box and undo + try another approach if it looks wrong.
- For physics (drop tests, "does it stand"), set parts with set_physics (fixed = a static anchor). The user runs the simulation from the toolbar.

Process line (a simulated conveyor+station cell, visible in the Process Twin panel): you can monitor and control it. Station 2 is a vision-inspection station that passes/fails each part. Use line_status to read the live state (including inspection pass/reject counts and the last verdict); line_control (start/stop/reset) to run the belt; set_dwell for station processing time; set_speed for belt speed; set_fail_rate to change the mock inspection defect rate. Answer status/quality questions from line_status.
You can also author automation rules that react to tags: add_rule (when "tag op value", do stop/start/set_speed/set_dwell/set_fail_rate/alarm). E.g. "stop the line if rejects exceed 5" → add_rule(reject.count > 5, stop) plus an alarm rule. Use list_rules / clear_rules to inspect or reset them.
You can redesign the whole line as a multi-step flow graph with set_flow_graph: nodes are steps (source=parts enter, process=a station, inspect=a QC pass/fail check, sink=parts exit like pack or reject) and edges connect them, where an edge leaving an inspect step can route on when="pass"/"fail". Cycles are allowed for rework (send failed parts back to an earlier step). E.g. "build a wash→mill→drill→inspect line, send failures to a rework station and back" → set_flow_graph with those nodes and a fail edge from inspect to rework and back to mill. Use get_flow_graph to read the current line first when editing it.

Be concise. Explain briefly what you did. Ask for clarification only when the request is genuinely ambiguous.`;

// OpenAI-style tool schema derived from the shared tool contract.
const OPENAI_TOOLS = AI_TOOLS.map((t) => ({
  type: 'function',
  function: { name: t.name, description: t.description, parameters: t.input_schema },
}));

// Loose OpenAI message shape.
export interface Msg {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content?: string | null;
  tool_calls?: RawToolCall[];
  tool_call_id?: string;
}
interface RawToolCall {
  id: string;
  type: string;
  function: { name: string; arguments: string };
}

export interface ToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface AgentCallbacks {
  // Streaming assistant text — one chunk at a time.
  onAssistantDelta: (textChunk: string) => void;
  onToolCall: (call: ToolCall) => void;
  onToolResult: (id: string, result: unknown) => void;
  // Approval-gated tools pause here for user approval.
  requestApproval: (call: ToolCall) => Promise<boolean>;
  onError: (message: string) => void;
  // Token usage for one model call (for the cost display); cached = prefix cache.
  onUsage?: (usage: Usage) => void;
}

export interface Usage {
  prompt_tokens: number;
  completion_tokens: number;
  cached_tokens: number;
}

interface ModelResult {
  message: Msg | null;
  finish: string;
  error?: string;
  usage?: Usage;
}

interface ToolAcc {
  id: string;
  name: string;
  args: string;
}

// Stream the GLM response (SSE), emitting text deltas live and accumulating any
// tool calls, then return the assembled assistant message.
async function callModel(messages: Msg[], onDelta: (t: string) => void): Promise<ModelResult> {
  let res: Response;
  try {
    res = await fetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: MODEL,
        max_tokens: 2048,
        stream: true,
        stream_options: { include_usage: true }, // final chunk reports token usage
        messages: [{ role: 'system', content: SYSTEM_PROMPT }, ...messages],
        tools: OPENAI_TOOLS,
        tool_choice: 'auto',
      }),
    });
  } catch (e) {
    return { message: null, finish: '', error: e instanceof Error ? e.message : 'network error' };
  }

  // Errors come back as JSON (not a stream).
  if (!res.ok || !res.body || !res.headers.get('content-type')?.includes('event-stream')) {
    let msg = `server returned ${res.status}`;
    try {
      const data = await res.json();
      const e = data.error as unknown;
      msg = typeof e === 'string' ? e : (e as { message?: string })?.message || msg;
    } catch {
      /* keep default */
    }
    return { message: null, finish: '', error: msg };
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let content = '';
  let finish = '';
  let usage: Usage | undefined;
  const tools = new Map<number, ToolAcc>();

  const handle = (data: string) => {
    if (data === '[DONE]') return;
    let obj: {
      choices?: Array<{ delta?: Record<string, unknown>; finish_reason?: string }>;
      usage?: { prompt_tokens?: number; completion_tokens?: number; prompt_tokens_details?: { cached_tokens?: number } };
    };
    try {
      obj = JSON.parse(data);
    } catch {
      return;
    }
    // The usage chunk (include_usage) has empty choices, so read it first.
    if (obj.usage) {
      usage = {
        prompt_tokens: obj.usage.prompt_tokens ?? 0,
        completion_tokens: obj.usage.completion_tokens ?? 0,
        cached_tokens: obj.usage.prompt_tokens_details?.cached_tokens ?? 0,
      };
    }
    const choice = obj.choices?.[0];
    if (!choice) return;
    const delta = choice.delta ?? {};
    if (typeof delta.content === 'string' && delta.content) {
      content += delta.content;
      onDelta(delta.content);
    }
    const deltaTools = delta.tool_calls as
      | Array<{ index?: number; id?: string; function?: { name?: string; arguments?: string } }>
      | undefined;
    if (deltaTools) {
      for (const tc of deltaTools) {
        const idx = tc.index ?? 0;
        const cur = tools.get(idx) ?? { id: '', name: '', args: '' };
        if (tc.id) cur.id = tc.id;
        if (tc.function?.name) cur.name = tc.function.name;
        if (tc.function?.arguments) cur.args += tc.function.arguments;
        tools.set(idx, cur);
      }
    }
    if (choice.finish_reason) finish = choice.finish_reason;
  };

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let nl: number;
    while ((nl = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, nl).trim();
      buffer = buffer.slice(nl + 1);
      if (line.startsWith('data:')) handle(line.slice(5).trim());
    }
  }

  const tool_calls = [...tools.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([, v]) => ({ id: v.id, type: 'function', function: { name: v.name, arguments: v.args } }));
  const message: Msg = {
    role: 'assistant',
    content: content || null,
    ...(tool_calls.length ? { tool_calls } : {}),
  };
  return { message, finish, usage };
}

/** Run one user turn to completion, returning the updated conversation history. */
// Client-side prompt cache: the document snapshot is large and is re-grounded
// each turn, but the model already has the previous one in the conversation.
// Only resend it when it actually changed (or the conversation is new) — this
// cuts prompt tokens (and cost) on multi-turn sessions where GLM exposes no
// provider-side prompt cache.
let lastSnapshot: string | null = null;

export async function runTurn(
  history: Msg[],
  userText: string,
  cb: AgentCallbacks
): Promise<Msg[]> {
  const snapshot = JSON.stringify(documentSnapshot());
  const fresh = history.length === 0 || snapshot !== lastSnapshot;
  lastSnapshot = snapshot;
  const content = fresh
    ? `${userText}\n\n[current document]\n${snapshot}`
    : `${userText}\n\n[document unchanged since my last message]`;
  const messages: Msg[] = [...history, { role: 'user', content }];

  for (let iter = 0; iter < MAX_ITERS; iter++) {
    const res = await callModel(messages, cb.onAssistantDelta);
    if (res.usage) cb.onUsage?.(res.usage);
    if (res.error || !res.message) {
      cb.onError(res.error || 'empty model response');
      return messages;
    }
    const msg = res.message;
    messages.push(msg);
    // Text was already streamed via onAssistantDelta.

    const toolCalls = msg.tool_calls ?? [];
    if (toolCalls.length === 0) return messages; // model is done

    for (const tc of toolCalls) {
      let input: Record<string, unknown> = {};
      try {
        input = tc.function.arguments ? JSON.parse(tc.function.arguments) : {};
      } catch {
        /* leave input empty; executeTool will surface the mistake */
      }
      const call: ToolCall = { id: tc.id, name: tc.function.name, input };
      cb.onToolCall(call);
      let result: unknown;
      if (requiresApproval(call.name)) {
        const approved = await cb.requestApproval(call);
        result = approved
          ? await executeTool(call.name, input)
          : { error: 'The user declined this action.' };
      } else {
        result = await executeTool(call.name, input);
      }
      cb.onToolResult(tc.id, result);
      messages.push({ role: 'tool', tool_call_id: tc.id, content: JSON.stringify(result) });
    }
  }

  cb.onError('Reached the tool-call limit for one turn.');
  return messages;
}

// Dev-only handle for headless verification (no effect in prod).
const isDev = (import.meta as unknown as { env?: { DEV?: boolean } }).env?.DEV;
if (typeof window !== 'undefined' && isDev) {
  (window as unknown as { __aiAgent: unknown }).__aiAgent = { runTurn, SYSTEM_PROMPT };
}
