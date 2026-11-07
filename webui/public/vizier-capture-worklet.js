// Collects the input's first channel and posts each 128-frame block to the page. A file of its own
// because the desktop app's CSP refuses a worklet loaded from a blob: URL (see lib/wavRecorder.ts).
class Capture extends AudioWorkletProcessor {
    process(inputs) {
        const channel = inputs[0] && inputs[0][0]
        if (channel) this.port.postMessage(channel.slice())
        return true
    }
}
registerProcessor('vizier-capture', Capture)
