-- Spec 064: Workspaces were removed (specs/064-consult-agents/data-model.md). The member
-- agents' Workspace conversations, the two permissions and the notifications are removed by the
-- startup migration `migration.drop_workspaces_v1`, which runs on both backends; this drops the
-- tables. The notification columns `workspace_id` and `author` stay, unread (#82).
DELETE FROM notification WHERE category = 'workspace_message';
DROP TABLE IF EXISTS workspace_read;
DROP TABLE IF EXISTS workspace_mention;
DROP TABLE IF EXISTS workspace_entry;
DROP TABLE IF EXISTS workspace_member;
DROP TABLE IF EXISTS workspace_topic;
DROP TABLE IF EXISTS workspace;
