# Page examples

Whole pages, returned by `ui_examples(kind)`. Each `### \`kind\`` section is one example.

### `dashboard`

Stats, a chart in a card, and a table filled from script.

```html
<h2>Signups · week 46</h2>
<div class="vz-grid" style="--vz-min:140px">
  <vz-stat label="New" value="1284" delta="+12% on last week"></vz-stat>
  <vz-stat label="Activated" value="0.61" format="percent"></vz-stat>
  <vz-stat label="Churned" value="0.032" format="percent" tone="error"></vz-stat>
</div>
<vz-card title="By day">
  <vz-chart kind="bar" data='[{"label":"Mon","value":160},{"label":"Tue","value":210},{"label":"Wed","value":190},{"label":"Thu","value":260}]'></vz-chart>
</vz-card>
<vz-table id="top" label="Top sources" columns='[{"key":"source","label":"Source"},{"key":"n","label":"Signups","align":"end"}]'></vz-table>
<script>
  document.getElementById('top').rows = [{ source: 'Search', n: 512 }, { source: 'Referral', n: 301 }]
</script>
```

### `form`

Fields whose values are sent to you as the person's next turn.

```html
<vz-card title="Plan the trip">
  <div class="vz-stack">
    <vz-field id="city" name="city" label="City" placeholder="Lisbon"></vz-field>
    <vz-field id="days" kind="slider" name="days" label="Days" value="4" min="1" max="14"></vz-field>
    <vz-field id="pace" kind="select" name="pace" label="Pace" value="easy" options='["easy","busy"]'></vz-field>
    <div class="vz-row"><button type="submit" id="go">Plan it</button></div>
  </div>
</vz-card>
<script>
  const $ = (id) => document.getElementById(id)
  $('go').onclick = () => vizier.send('Plan my trip', { city: $('city').value, days: $('days').value, pace: $('pace').value })
</script>
```

### `game`

A canvas game loop on `requestAnimationFrame`, its state saved across reloads.

```html
<canvas id="c" width="480" height="320" style="width:100%;background:var(--vz-surface);border-radius:8px"></canvas>
<script>
  const c = document.getElementById('c'), g = c.getContext('2d')
  const s = vizier.state.get() ?? { x: 40, y: 40, score: 0 }
  const keys = new Set()
  addEventListener('keydown', e => keys.add(e.key))
  addEventListener('keyup', e => keys.delete(e.key))
  function frame() {
    if (keys.has('ArrowRight')) s.x += 3
    if (keys.has('ArrowLeft')) s.x -= 3
    g.clearRect(0, 0, c.width, c.height)
    g.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--vz-ink')
    g.fillRect(s.x, s.y, 16, 16)
    requestAnimationFrame(frame)
  }
  setInterval(() => vizier.state.set(s), 1000)
  requestAnimationFrame(frame)
</script>
```

### `worker`

An opponent that thinks on its own thread, then sends the move to you.

```html
<button id="go">My move: e2e4</button><p id="out"></p>
<script type="vz-worker" id="ai">
  self.run = ({ position }) => {
    let best = 0
    for (let i = 0; i < 5e7; i++) best = (best + i) % 97   // stands in for a real search
    return { move: "e7e5", score: best }
  }
</script>
<script>
  const ai = vizier.worker('#ai')
  document.getElementById('go').onclick = async () => {
    document.getElementById('out').textContent = 'Thinking…'
    const { move } = await ai.run({ position: 'start' }, { timeoutMs: 20000 })
    document.getElementById('out').textContent = 'Reply: ' + move
    await vizier.send('I played e2e4; the board replied ' + move, { move: 'e2e4' })
  }
</script>
```

### `three`

A spinning 3D shape with the bundled three.js, on a transparent background.

```html
<div id="view" style="height:320px"></div>
<script type="module">
  import * as THREE from "three"
  const el = document.getElementById('view')
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
  renderer.setSize(el.clientWidth, el.clientHeight)
  el.appendChild(renderer.domElement)
  const scene = new THREE.Scene()
  const camera = new THREE.PerspectiveCamera(50, el.clientWidth / el.clientHeight, 0.1, 100)
  camera.position.z = 4
  const cube = new THREE.Mesh(new THREE.BoxGeometry(1.4, 1.4, 1.4), new THREE.MeshNormalMaterial())
  scene.add(cube)
  renderer.setAnimationLoop(() => {
    cube.rotation.x += 0.01
    cube.rotation.y += 0.013
    renderer.render(scene, camera)
  })
</script>
```
