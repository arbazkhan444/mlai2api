export interface ClientField {
  label: string;
  value: string;
  secret?: boolean;
}

export interface ClientSpec {
  id: string;
  name: string;
  tagline: string;
  ready: boolean;
  note?: string;
  fields?: ClientField[];
  snippet?: { title: string; text: string };
  docs?: { label: string; href: string };
}

/**
 * Ready-to-paste config per client. Field names follow each client's own docs so users do not have to translate.
 * Values are filled from the running bridge, so copying is the whole job.
 */
export function clientSpecs({ v1, root, key, model }: { v1: string; root: string; key: string; model: string }): ClientSpec[] {
  const opencode = JSON.stringify(
    {
      providers: {
        mlai: {
          package: "@opencode/ai/providers/openai-compatible",
          settings: { baseURL: v1 },
          env: ["MLAI_KEY"],
          models: { [model]: {} },
        },
      },
    },
    null,
    2,
  );
  const shell = [
    "# PowerShell",
    "$env:OPENAI_BASE_URL = '" + v1 + "'",
    "$env:OPENAI_API_KEY = '" + key + "'",
    "",
    "# bash / zsh",
    "export OPENAI_BASE_URL='" + v1 + "'",
    "export OPENAI_API_KEY='" + key + "'",
  ].join("\n");
  const python = [
    "from openai import OpenAI",
    "",
    "client = OpenAI(base_url='" + v1 + "', api_key='" + key + "')",
    "reply = client.chat.completions.create(",
    "    model='" + model + "',",
    "    messages=[{'role': 'user', 'content': 'hello'}],",
    ")",
    "print(reply.choices[0].message.content)",
  ].join("\n");
  const curl = [
    "curl '" + v1 + "/chat/completions' \\",
    "  -H 'Content-Type: application/json' \\",
    "  -H 'Authorization: Bearer " + key + "' \\",
    "  -d '{\"model\":\"" + model + "\",\"messages\":[{\"role\":\"user\",\"content\":\"hello\"}]}'",
  ].join("\n");

  return [
    {
      id: "cline",
      name: "Cline",
      tagline: "VS Code extension",
      ready: true,
      fields: [
        { label: "API Provider", value: "OpenAI Compatible" },
        { label: "Base URL", value: v1 },
        { label: "API Key", value: key, secret: true },
        { label: "Model ID", value: model },
      ],
      docs: { label: "Cline docs", href: "https://docs.cline.bot/provider-config/openai-compatible" },
    },
    {
      id: "roo",
      name: "Roo Code",
      tagline: "VS Code extension",
      ready: true,
      fields: [
        { label: "API Provider", value: "OpenAI Compatible" },
        { label: "Base URL", value: v1 },
        { label: "API Key", value: key, secret: true },
        { label: "Model ID", value: model },
      ],
      note: "Roo Code needs tool calling; the bridge passes tools through unchanged.",
      docs: { label: "Roo Code docs", href: "https://docs.roocode.com/providers/openai-compatible" },
    },
    {
      id: "cursor",
      name: "Cursor",
      tagline: "Editor",
      ready: true,
      fields: [
        { label: "Base URL", value: v1 },
        { label: "API Key", value: key, secret: true },
        { label: "Model name", value: model },
      ],
      note: "Settings \u2192 Models \u2192 OpenAI API key, then add the alias as a custom model.",
    },
    {
      id: "opencode",
      name: "opencode",
      tagline: "Terminal agent",
      ready: true,
      snippet: { title: "opencode.jsonc", text: opencode },
      note: "Set MLAI_KEY to the API key above before starting opencode.",
      docs: { label: "opencode docs", href: "https://opencode.ai/docs/providers" },
    },
    {
      id: "curl",
      name: "curl",
      tagline: "Any shell",
      ready: true,
      snippet: { title: "bash", text: curl },
    },
    {
      id: "python",
      name: "Python",
      tagline: "openai SDK",
      ready: true,
      snippet: { title: "python", text: python },
    },
    {
      id: "env",
      name: "Shell environment",
      tagline: "OPENAI_BASE_URL + OPENAI_API_KEY",
      ready: true,
      snippet: { title: "PowerShell / bash", text: shell },
      note: "Works for tools that read the standard OpenAI environment variables.",
    },
    {
      id: "claude-code",
      name: "Claude Code",
      tagline: "Anthropic Messages API",
      ready: false,
      note:
        "Not supported yet. Claude Code speaks the Anthropic Messages API (" +
        root +
        "/v1/messages), but this bridge serves the OpenAI API (" +
        v1 +
        "/chat/completions) only. Point Claude Code at the bridge only after a Messages endpoint is added.",
      docs: { label: "Claude Code gateway docs", href: "https://code.claude.com/docs/en/llm-gateway-connect" },
    },
  ];
}
