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
    route('agents/new', 'routes/agent-new.tsx'),
    route('settings', 'routes/settingsRoot.tsx'),
    route(':agentId/chat', 'routes/chat.tsx', { id: 'agent-chat' }),
    route(':agentId/chat/:topicId', 'routes/chat.tsx', { id: 'agent-chat-topic' }),
    route(':agentId/memory', 'routes/memory.tsx'),
    route(':agentId/tasks', 'routes/tasks.tsx'),
    route(':agentId/settings', 'routes/agent-settings.tsx'),
    route(':agentId/usage', 'routes/usage.tsx'),
  ]),
] satisfies RouteConfig
