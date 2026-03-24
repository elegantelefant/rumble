// ABOUTME: Shared chat message type used across document review and research views.
// ABOUTME: Single definition eliminates drift between views.

export type ChatMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  timestamp: string;
  citations?: string[];
};
