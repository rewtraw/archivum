import { useState, useRef, useEffect, useCallback } from "react";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import { askDocument, reembedDocument } from "../lib/api";
import type { ChatEvent } from "../lib/api";

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  sources?: { content: string; chunk_index: number; distance: number }[];
  streaming?: boolean;
}

interface AskDrawerProps {
  documentId: string;
  documentTitle: string;
  hasChunks: boolean;
  isOpen: boolean;
  onClose: () => void;
}

export function AskDrawer({
  documentId,
  documentTitle,
  hasChunks,
  isOpen,
  onClose,
}: AskDrawerProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [isEmbedding, setIsEmbedding] = useState(false);
  const [showSources, setShowSources] = useState<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen && inputRef.current) {
      setTimeout(() => inputRef.current?.focus(), 200);
    }
  }, [isOpen]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleEmbed = useCallback(async () => {
    setIsEmbedding(true);
    try {
      await reembedDocument(documentId);
      window.location.reload();
    } catch (e: any) {
      console.error("Embedding failed:", e);
    } finally {
      setIsEmbedding(false);
    }
  }, [documentId]);

  const handleSubmit = useCallback(async () => {
    const q = input.trim();
    if (!q || isStreaming) return;

    setInput("");
    setMessages((prev) => [
      ...prev,
      { role: "user", content: q },
      { role: "assistant", content: "", streaming: true },
    ]);
    setIsStreaming(true);

    try {
      await askDocument(documentId, q, (event: ChatEvent) => {
        switch (event.event) {
          case "token":
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  content: last.content + (event.data.text || ""),
                };
              }
              return updated;
            });
            break;
          case "context":
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  sources: event.data.chunks,
                };
              }
              return updated;
            });
            break;
          case "done":
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  content: event.data.full_text || last.content,
                  streaming: false,
                };
              }
              return updated;
            });
            setIsStreaming(false);
            break;
          case "error":
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  content: `Error: ${event.data.message}`,
                  streaming: false,
                };
              }
              return updated;
            });
            setIsStreaming(false);
            break;
        }
      });
    } catch (e: any) {
      setMessages((prev) => {
        const updated = [...prev];
        const last = updated[updated.length - 1];
        if (last.role === "assistant") {
          updated[updated.length - 1] = {
            ...last,
            content: `Error: ${typeof e === "string" ? e : e?.message || "Request failed"}`,
            streaming: false,
          };
        }
        return updated;
      });
      setIsStreaming(false);
    }
  }, [documentId, input, isStreaming]);

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ x: "100%", opacity: 0 }}
          animate={{ x: 0, opacity: 1 }}
          exit={{ x: "100%", opacity: 0 }}
          transition={{ type: "spring", damping: 25, stiffness: 300 }}
          className={css({
            position: "absolute",
            top: 0,
            right: 0,
            bottom: 0,
            width: "360px",
            bg: "rgba(10, 10, 12, 0.92)",
            backdropFilter: "blur(16px)",
            borderLeft: "1px solid rgba(255,255,255,0.06)",
            display: "flex",
            flexDirection: "column",
            zIndex: 70,
          })}
        >
          {/* Header */}
          <div
            className={css({
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "md",
              borderBottom: "1px solid rgba(255,255,255,0.06)",
              flexShrink: 0,
            })}
          >
            <h3
              className={css({
                fontSize: "sm",
                fontWeight: 600,
                color: "text.primary",
              })}
            >
              Ask this document
            </h3>
            <button
              onClick={onClose}
              className={css({
                bg: "transparent",
                border: "none",
                color: "text.muted",
                cursor: "pointer",
                padding: "2px",
                _hover: { color: "text.primary" },
              } as any)}
            >
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          </div>

          {/* Content */}
          {!hasChunks ? (
            <div
              className={css({
                flex: 1,
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                gap: "md",
                padding: "lg",
              })}
            >
              <p
                className={css({
                  fontSize: "sm",
                  color: "text.muted",
                  textAlign: "center",
                  lineHeight: 1.5,
                })}
              >
                This document hasn't been indexed for Q&A yet.
              </p>
              <button
                onClick={handleEmbed}
                disabled={isEmbedding}
                className={css({
                  bg: "accent.subtle",
                  color: "accent.bright",
                  border: "1px solid",
                  borderColor: "accent.dim",
                  borderRadius: "md",
                  padding: "sm",
                  paddingLeft: "md",
                  paddingRight: "md",
                  fontSize: "sm",
                  fontWeight: 500,
                  cursor: "pointer",
                  transition: "all 150ms",
                  _hover: { bg: "accent.base", color: "text.inverse" },
                  _disabled: { opacity: 0.5, cursor: "not-allowed" },
                } as any)}
              >
                {isEmbedding ? "Indexing..." : "Generate index"}
              </button>
            </div>
          ) : (
            <>
              {/* Messages */}
              <div
                ref={scrollRef}
                className={css({
                  flex: 1,
                  overflow: "auto",
                  padding: "md",
                  display: "flex",
                  flexDirection: "column",
                  gap: "md",
                })}
              >
                {messages.length === 0 && (
                  <div
                    className={css({
                      flex: 1,
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      justifyContent: "center",
                      gap: "sm",
                      opacity: 0.5,
                    })}
                  >
                    <svg
                      width="32"
                      height="32"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth={1}
                      className={css({ color: "text.muted" })}
                    >
                      <path d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
                    </svg>
                    <p className={css({ fontSize: "sm", color: "text.muted" })}>
                      Ask anything about "{documentTitle}"
                    </p>
                  </div>
                )}

                {messages.map((msg, i) => (
                  <div key={i}>
                    <div
                      className={css({
                        display: "flex",
                        justifyContent:
                          msg.role === "user" ? "flex-end" : "flex-start",
                      })}
                    >
                      <div
                        className={css({
                          maxWidth: "85%",
                          padding: "sm",
                          paddingLeft: "md",
                          paddingRight: "md",
                          borderRadius: "lg",
                          fontSize: "sm",
                          lineHeight: 1.6,
                          whiteSpace: "pre-wrap",
                          ...(msg.role === "user"
                            ? {
                                bg: "accent.subtle",
                                color: "text.primary",
                                borderBottomRightRadius: "sm",
                              }
                            : {
                                bg: "rgba(255,255,255,0.03)",
                                color: "text.secondary",
                                borderBottomLeftRadius: "sm",
                              }),
                        })}
                      >
                        {msg.content}
                        {msg.streaming && (
                          <motion.span
                            animate={{ opacity: [0.3, 1, 0.3] }}
                            transition={{ duration: 1, repeat: Infinity }}
                            className={css({
                              display: "inline-block",
                              width: "6px",
                              height: "6px",
                              borderRadius: "full",
                              bg: "accent.base",
                              marginLeft: "xs",
                              verticalAlign: "middle",
                            })}
                          />
                        )}
                      </div>
                    </div>

                    {/* Sources toggle */}
                    {msg.sources && msg.sources.length > 0 && !msg.streaming && (
                      <div className={css({ marginTop: "xs" })}>
                        <button
                          onClick={() =>
                            setShowSources(showSources === i ? null : i)
                          }
                          className={css({
                            bg: "transparent",
                            border: "none",
                            color: "text.muted",
                            fontSize: "xs",
                            cursor: "pointer",
                            padding: "2px",
                            _hover: { color: "text.secondary" },
                          } as any)}
                        >
                          {showSources === i ? "Hide" : "Show"}{" "}
                          {msg.sources.length} sources
                        </button>
                        <AnimatePresence>
                          {showSources === i && (
                            <motion.div
                              initial={{ opacity: 0, height: 0 }}
                              animate={{ opacity: 1, height: "auto" }}
                              exit={{ opacity: 0, height: 0 }}
                              className={css({
                                overflow: "hidden",
                                marginTop: "xs",
                              })}
                            >
                              {msg.sources.map((src, j) => (
                                <div
                                  key={j}
                                  className={css({
                                    padding: "sm",
                                    marginBottom: "xs",
                                    bg: "rgba(255,255,255,0.02)",
                                    borderRadius: "md",
                                    borderLeft: "2px solid",
                                    borderColor: "accent.dim",
                                    fontSize: "xs",
                                    color: "text.muted",
                                    lineHeight: 1.4,
                                    maxHeight: "100px",
                                    overflow: "hidden",
                                  })}
                                >
                                  {src.content.slice(0, 200)}
                                  {src.content.length > 200 ? "..." : ""}
                                </div>
                              ))}
                            </motion.div>
                          )}
                        </AnimatePresence>
                      </div>
                    )}
                  </div>
                ))}
              </div>

              {/* Input */}
              <div
                className={css({
                  padding: "md",
                  borderTop: "1px solid rgba(255,255,255,0.06)",
                  display: "flex",
                  gap: "sm",
                  flexShrink: 0,
                })}
              >
                <input
                  ref={inputRef}
                  type="text"
                  placeholder="Ask a question..."
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey) {
                      e.preventDefault();
                      handleSubmit();
                    }
                  }}
                  disabled={isStreaming}
                  className={css({
                    flex: 1,
                    bg: "rgba(255,255,255,0.05)",
                    border: "1px solid rgba(255,255,255,0.08)",
                    borderRadius: "md",
                    padding: "sm",
                    paddingLeft: "md",
                    color: "text.primary",
                    fontSize: "sm",
                    outline: "none",
                    _focus: { borderColor: "accent.dim" },
                    _placeholder: { color: "text.muted" },
                    _disabled: { opacity: 0.5 },
                  } as any)}
                />
                <button
                  onClick={handleSubmit}
                  disabled={!input.trim() || isStreaming}
                  className={css({
                    bg: "accent.subtle",
                    color: "accent.bright",
                    border: "1px solid",
                    borderColor: "accent.dim",
                    borderRadius: "md",
                    padding: "sm",
                    cursor: "pointer",
                    transition: "all 150ms",
                    flexShrink: 0,
                    _hover: { bg: "accent.base", color: "text.inverse" },
                    _disabled: { opacity: 0.3, cursor: "not-allowed" },
                  } as any)}
                >
                  <svg
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={2}
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M6 12L3.269 3.126A59.768 59.768 0 0121.485 12 59.77 59.77 0 013.27 20.876L5.999 12zm0 0h7.5"
                    />
                  </svg>
                </button>
              </div>
            </>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
