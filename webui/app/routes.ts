import {
  type RouteConfig,
  index,
  layout,
  route,
} from '@react-router/dev/routes'

export default [
  // Public routes (no auth)
  route('login', 'routes/login.tsx'),

  // Protected routes with layout
  layout('layout.tsx', [
    index('routes/home.tsx'),
    route('settings', 'routes/settingsRoot.tsx'),
    route('agent/new', 'routes/newAgent.tsx'),
    route('agent/:agentId/chat/:topicId', 'routes/chat.tsx'),
    route('agent/:agentId/memory', 'routes/memory.tsx'),
    route('agent/:agentId/tasks', 'routes/tasks.tsx'),
    route('agent/:agentId/documents', 'routes/documents.tsx'),
    route('agent/:agentId/usage', 'routes/usage.tsx'),
    route('agent/:agentId/settings', 'routes/settings.tsx'),
  ]),
] satisfies RouteConfig
