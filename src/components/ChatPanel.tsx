import { useState, useRef, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import ReactMarkdown from "react-markdown";
import {
  askDocument,
  askLibrary,
  reembedDocument,
  createChatSession,
  listChatSessions,
  getChatMessages,
  deleteChatSession,
  saveChatMessage,
  updateSessionTitle,
  autoTitleSession,
} from "../lib/api";
import type { ChatEvent, SourceChunk, ChatSession } from "../lib/api";

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  sources?: SourceChunk[];
  streaming?: boolean;
  toolActivity?: string;
}

interface ChatPanelProps {
  isOpen: boolean;
  onClose: () => void;
  mode: "document" | "library";
  documentId?: string;
  documentTitle?: string;
  hasChunks?: boolean;
  onChunksCreated?: () => void;
  fullHeight?: boolean;
}

const MIN_HEIGHT = 180;
const DEFAULT_HEIGHT_VH = 55;

export function ChatPanel({
  isOpen,
  onClose,
  mode,
  documentId,
  documentTitle,
  hasChunks,
  onChunksCreated,
  fullHeight = false,
}: ChatPanelProps) {
  const navigate = useNavigate();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [isEmbedding, setIsEmbedding] = useState(false);
  const [showSources, setShowSources] = useState<number | null>(null);
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);
  const [needsIndex, setNeedsIndex] = useState(false);

  // Resizable height (only used when !fullHeight)
  const [panelHeight, setPanelHeight] = useState<number | null>(null);
  const dragRef = useRef<{ startY: number; startH: number } | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  // Session management
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [showSessions, setShowSessions] = useState(false);

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

  // Drag-to-resize
  useEffect(() => {
    if (fullHeight) return;
    const onMouseMove = (e: MouseEvent) => {
      if (!dragRef.current) return;
      e.preventDefault();
      const delta = dragRef.current.startY - e.clientY;
      const containerH = panelRef.current?.parentElement?.clientHeight ?? window.innerHeight - 100;
      const newH = Math.min(containerH, Math.max(MIN_HEIGHT, dragRef.current.startH + delta));
      setPanelHeight(newH);
    };
    const onMouseUp = () => {
      dragRef.current = null;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [fullHeight]);

  const startDrag = useCallback(
    (e: React.MouseEvent) => {
      if (fullHeight) return;
      e.preventDefault();
      const currentH =
        panelHeight ?? (window.innerHeight * DEFAULT_HEIGHT_VH) / 100;
      dragRef.current = { startY: e.clientY, startH: currentH };
      document.body.style.cursor = "row-resize";
      document.body.style.userSelect = "none";
    },
    [fullHeight, panelHeight]
  );

  // Load sessions when panel opens
  useEffect(() => {
    if (!isOpen) return;
    const docId = mode === "document" ? documentId : undefined;
    listChatSessions(docId).then(setSessions).catch(() => {});
  }, [isOpen, mode, documentId]);

  const refreshSessions = useCallback(() => {
    const docId = mode === "document" ? documentId : undefined;
    listChatSessions(docId).then(setSessions).catch(() => {});
  }, [mode, documentId]);

  const loadSession = useCallback(async (sessionId: string) => {
    setActiveSessionId(sessionId);
    setShowSessions(false);
    try {
      const msgs = await getChatMessages(sessionId);
      setMessages(
        msgs.map((m) => ({
          role: m.role as "user" | "assistant",
          content: m.content,
          sources: m.sources ? JSON.parse(m.sources) : undefined,
        }))
      );
    } catch {
      setMessages([]);
    }
  }, []);

  const startNewChat = useCallback(() => {
    setActiveSessionId(null);
    setMessages([]);
    setShowSessions(false);
    setTimeout(() => inputRef.current?.focus(), 100);
  }, []);

  const handleDeleteSession = useCallback(
    async (id: string) => {
      await deleteChatSession(id);
      if (activeSessionId === id) {
        setActiveSessionId(null);
        setMessages([]);
      }
      refreshSessions();
    },
    [activeSessionId, refreshSessions]
  );

  const [embedError, setEmbedError] = useState<string | null>(null);

  const handleEmbed = useCallback(async () => {
    if (!documentId) return;
    setIsEmbedding(true);
    setEmbedError(null);
    try {
      await reembedDocument(documentId);
      onChunksCreated?.();
      setNeedsIndex(false);
    } catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message || "Indexing failed";
      setEmbedError(msg);
      console.error("Embedding failed:", e);
    } finally {
      setIsEmbedding(false);
    }
  }, [documentId]);

  const handleCopy = useCallback((content: string, idx: number) => {
    navigator.clipboard.writeText(content).then(() => {
      setCopiedIdx(idx);
      setTimeout(() => setCopiedIdx(null), 1500);
    });
  }, []);

  const handleSubmit = useCallback(async () => {
    const q = input.trim();
    if (!q || isStreaming) return;

    setInput("");

    // Create session if needed
    let sessionId = activeSessionId;
    if (!sessionId) {
      const title = q.length > 60 ? q.slice(0, 57) + "..." : q;
      const docId = mode === "document" ? documentId : undefined;
      const session = await createChatSession(title, docId);
      sessionId = session.id;
      setActiveSessionId(sessionId);
      refreshSessions();
    }

    // Persist user message
    await saveChatMessage(sessionId, "user", q).catch(() => {});

    setMessages((prev) => [
      ...prev,
      { role: "user", content: q },
      { role: "assistant", content: "", streaming: true },
    ]);
    setIsStreaming(true);

    let collectedSources: SourceChunk[] | undefined;

    const onEvent = (event: ChatEvent) => {
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
          collectedSources = event.data.chunks;
          setMessages((prev) => {
            const updated = [...prev];
            const last = updated[updated.length - 1];
            if (last.role === "assistant") {
              updated[updated.length - 1] = { ...last, sources: event.data.chunks };
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
          // Persist assistant message
          saveChatMessage(
            sessionId!,
            "assistant",
            event.data.full_text || "",
            collectedSources ? JSON.stringify(collectedSources) : undefined
          ).catch(() => {});
          // Auto-title on first exchange
          if (messages.length <= 1) {
            autoTitleSession(sessionId!, q, event.data.full_text || "")
              .then((newTitle) => {
                refreshSessions();
              })
              .catch(() => {});
          }
          setIsStreaming(false);
          break;
        case "toolCall":
          setMessages((prev) => {
            const updated = [...prev];
            const last = updated[updated.length - 1];
            if (last.role === "assistant") {
              const toolName = event.data.tool || "tool";
              const labels: Record<string, string> = {
                search_content: "Searching",
                get_document_summary: "Reading summary",
                get_section_summaries: "Scanning sections",
                keyword_search: "Keyword search",
                list_documents: "Listing documents",
                get_related_documents: "Finding related",
              };
              const label = labels[toolName] || toolName;
              const q = event.data.query ? `: ${event.data.query}` : "";
              updated[updated.length - 1] = {
                ...last,
                toolActivity: `${label}${q}`,
              };
            }
            return updated;
          });
          break;
        case "toolResult":
          setMessages((prev) => {
            const updated = [...prev];
            const last = updated[updated.length - 1];
            if (last.role === "assistant") {
              updated[updated.length - 1] = { ...last, toolActivity: undefined };
            }
            return updated;
          });
          break;
        case "error": {
          const msg = event.data.message || "Unknown error";
          const isIndexError = msg.includes("hasn't been indexed") || msg.includes("No indexed content");
          if (isIndexError && mode === "document") {
            setNeedsIndex(true);
          }
          setMessages((prev) => {
            const updated = [...prev];
            const last = updated[updated.length - 1];
            if (last.role === "assistant") {
              updated[updated.length - 1] = {
                ...last,
                content: isIndexError ? "This document needs to be indexed before I can answer questions about it." : `Error: ${msg}`,
                streaming: false,
              };
            }
            return updated;
          });
          setIsStreaming(false);
          break;
        }
      }
    };

    try {
      if (mode === "document" && documentId) {
        await askDocument(documentId, q, onEvent);
      } else {
        await askLibrary(q, onEvent);
      }
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
  }, [input, isStreaming, activeSessionId, mode, documentId, refreshSessions]);

  const title =
    mode === "document" ? `Ask: ${documentTitle || "Document"}` : "Library Chat";

  const heightStyle = fullHeight
    ? { top: 0, bottom: 0, height: "auto" as const }
    : {
        bottom: 0,
        height: panelHeight ? `${panelHeight}px` : `${DEFAULT_HEIGHT_VH}vh`,
        maxHeight: "100%",
      };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          ref={panelRef}
          initial={fullHeight ? { opacity: 0 } : { y: "100%", opacity: 0.5 }}
          animate={fullHeight ? { opacity: 1 } : { y: 0, opacity: 1 }}
          exit={fullHeight ? { opacity: 0 } : { y: "100%", opacity: 0 }}
          transition={{ type: "spring", damping: 30, stiffness: 350 }}
          style={fullHeight ? undefined : { height: heightStyle.height }}
          className={css({
            position: "absolute",
            left: 0,
            right: 0,
            ...heightStyle,
            bg: fullHeight ? "bg.base" : "rgba(10, 10, 12, 0.95)",
            backdropFilter: fullHeight ? undefined : "blur(20px)",
            borderTop: fullHeight ? "none" : "1px solid rgba(255,255,255,0.08)",
            display: "flex",
            flexDirection: "column",
            zIndex: 100,
            borderTopLeftRadius: fullHeight ? "0" : "xl",
            borderTopRightRadius: fullHeight ? "0" : "xl",
          })}
        >
          {/* Drag handle + header */}
          <div
            className={css({
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              flexShrink: 0,
            })}
          >
            {!fullHeight && (
              <div
                onMouseDown={startDrag}
                className={css({
                  width: "100%",
                  display: "flex",
                  justifyContent: "center",
                  paddingTop: "6px",
                  paddingBottom: "2px",
                  cursor: "row-resize",
                })}
              >
                <div
                  className={css({
                    width: "36px",
                    height: "4px",
                    borderRadius: "full",
                    bg: "rgba(255,255,255,0.15)",
                    transition: "background 150ms",
                    _hover: { bg: "rgba(255,255,255,0.3)" },
                  } as any)}
                />
              </div>
            )}
            <div
              className={css({
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                width: "100%",
                padding: fullHeight ? "12px 16px" : "8px 16px 10px",
                paddingTop: fullHeight ? "48px" : undefined,
                borderBottom: "1px solid rgba(255,255,255,0.06)",
              })}
            >
              <div className={css({ display: "flex", alignItems: "center", gap: "sm" })}>
                {/* Session list toggle */}
                <button
                  onClick={() => setShowSessions(!showSessions)}
                  className={css({
                    bg: "transparent",
                    border: "none",
                    color: showSessions ? "accent.base" : "text.muted",
                    cursor: "pointer",
                    padding: "4px",
                    borderRadius: "sm",
                    _hover: { color: "text.primary" },
                  } as any)}
                  title="Chat history"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                    <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                </button>
                <h3
                  className={css({
                    fontSize: "sm",
                    fontWeight: 600,
                    color: "text.primary",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    maxWidth: "280px",
                  })}
                >
                  {title}
                </h3>
              </div>
              <div className={css({ display: "flex", alignItems: "center", gap: "xs" })}>
                <button
                  onClick={startNewChat}
                  className={css({
                    bg: "transparent",
                    border: "none",
                    color: "text.muted",
                    cursor: "pointer",
                    padding: "4px",
                    fontSize: "xs",
                    _hover: { color: "text.primary" },
                  } as any)}
                  title="New chat"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                    <path d="M12 4v16m8-8H4" />
                  </svg>
                </button>
                {!fullHeight && (
                  <button
                    onClick={onClose}
                    className={css({
                      bg: "transparent",
                      border: "none",
                      color: "text.muted",
                      cursor: "pointer",
                      padding: "4px",
                      _hover: { color: "text.primary" },
                    } as any)}
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                      <path d="M19 9l-7 7-7-7" />
                    </svg>
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* Body */}
          <div className={css({ flex: 1, overflow: "hidden", display: "flex" })}>
            {/* Session sidebar */}
            <AnimatePresence>
              {showSessions && (
                <motion.div
                  initial={{ width: 0, opacity: 0 }}
                  animate={{ width: 220, opacity: 1 }}
                  exit={{ width: 0, opacity: 0 }}
                  transition={{ type: "spring", damping: 25, stiffness: 300 }}
                  className={css({
                    overflow: "hidden",
                    borderRight: "1px solid rgba(255,255,255,0.06)",
                    flexShrink: 0,
                    display: "flex",
                    flexDirection: "column",
                  })}
                >
                  <div className={css({ flex: 1, overflow: "auto", padding: "sm" })}>
                    {sessions.length === 0 ? (
                      <p
                        className={css({
                          fontSize: "xs",
                          color: "text.muted",
                          textAlign: "center",
                          paddingTop: "lg",
                        })}
                      >
                        No conversations yet
                      </p>
                    ) : (
                      sessions.map((s) => (
                        <div
                          key={s.id}
                          onClick={() => loadSession(s.id)}
                          className={css({
                            display: "flex",
                            alignItems: "center",
                            gap: "xs",
                            padding: "sm",
                            borderRadius: "md",
                            cursor: "pointer",
                            bg:
                              activeSessionId === s.id
                                ? "rgba(255,255,255,0.06)"
                                : "transparent",
                            transition: "all 120ms",
                            _hover: { bg: "rgba(255,255,255,0.04)" },
                          })}
                        >
                          <div className={css({ flex: 1, minWidth: 0 })}>
                            <p
                              className={css({
                                fontSize: "xs",
                                color:
                                  activeSessionId === s.id
                                    ? "text.primary"
                                    : "text.secondary",
                                overflow: "hidden",
                                textOverflow: "ellipsis",
                                whiteSpace: "nowrap",
                              })}
                            >
                              {s.title}
                            </p>
                          </div>
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDeleteSession(s.id);
                            }}
                            className={css({
                              bg: "transparent",
                              border: "none",
                              color: "text.muted",
                              cursor: "pointer",
                              padding: "2px",
                              opacity: 0,
                              transition: "opacity 120ms",
                              flexShrink: 0,
                              _groupHover: { opacity: 1 },
                              _hover: { color: "status.error" },
                            } as any)}
                          >
                            <svg
                              width="12"
                              height="12"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth={2}
                            >
                              <path d="M6 18L18 6M6 6l12 12" />
                            </svg>
                          </button>
                        </div>
                      ))
                    )}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Main chat area */}
            <div className={css({ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" })}>
              {mode === "document" && hasChunks === false ? (
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
                  <p className={css({ fontSize: "sm", color: "text.muted", textAlign: "center" })}>
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
                      _hover: { bg: "accent.base", color: "text.inverse" },
                      _disabled: { opacity: 0.5, cursor: "not-allowed" },
                    } as any)}
                  >
                    {isEmbedding ? "Indexing..." : "Generate index"}
                  </button>
                  {embedError && (
                    <p className={css({ fontSize: "xs", color: "status.error", textAlign: "center", maxWidth: "360px" })}>
                      {embedError}
                    </p>
                  )}
                </div>
              ) : (
                <>
                  {/* Messages */}
                  <div
                    ref={scrollRef}
                    className={`selectable ${css({
                      flex: 1,
                      overflow: "auto",
                      padding: "md",
                      paddingBottom: "sm",
                      display: "flex",
                      flexDirection: "column",
                      gap: "md",
                    })}`}
                  >
                    {messages.length === 0 && (
                      <div
                        className={css({
                          flex: 1,
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "center",
                          opacity: 0.4,
                        })}
                      >
                        <p className={css({ fontSize: "sm", color: "text.muted" })}>
                          {mode === "library"
                            ? "Ask anything across your entire library"
                            : `Ask anything about "${documentTitle}"`}
                        </p>
                      </div>
                    )}

                    {messages.map((msg, i) => (
                      <div key={i}>
                        <div
                          className={css({
                            display: "flex",
                            justifyContent: msg.role === "user" ? "flex-end" : "flex-start",
                          })}
                        >
                          <div
                            className={css({
                              maxWidth: "80%",
                              position: "relative",
                              ...(msg.role === "user"
                                ? {
                                    padding: "8px 12px",
                                    borderRadius: "lg",
                                    fontSize: "sm",
                                    lineHeight: 1.6,
                                    whiteSpace: "pre-wrap",
                                    bg: "accent.subtle",
                                    color: "text.primary",
                                    borderBottomRightRadius: "sm",
                                  }
                                : {
                                    padding: "8px 12px",
                                    borderRadius: "lg",
                                    bg: "rgba(255,255,255,0.03)",
                                    borderBottomLeftRadius: "sm",
                                    "& > button[title='Copy raw markdown']": { opacity: 0 },
                                    "&:hover > button[title='Copy raw markdown']": { opacity: 1 },
                                  }),
                            })}
                          >
                            {msg.role === "user" ? (
                              msg.content
                            ) : (
                              <div
                                className={css({
                                  fontSize: "sm",
                                  lineHeight: 1.7,
                                  color: "text.secondary",
                                  "& p": { marginBottom: "sm" },
                                  "& p:last-child": { marginBottom: 0 },
                                  "& h1, & h2, & h3, & h4": {
                                    color: "text.primary",
                                    fontWeight: 600,
                                    marginTop: "md",
                                    marginBottom: "xs",
                                  },
                                  "& h1": { fontSize: "lg" },
                                  "& h2": { fontSize: "md" },
                                  "& h3": { fontSize: "sm", fontWeight: 600 },
                                  "& ul, & ol": { paddingLeft: "lg", marginBottom: "sm" },
                                  "& li": { marginBottom: "2px" },
                                  "& code": {
                                    fontFamily: "mono",
                                    fontSize: "xs",
                                    bg: "rgba(255,255,255,0.06)",
                                    padding: "1px 5px",
                                    borderRadius: "sm",
                                  },
                                  "& pre": {
                                    bg: "rgba(255,255,255,0.04)",
                                    border: "1px solid rgba(255,255,255,0.06)",
                                    borderRadius: "md",
                                    padding: "sm",
                                    overflow: "auto",
                                    marginBottom: "sm",
                                  },
                                  "& pre code": { bg: "transparent", padding: 0 },
                                  "& blockquote": {
                                    borderLeft: "3px solid",
                                    borderColor: "accent.dim",
                                    paddingLeft: "sm",
                                    color: "text.muted",
                                    fontStyle: "italic",
                                    margin: "sm 0",
                                  },
                                  "& a": { color: "accent.base", textDecoration: "none" },
                                  "& hr": { border: "none", borderTop: "1px solid rgba(255,255,255,0.06)", margin: "md 0" },
                                  "& table": { width: "100%", borderCollapse: "collapse", marginBottom: "sm", fontSize: "xs" },
                                  "& th, & td": { border: "1px solid rgba(255,255,255,0.08)", padding: "xs sm", textAlign: "left" },
                                  "& th": { bg: "rgba(255,255,255,0.04)", fontWeight: 600 },
                                  "& strong": { color: "text.primary", fontWeight: 600 },
                                  "& em": { fontStyle: "italic" },
                                } as any)}
                              >
                                <ReactMarkdown>{msg.content}</ReactMarkdown>
                              </div>
                            )}
                            {msg.toolActivity && (
                              <motion.div
                                initial={{ opacity: 0, height: 0 }}
                                animate={{ opacity: 1, height: "auto" }}
                                exit={{ opacity: 0, height: 0 }}
                                className={css({
                                  fontSize: "xs",
                                  color: "text.muted",
                                  fontStyle: "italic",
                                  display: "flex",
                                  alignItems: "center",
                                  gap: "xs",
                                  marginTop: "xs",
                                })}
                              >
                                <motion.span
                                  animate={{ opacity: [0.4, 1, 0.4] }}
                                  transition={{ duration: 1.2, repeat: Infinity }}
                                >
                                  {msg.toolActivity}
                                </motion.span>
                              </motion.div>
                            )}
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
                            {/* Copy button for assistant messages */}
                            {msg.role === "assistant" && msg.content && !msg.streaming && (
                              <button
                                onClick={() => handleCopy(msg.content, i)}
                                title="Copy raw markdown"
                                className={css({
                                  position: "absolute",
                                  top: "6px",
                                  right: "6px",
                                  bg: "transparent",
                                  border: "none",
                                  color: copiedIdx === i ? "accent.base" : "text.muted",
                                  cursor: "pointer",
                                  padding: "3px",
                                  borderRadius: "sm",
                                  transition: "all 150ms",
                                  _hover: { color: "text.primary" },
                                } as any)}
                              >
                                {copiedIdx === i ? (
                                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                                    <path d="M20 6L9 17l-5-5" />
                                  </svg>
                                ) : (
                                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
                                  </svg>
                                )}
                              </button>
                            )}
                          </div>
                        </div>

                        {/* Sources */}
                        {msg.sources && msg.sources.length > 0 && !msg.streaming && (
                          <div className={css({ marginTop: "4px", marginLeft: "4px" })}>
                            <button
                              onClick={() => setShowSources(showSources === i ? null : i)}
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
                              {showSources === i ? "Hide" : "Show"} {msg.sources.length}{" "}
                              sources
                            </button>
                            <AnimatePresence>
                              {showSources === i && (
                                <motion.div
                                  initial={{ opacity: 0, height: 0 }}
                                  animate={{ opacity: 1, height: "auto" }}
                                  exit={{ opacity: 0, height: 0 }}
                                  className={css({ overflow: "hidden", marginTop: "4px" })}
                                >
                                  {msg.sources.map((src, j) => (
                                    <div
                                      key={j}
                                      className={css({
                                        padding: "6px 10px",
                                        marginBottom: "4px",
                                        bg: "rgba(255,255,255,0.02)",
                                        borderRadius: "md",
                                        borderLeft: "2px solid",
                                        borderColor: "accent.dim",
                                        fontSize: "xs",
                                        lineHeight: 1.4,
                                      })}
                                    >
                                      {src.document_title && (
                                        <button
                                          onClick={() =>
                                            navigate(`/document/${src.document_id}`)
                                          }
                                          className={css({
                                            display: "block",
                                            bg: "transparent",
                                            border: "none",
                                            color: "accent.base",
                                            fontSize: "xs",
                                            fontWeight: 500,
                                            cursor: "pointer",
                                            padding: 0,
                                            marginBottom: "2px",
                                            _hover: { textDecoration: "underline" },
                                          } as any)}
                                        >
                                          {src.document_title}
                                        </button>
                                      )}
                                      <span className={css({ color: "text.muted" })}>
                                        {src.content.slice(0, 200)}
                                        {src.content.length > 200 ? "..." : ""}
                                      </span>
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

                  {/* Input or Index prompt */}
                  {needsIndex ? (
                    <div
                      className={css({
                        padding: "md",
                        borderTop: "1px solid rgba(255,255,255,0.06)",
                        display: "flex",
                        flexDirection: "column",
                        alignItems: "center",
                        gap: "sm",
                        flexShrink: 0,
                      })}
                    >
                      <button
                        onClick={() => { setNeedsIndex(false); handleEmbed(); }}
                        disabled={isEmbedding}
                        className={css({
                          bg: "accent.subtle",
                          color: "accent.bright",
                          border: "1px solid",
                          borderColor: "accent.dim",
                          borderRadius: "md",
                          padding: "sm",
                          paddingLeft: "lg",
                          paddingRight: "lg",
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
                      {embedError && (
                        <p className={css({ fontSize: "xs", color: "status.error", textAlign: "center", maxWidth: "360px" })}>
                          {embedError}
                        </p>
                      )}
                    </div>
                  ) : (
                  <div
                    className={css({
                      padding: "10px 16px 14px",
                      borderTop: "1px solid rgba(255,255,255,0.06)",
                      display: "flex",
                      gap: "sm",
                      flexShrink: 0,
                    })}
                  >
                    <input
                      ref={inputRef}
                      type="text"
                      placeholder={
                        mode === "library"
                          ? "Ask across your library..."
                          : "Ask a question..."
                      }
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
                        padding: "8px 12px",
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
                        padding: "8px",
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
                  )}
                </>
              )}
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
