// The app shell, precached. The built assets are content-hashed and change with every
// build, so they are cached as the page loads them (see the fetch handler) rather than
// listed here.
const SHELL = ['/index.html', '/favicon.svg', '/manifest.webmanifest', '/icons/icon.svg', '/icons/icon-192.png']

const CACHE = 'vizier-shell-v1'

// The shell is precached; the install does NOT skipWaiting — a new worker waits for the
// person to choose it (see the message handler).
self.addEventListener('install', (event) => {
    event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL)))
})

// The person chose the update: take over now.
self.addEventListener('message', (event) => {
    if (event.data?.type === 'SKIP_WAITING') self.skipWaiting()
})

self.addEventListener('activate', (event) => {
    event.waitUntil(
        caches.keys().then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
    )
})

self.addEventListener('fetch', (event) => {
    const request = event.request
    if (request.method !== 'GET') return

    const url = new URL(request.url)
    if (url.origin !== self.location.origin) return

    // The API is never cached: a stale chat is a lie about a live system.
    if (url.pathname.startsWith('/api/')) return

    // WebSockets never reach a fetch handler, so the three topic streams are untouched.

    if (request.mode === 'navigate') {
        // Network first, so a new deployment is picked up; the cached shell when offline.
        event.respondWith(
            fetch(request).catch(() => caches.match('/index.html').then((r) => r ?? Response.error()))
        )
        return
    }

    // Hashed assets are immutable; the shell's own files are revalidated.
    event.respondWith(
        caches.match(request).then((cached) => {
            const network = fetch(request)
                .then((response) => {
                    if (response.ok && response.type === 'basic') {
                        const copy = response.clone()
                        caches.open(CACHE).then((cache) => cache.put(request, copy))
                    }
                    return response
                })
                .catch(() => cached ?? Response.error())
            return cached ?? network
        })
    )
})
