import { createDummyplugTestProxy } from './dummyplug-test-proxy.mjs';

/** Reusable plans for Vizier integration tests; the proxy owns all injected behavior. */
export const dummyplugScenarios = Object.freeze({
  retryThenSuccess: [{ status: 429, retry_after: 0 }, { status: 503, retry_after: 0 }],
  // Vizier's default policy is 4 attempts in total, including the first request.
  exhaustedRetries: [{ status: 503 }, { status: 503 }, { status: 503 }, { status: 503 }],
  delayedCancellation: [{ delay_ms: 30_000 }],
  truncatedSse: [{ body: 'data: {"choices":[{"delta":{"content":"partial"}}]}\n\n' }],
  nonzeroUsage: [{ body: { usage: { prompt_tokens: 12, completion_tokens: 5, total_tokens: 17 } } }],
  cachedUsage: [{ body: { usage: { prompt_tokens: 12, completion_tokens: 5, total_tokens: 17, prompt_tokens_details: { cached_tokens: 8 } } } }],
  reasoning: [{ body: { choices: [{ message: { reasoning_content: 'test reasoning', content: 'done' }, finish_reason: 'stop' }] } }],
  multipleToolCalls: [{ body: { choices: [{ message: { tool_calls: [
    { id: 'call-1', type: 'function', function: { name: 'first', arguments: '{}' } },
    { id: 'call-2', type: 'function', function: { name: 'second', arguments: '{"ok":true}' } },
  ] }, finish_reason: 'tool_calls' }] } }],
  malformedToolArguments: [{ body: { choices: [{ message: { tool_calls: [
    { id: 'call-bad', type: 'function', function: { name: 'echo', arguments: '{' } },
  ] }, finish_reason: 'tool_calls' }] } }],
  alternateFinishReason: [{ body: { choices: [{ message: { content: 'stopped' }, finish_reason: 'length' }] } }],
});

export function createVizierE2EHarness(options) {
  return createDummyplugTestProxy(options);
}
