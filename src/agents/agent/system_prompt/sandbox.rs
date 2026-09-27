use crate::agents::tools::ToolExposure;

/// The mode-specific `SANDBOX.md` briefing, or `None` while the sandbox is off.
pub fn sandbox_md(exposure: &ToolExposure, tools_timeout: &str) -> Option<String> {
    match exposure {
        ToolExposure::Direct => None,
        ToolExposure::SandboxAdditive => Some(format!(
            r#"# SANDBOX.md - Python Sandbox

You can run Python with the `execute_python` tool. Use it whenever an answer depends on exact
computation — arithmetic, dates, parsing, sorting, regex, JSON reshaping — instead of working it
out in your head. The value of the script's last expression comes back as `result`; `print()`
output comes back as `stdout`.

The sandbox is isolated: no files, network, environment or OS, and your other tools are NOT
callable from inside a script — call them directly as usual. A script must finish within
{tools_timeout}. If a script fails you receive the exception and a traceback; fix it and run again."#
        )),
        ToolExposure::CodeModeExclusive => Some(format!(
            r#"# SANDBOX.md - Code Mode

Code mode is on: your tools are not offered to you directly. You reach every one of them by
writing a Python script for `execute_python`, where each tool is a plain function taking
keyword arguments and returning plain data.

- Not sure what exists? Call `list_tool_functions` first, then `describe_tool_function` for
  parameters and an example. Inside a script the same information is available from
  `list_tools()` and `describe_tool("name")`.
- Prefer ONE script that loops, filters and aggregates over many small round-trips; only the
  script's final `result` (and `stdout`) enters your context.
- A tool error is raised inside the script as `RuntimeError("<tool>: ...")` — catch it if partial
  results are acceptable; uncaught, it ends the run and you get the traceback.
- The whole script, including every tool call it makes, must finish within {tools_timeout}.
- Return only what you need. Everything you return or print is delivered to you verbatim.
- `think` is still available directly for reasoning."#
        )),
    }
}
