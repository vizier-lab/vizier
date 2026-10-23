import { type RouteConfig, index, layout, route } from '@react-router/dev/routes'

export default [
    // Public
    layout('layouts/AuthLayout.tsx', [
        route('login', 'routes/login.tsx'),
        route('onboarding', 'routes/onboarding.tsx'),
    ]),

    // Signed in
    layout('layouts/RequireAuth.tsx', [
        layout('layouts/AppShell.tsx', [
            index('routes/home.tsx'),
            route('agents/new', 'routes/agent-new.tsx'),

            route(':agentId/chat', 'routes/chat.tsx', { id: 'chat-latest' }),
            route(':agentId/chat/read', 'routes/chat-read.tsx'),
            route(':agentId/chat/:topicId', 'routes/chat.tsx', { id: 'chat-topic' }),
            route(':agentId/core', 'routes/agent-core.tsx'),
            route(':agentId/memory', 'routes/memory.tsx', { id: 'memory-all' }),
            route(':agentId/memory/:bundle', 'routes/memory.tsx', { id: 'memory-bundle' }),
            route(':agentId/tasks', 'routes/tasks.tsx'),
            route(':agentId/skills', 'routes/skills.tsx'),
            route(':agentId/skills/:slug/edit', 'routes/skill-edit.tsx'),
            route(':agentId/dream', 'routes/agent-dream.tsx'),
            route(':agentId/usage', 'routes/usage.tsx'),
            route(':agentId/config', 'routes/agent-config.tsx'),
            route(':agentId/integrations/channels', 'routes/integrations-channels.tsx'),
            route(':agentId/integrations/addons', 'routes/integrations-addons.tsx'),
            route(':agentId/integrations/mcp', 'routes/integrations-mcp.tsx'),
            route(':agentId/settings', 'routes/redirect-agent-settings.tsx'),

            layout('layouts/SettingsLayout.tsx', [
                route('settings', 'routes/redirect-settings.tsx'),
                route('settings/profile', 'routes/settings-profile.tsx'),
                route('settings/password', 'routes/settings-password.tsx'),
                route('settings/api-keys', 'routes/settings-api-keys.tsx'),
                route('settings/providers', 'routes/settings-providers.tsx'),
                route('settings/users', 'routes/settings-users.tsx'),
                route('settings/roles', 'routes/settings-roles.tsx'),
            ]),

            route('*', 'routes/redirect-unknown.tsx'),
        ]),
    ]),
] satisfies RouteConfig
