import { z } from "zod";
import { bool, defineComponent, label } from "./define";

// A `file` is a file's name in this conversation's session files (what `list_session_files`
// returns). The WebUI resolves it at draw time; nothing is copied into the interface, and nothing
// is loaded from an address.
const file = z.string().min(1).max(255).describe("a session file's name");

export const media = [
  defineComponent({
    name: "Link",
    group: "media",
    description:
      "A link the person can follow: href (https, http or mailto only) opens in a new tab; file downloads a conversation file. The host is shown beside text that does not name it.",
    props: {
      text: label,
      href: z.string().max(2048).optional().describe("https:, http: or mailto: address"),
      file: file.optional().describe("download this conversation file instead of following href"),
    },
    children: "none",
    events: [],
    example: { props: { text: "Docs", href: "https://example.com/docs" } },
  }),
  defineComponent({
    name: "Image",
    group: "media",
    description:
      "A picture from this conversation's files (file) or a small inline graphic (src: data:image/…). External addresses are not allowed. alt is required.",
    props: {
      file: file.optional(),
      src: z.string().max(200_000).regex(/^data:image\//, "must be a data:image/ URL. External addresses are not loaded; use file for a file of this conversation.").optional().describe("an inline data:image/ URL"),
      alt: label,
      width: z.number().min(16).max(4000).optional().describe("max width in px"),
      fit: z.enum(["contain", "cover"]).optional(),
    },
    children: "none",
    events: ["press"],
    example: { props: { file: "chart.png", alt: "Sales by month" } },
  }),
  defineComponent({
    name: "Audio",
    group: "media",
    description: "A player for an audio file of this conversation. Never plays by itself.",
    props: { file, title: label.optional(), transcript: z.string().max(20_000).optional() },
    children: "none",
    events: [],
    example: { props: { file: "voice-note.mp3", title: "Voice note" } },
  }),
  defineComponent({
    name: "Video",
    group: "media",
    description: "A player for a video file of this conversation. Never plays by itself.",
    props: { file, title: label.optional(), poster: file.optional().describe("a session image shown before playing"), height: z.number().min(60).max(1200).optional() },
    children: "none",
    events: [],
    example: { props: { file: "clip.mp4", title: "Demo" } },
  }),
  defineComponent({
    name: "FileCard",
    group: "media",
    description: "A file of this conversation with Open and Download. preview opens the standard preview (PDF, CSV, text, images).",
    props: { file, title: label.optional(), preview: bool.optional() },
    children: "none",
    events: ["press"],
    example: { props: { file: "report.pdf", title: "Quarterly report", preview: true } },
  }),
  defineComponent({
    name: "Gallery",
    group: "media",
    description: "Several images of this conversation in a grid; pressing one opens it larger and emits press with its file and index.",
    props: {
      files: z.array(file).min(1).max(200),
      cols: z.number().int().min(1).max(6).optional(),
      captions: z.array(z.string().max(300)).max(200).optional(),
    },
    children: "none",
    events: ["press"],
    example: { props: { files: ["a.png", "b.png"], cols: 2 } },
  }),
];
