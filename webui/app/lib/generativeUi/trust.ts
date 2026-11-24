// Whether the action chain now running was started by a person (research R9). Every emitted
// event records it first; `sendMessage` checks it. A timer, a load or a function's result
// records `false`, so only a real click, key or tap can send a message as the person. The
// latest emit wins, which fails closed: a tick landing mid-chain refuses the send.

let trusted = false

export function noteGesture(isTrusted: boolean): void {
  trusted = isTrusted
}

export function currentChainIsTrusted(): boolean {
  return trusted
}
