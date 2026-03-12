import { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion } from "framer-motion";
import ReactMarkdown from "react-markdown";
import {
  getDocument,
  getDocumentMarkdown,
  deleteDocument,
  formatFileSize,
  formatDate,
} from "../lib/api";
import type { Document } from "../lib/api";

export function DocumentView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [doc, setDoc] = useState<Document | null>(null);
  const [markdown, setMarkdown] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<"read" | "info">("read");

  useEffect(() => {
    if (!id) return;
    async function load() {
      try {
        const [document, md] = await Promise.all([
          getDocument(id!),
          getDocumentMarkdown(id!).catch(() => null),
        ]);
        setDoc(document);
        setMarkdown(md);
      } catch (e) {
        console.error("Failed to load document:", e);
      } finally {
        setLoading(false);
      }
    }
    load();
  }, [id]);

  const handleDelete = async () => {
    if (!id) return;
    await deleteDocument(id);
    navigate("/");
  };

  if (loading) {
    return (
      <div
        className={css({
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "text.muted",
        })}
      >
        <motion.div animate={{ opacity: [0.3, 1, 0.3] }} transition={{ duration: 1.5, repeat: Infinity }}>
          Loading...
        </motion.div>
      </div>
    );
  }

  if (!doc) {
    return (
      <div
        className={css({
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "text.muted",
        })}
      >
        Document not found
      </div>
    );
  }

  return (
    <div
      className={css({
        flex: 1,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      })}
    >
      {/* Header */}
      <header
        className={css({
          padding: "lg",
          paddingTop: "48px",
          paddingBottom: "md",
          borderBottom: "1px solid",
          borderColor: "border.subtle",
          WebkitAppRegion: "drag",
        } as any)}
      >
        <div
          className={css({
            display: "flex",
            alignItems: "flex-start",
            justifyContent: "space-between",
            WebkitAppRegion: "no-drag",
          } as any)}
        >
          <div>
            <button
              onClick={() => navigate(-1)}
              className={css({
                bg: "transparent",
                border: "none",
                color: "text.muted",
                fontSize: "sm",
                cursor: "pointer",
                padding: 0,
                marginBottom: "xs",
                display: "flex",
                alignItems: "center",
                gap: "xs",
                _hover: { color: "text.primary" },
              } as any)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
              </svg>
              Back
            </button>
            <h1
              className={css({
                fontSize: "xl",
                fontWeight: 700,
                letterSpacing: "-0.02em",
                color: "text.primary",
              })}
            >
              {doc.title || "Untitled"}
            </h1>
            <p className={css({ fontSize: "sm", color: "text.secondary", marginTop: "2px" })}>
              {doc.author || "Unknown author"}
            </p>
          </div>

          <button
            onClick={handleDelete}
            className={css({
              bg: "transparent",
              border: "1px solid",
              borderColor: "border.subtle",
              color: "text.muted",
              borderRadius: "md",
              padding: "xs",
              paddingLeft: "sm",
              paddingRight: "sm",
              fontSize: "xs",
              cursor: "pointer",
              transition: "all 150ms",
              _hover: {
                borderColor: "status.error",
                color: "status.error",
              },
            } as any)}
          >
            Delete
          </button>
        </div>

        {/* Tabs */}
        <div
          className={css({
            display: "flex",
            gap: "md",
            marginTop: "md",
            WebkitAppRegion: "no-drag",
          } as any)}
        >
          {(["read", "info"] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={css({
                bg: "transparent",
                border: "none",
                color: activeTab === tab ? "text.primary" : "text.muted",
                fontSize: "sm",
                fontWeight: 500,
                cursor: "pointer",
                padding: 0,
                paddingBottom: "xs",
                borderBottom: "2px solid",
                borderColor: activeTab === tab ? "accent.base" : "transparent",
                transition: "all 150ms",
                textTransform: "capitalize",
                _hover: {
                  color: "text.primary",
                },
              } as any)}
            >
              {tab}
            </button>
          ))}
        </div>
      </header>

      {/* Content */}
      <div
        className={css({
          flex: 1,
          overflow: "auto",
          padding: "lg",
        })}
      >
        {activeTab === "read" ? (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className={`selectable ${css({
              maxWidth: "720px",
              margin: "0 auto",
              fontFamily: "reading",
              fontSize: "md",
              lineHeight: 1.8,
              color: "text.primary",
              "& h1": {
                fontSize: "2xl",
                fontWeight: 700,
                marginTop: "xl",
                marginBottom: "md",
                fontFamily: "heading",
                letterSpacing: "-0.02em",
              },
              "& h2": {
                fontSize: "xl",
                fontWeight: 600,
                marginTop: "xl",
                marginBottom: "sm",
                fontFamily: "heading",
              },
              "& h3": {
                fontSize: "lg",
                fontWeight: 600,
                marginTop: "lg",
                marginBottom: "sm",
                fontFamily: "heading",
              },
              "& p": {
                marginBottom: "md",
              },
              "& ul, & ol": {
                paddingLeft: "lg",
                marginBottom: "md",
              },
              "& li": {
                marginBottom: "xs",
              },
              "& blockquote": {
                borderLeft: "3px solid",
                borderColor: "accent.dim",
                paddingLeft: "md",
                color: "text.secondary",
                fontStyle: "italic",
                margin: "md 0",
              },
              "& code": {
                fontFamily: "mono",
                fontSize: "sm",
                bg: "bg.elevated",
                padding: "2px 6px",
                borderRadius: "sm",
              },
              "& pre": {
                bg: "bg.surface",
                border: "1px solid",
                borderColor: "border.subtle",
                borderRadius: "md",
                padding: "md",
                overflow: "auto",
                marginBottom: "md",
              },
              "& pre code": {
                bg: "transparent",
                padding: 0,
              },
              "& hr": {
                border: "none",
                borderTop: "1px solid",
                borderColor: "border.subtle",
                margin: "xl 0",
              },
              "& a": {
                color: "accent.base",
                textDecoration: "none",
                _hover: {
                  textDecoration: "underline",
                },
              },
              "& table": {
                width: "100%",
                borderCollapse: "collapse",
                marginBottom: "md",
              },
              "& th, & td": {
                border: "1px solid",
                borderColor: "border.subtle",
                padding: "sm",
                textAlign: "left",
                fontSize: "sm",
              },
              "& th": {
                bg: "bg.surface",
                fontWeight: 600,
              },
            } as any)}`}
          >
            {markdown ? (
              <ReactMarkdown>{markdown}</ReactMarkdown>
            ) : doc.status === "processing" ? (
              <div className={css({ color: "text.muted", textAlign: "center", paddingTop: "3xl" })}>
                <motion.div animate={{ opacity: [0.3, 1, 0.3] }} transition={{ duration: 1.5, repeat: Infinity }}>
                  Processing with Claude...
                </motion.div>
              </div>
            ) : (
              <div className={css({ color: "text.muted", textAlign: "center", paddingTop: "3xl" })}>
                No content available
              </div>
            )}
          </motion.div>
        ) : (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className={css({
              maxWidth: "560px",
              display: "flex",
              flexDirection: "column",
              gap: "md",
            })}
          >
            {[
              ["Format", doc.original_format.toUpperCase()],
              ["Size", formatFileSize(doc.file_size)],
              ["Language", doc.language],
              ["ISBN", doc.isbn],
              ["Publisher", doc.publisher],
              ["Published", doc.published_date],
              ["Pages", doc.page_count?.toString()],
              ["Imported", formatDate(doc.imported_at)],
              ["Status", doc.status],
              ["Hash", doc.file_hash],
            ]
              .filter(([, val]) => val)
              .map(([label, value]) => (
                <div
                  key={label}
                  className={css({
                    display: "flex",
                    alignItems: "baseline",
                    gap: "md",
                    padding: "sm 0",
                    borderBottom: "1px solid",
                    borderColor: "border.subtle",
                  })}
                >
                  <span
                    className={css({
                      fontSize: "sm",
                      color: "text.muted",
                      width: "100px",
                      flexShrink: 0,
                    })}
                  >
                    {label}
                  </span>
                  <span
                    className={css({
                      fontSize: "sm",
                      color: "text.primary",
                      fontFamily: label === "Hash" ? "mono" : "body",
                      wordBreak: "break-all",
                    })}
                  >
                    {value}
                  </span>
                </div>
              ))}

            {doc.description && (
              <div className={css({ marginTop: "md" })}>
                <span className={css({ fontSize: "sm", color: "text.muted" })}>Description</span>
                <p
                  className={css({
                    fontSize: "sm",
                    color: "text.secondary",
                    marginTop: "xs",
                    lineHeight: 1.6,
                  })}
                >
                  {doc.description}
                </p>
              </div>
            )}

            {doc.tags.length > 0 && (
              <div className={css({ marginTop: "sm" })}>
                <span className={css({ fontSize: "sm", color: "text.muted" })}>Tags</span>
                <div
                  className={css({
                    display: "flex",
                    flexWrap: "wrap",
                    gap: "xs",
                    marginTop: "xs",
                  })}
                >
                  {doc.tags.map((tag) => (
                    <span
                      key={tag}
                      className={css({
                        fontSize: "xs",
                        color: "accent.base",
                        bg: "accent.subtle",
                        borderRadius: "full",
                        padding: "2px 10px",
                      })}
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </motion.div>
        )}
      </div>
    </div>
  );
}
