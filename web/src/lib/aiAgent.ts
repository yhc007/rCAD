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

Important limitations:
- boolean_op (union/subtract/intersect) uses a weak kernel and OFTEN produces wrong or empty geometry. Prefer building ASSEMBLIES by positioning separate parts over using subtract. If you must use a boolean, check the returned bounding box and undo + try another approach if it looks wrong.
- For physics (drop tests, "does it stand"), set parts with set_physics (fixed = a static anchor). The user runs the simulation from the toolbar.

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
  onAssistantText: (text: string) => void;
  onToolCall: (call: ToolCall) => void;
  onToolResult: (id: string, result: unknown) => void;
  // Destructive tools (boolean_op, delete_feature) pause here for user approval.
  requestApproval: (call: ToolCall) => Promise<boolean>;
  onError: (message: string) => void;
}

interface ModelResult {
  message: Msg | null;
  finish: string;
  error?: string;
}

async function callModel(messages: Msg[]): Promise<ModelResult> {
  let res: Response;
  try {
    res = await fetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: MODEL,
        max_tokens: 2048,
        messages: [{ role: 'system', content: SYSTEM_PROMPT }, ...messages],
        tools: OPENAI_TOOLS,
        tool_choice: 'auto',
      }),
    });
  } catch (e) {
    return { message: null, finish: '', error: e instanceof Error ? e.message : 'network error' };
  }
  let data: Record<string, unknown>;
  try {
    data = await res.json();
  } catch {
    return { message: null, finish: '', error: `server returned ${res.status}` };
  }
  const errField = data.error as unknown;
  if (!res.ok || errField) {
    const msg =
      typeof errField === 'string'
        ? errField
        : (errField as { message?: string })?.message || `server returned ${res.status}`;
    return { message: null, finish: '', error: msg };
  }
  const choice = (data.choices as Array<{ message: Msg; finish_reason: string }> | undefined)?.[0];
  if (!choice) return { message: null, finish: '', error: 'no choices in model response' };
  return { message: choice.message, finish: choice.finish_reason ?? '' };
}

/** Run one user turn to completion, returning the updated conversation history. */
export async function runTurn(
  history: Msg[],
  userText: string,
  cb: AgentCallbacks
): Promise<Msg[]> {
  // Ground the model in the current document at the start of the turn.
  const snapshot = JSON.stringify(documentSnapshot());
  const messages: Msg[] = [
    ...history,
    { role: 'user', content: `${userText}\n\n[current document]\n${snapshot}` },
  ];

  for (let iter = 0; iter < MAX_ITERS; iter++) {
    const res = await callModel(messages);
    if (res.error || !res.message) {
      cb.onError(res.error || 'empty model response');
      return messages;
    }
    const msg = res.message;
    messages.push(msg);
    if (msg.content && msg.content.trim()) cb.onAssistantText(msg.content);

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
