// Which built-in category each built-in tool belongs to, so the trail can say what a script did
// without the server telling it. The names are the dispatch keys the server reserves before any
// MCP server or function can take them, so a match here is exact.
//
// BUILTIN_TOOLS mirrors the `EXPECTED` table of `every_tool_is_in_its_category` in
// crates/vizier-core/src/agents/tools/mod.rs; `scripts/trail-tool-categories.test.mjs` in vizier
// fails when the two disagree. PROJECT_TOOLS are the coding tools of a project chat
// (crates/vizier-core/src/projects/tools/), and ADDON_TOOLS the addon ids, whose tools are
// named `<id>_<tool>`.

/** The built-in tool categories (`ToolCategory`), by their snake_case names. */
export type BuiltinCategory =
    | 'memory'
    | 'scheduling'
    | 'agents'
    | 'skills'
    | 'files'
    | 'scratchpad'
    | 'web'
    | 'audio'
    | 'media'
    | 'discord'
    | 'telegram'
    | 'webui'
    | 'shell'
    | 'coding'
    | 'integrations'
    | 'secrets'
    | 'questions'
    | 'time'
    | 'subagents'
    | 'history'
    | 'custom_tools'
    | 'ui'
    | 'consultations'
    // Not a `ToolCategory`: the sandbox's own lookups and the dream journal.
    | 'lookup'
    | 'dreams'

export const BUILTIN_TOOLS: Record<string, BuiltinCategory> = {
    memory_search: 'memory',
    memory_read: 'memory',
    memory_write: 'memory',
    memory_list: 'memory',
    memory_follow: 'memory',
    memory_graph: 'memory',
    memory_delete: 'memory',
    memory_delete_bundle: 'memory',
    attach_artifact_to_memory: 'memory',
    detach_attachment_from_memory: 'memory',
    schedule_one_time_task: 'scheduling',
    schedule_cron_task: 'scheduling',
    list_task: 'scheduling',
    get_task_detail: 'scheduling',
    delete_task: 'scheduling',
    list_task_runs: 'scheduling',
    get_task_run_detail: 'scheduling',
    consult_agent: 'consultations',
    wait_consultations: 'consultations',
    reply_consultation: 'consultations',
    stop_consultation: 'consultations',
    list_consultations: 'consultations',
    get_consultation: 'consultations',
    list_consultable_agents: 'consultations',
    ask_consulting_agent: 'consultations',
    spawn_subagents: 'subagents',
    wait_subagents: 'subagents',
    reply_subagent: 'subagents',
    stop_subagent: 'subagents',
    list_subagents: 'subagents',
    get_subagent: 'subagents',
    ask_main_agent: 'subagents',
    list_detached_scripts: 'agents',
    get_detached_script: 'agents',
    cancel_detached_script: 'agents',
    create_skill: 'skills',
    update_skill: 'skills',
    delete_skill: 'skills',
    list_skills: 'skills',
    get_skill_details: 'skills',
    use_skill: 'skills',
    read_skill_resource: 'skills',
    execute_skill_resource: 'skills',
    list_session_files: 'files',
    read_document_file: 'files',
    read_image_file: 'files',
    send_attachment: 'files',
    get_session_file_path: 'files',
    load_path_as_session_file: 'files',
    promote_artifact: 'files',
    update_artifact: 'files',
    list_artifacts: 'files',
    load_artifact_as_session_file: 'files',
    current_time: 'time',
    convert_time: 'time',
    shift_time: 'time',
    time_between: 'time',
    write_tmp: 'scratchpad',
    append_tmp: 'scratchpad',
    read_tmp: 'scratchpad',
    recall_history: 'history',
    create_custom_tool: 'custom_tools',
    update_custom_tool: 'custom_tools',
    list_custom_tools: 'custom_tools',
    get_custom_tool: 'custom_tools',
    custom_tool_history: 'custom_tools',
    diff_custom_tool: 'custom_tools',
    rollback_custom_tool: 'custom_tools',
    delete_custom_tool: 'custom_tools',
    set_custom_tool_enabled: 'custom_tools',
    update_custom_tool_category: 'custom_tools',
    fetch: 'web',
    http_client: 'web',
    tts_generate: 'audio',
    stt_transcribe: 'audio',
    image_generate: 'media',
    discord_send_message: 'discord',
    discord_react_message: 'discord',
    discord_get_message_by_id: 'discord',
    discord_get_guild_info: 'discord',
    discord_get_channel_info: 'discord',
    discord_get_member_info: 'discord',
    telegram_send_message: 'telegram',
    telegram_react_message: 'telegram',
    telegram_get_message_by_id: 'telegram',
    webui_send_message: 'webui',
    webui_list_topics: 'webui',
    shell_exec: 'shell',
    list_integration_kinds: 'integrations',
    list_integrations: 'integrations',
    create_integration: 'integrations',
    update_integration: 'integrations',
    delete_integration: 'integrations',
    set_secret: 'secrets',
    get_secret: 'secrets',
    ask_sensitive_input: 'secrets',
    ask_question: 'questions',
    ui_render: 'ui',
    ui_describe_elements: 'ui',
    ui_examples: 'ui',
}

/** The sandbox's own tools and the dream journal: built in, but in no `ToolCategory`. */
export const UNCATEGORISED_TOOLS: Record<string, BuiltinCategory> = {
    list_tool_functions: 'lookup',
    describe_tool_function: 'lookup',
    read_dream_journal: 'dreams',
    write_dream_journal: 'dreams',
}

/** A project chat's coding tools (`projects/tools/` and `workspace_shell`), all in `coding`. */
export const PROJECT_TOOLS = [
    'read_file',
    'write_file',
    'edit_file',
    'list_files',
    'glob',
    'grep',
    'git_status',
    'git_diff',
    'git_log',
    'propose_plan',
    'workspace_shell',
    'lsp_definition',
    'lsp_diagnostics',
    'lsp_hover',
    'lsp_references',
    'lsp_symbols',
] as const

/** Addon ids: an addon's tools are named `<id>_<tool>`. */
export const ADDON_IDS = ['brave_search', 'office'] as const
export type AddonId = (typeof ADDON_IDS)[number]
