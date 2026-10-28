import assert from 'node:assert/strict';
import test from 'node:test';
import { dummyplugScenarios } from './dummyplug-vizier-e2e.mjs';

test('integration fixture provides isolated retry, cancellation, and response plans', () => {
  assert.equal(dummyplugScenarios.retryThenSuccess.length, 2);
  assert.equal(dummyplugScenarios.exhaustedRetries.length, 4);
  assert.ok(dummyplugScenarios.delayedCancellation[0].delay_ms > 0);
  assert.match(dummyplugScenarios.truncatedSse[0].body, /partial/u);
  assert.equal(dummyplugScenarios.nonzeroUsage[0].body.usage.total_tokens, 17);
  assert.equal(dummyplugScenarios.cachedUsage[0].body.usage.prompt_tokens_details.cached_tokens, 8);
  assert.equal(dummyplugScenarios.reasoning[0].body.choices[0].message.reasoning_content, 'test reasoning');
  assert.equal(dummyplugScenarios.multipleToolCalls[0].body.choices[0].message.tool_calls.length, 2);
  assert.equal(dummyplugScenarios.malformedToolArguments[0].body.choices[0].message.tool_calls[0].function.arguments, '{');
  assert.equal(dummyplugScenarios.alternateFinishReason[0].body.choices[0].finish_reason, 'length');
});
