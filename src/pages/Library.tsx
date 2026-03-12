import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import { listDocuments, getStats, formatFileSize, formatDate } from "../lib/api";
import type { Document, LibraryStats } from "../lib/api";

export function Library() {
  const [documents, setDocuments] = useState<Document[]>([]);
  const [stats, setStats] = useState<LibraryStats | null>(null);
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();

  useEffect(() => {
    async function load() {
      try {
        const [docs, libraryStats] = await Promise.all([
          listDocuments(),
          getStats(),
        ]);
        setDocuments(docs);
        setStats(libraryStats);
      } catch (e) {
        console.error("Failed to load library:", e);
      } finally {
        setLoading(false);
      }
    }
    load();
  }, []);

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
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          WebkitAppRegion: "drag",
        } as any)}
      >
        <div className={css({ WebkitAppRegion: "no-drag" } as any)}>
          <h1
            className={css({
              fontSize: "2xl",
              fontWeight: 700,
              letterSpacing: "-0.03em",
              color: "text.primary",
            })}
          >
            Library
          </h1>
          {stats && (
            <p
              className={css({
                fontSize: "sm",
                color: "text.muted",
                marginTop: "xs",
              })}
            >
              {stats.total_documents} documents &middot;{" "}
              {formatFileSize(stats.total_size_bytes)}
            </p>
          )}
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
        {loading ? (
          <div
            className={css({
              display: "flex",
              justifyContent: "center",
              alignItems: "center",
              height: "100%",
              color: "text.muted",
            })}
          >
            <motion.div
              animate={{ opacity: [0.3, 1, 0.3] }}
              transition={{ duration: 1.5, repeat: Infinity }}
            >
              Loading...
            </motion.div>
          </div>
        ) : documents.length === 0 ? (
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className={css({
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              gap: "md",
            })}
          >
            <div
              className={css({
                fontSize: "3xl",
                opacity: 0.2,
              })}
            >
              <svg
                width="64"
                height="64"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={1}
              >
                <path d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
              </svg>
            </div>
            <p className={css({ color: "text.muted", fontSize: "md" })}>
              Your archive is empty
            </p>
            <button
              onClick={() => navigate("/import")}
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
                transition: "all 200ms",
                _hover: {
                  bg: "accent.base",
                  color: "text.inverse",
                },
              } as any)}
            >
              Import your first document
            </button>
          </motion.div>
        ) : (
          <div
            className={css({
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))",
              gap: "md",
            })}
          >
            <AnimatePresence>
              {documents.map((doc, i) => (
                <motion.div
                  key={doc.id}
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: i * 0.03 }}
                  onClick={() => navigate(`/document/${doc.id}`)}
                  className={css({
                    bg: "bg.surface",
                    borderRadius: "lg",
                    border: "1px solid",
                    borderColor: "border.subtle",
                    overflow: "hidden",
                    cursor: "pointer",
                    transition: "all 200ms",
                    _hover: {
                      borderColor: "border.strong",
                      transform: "translateY(-2px)",
                    },
                  } as any)}
                >
                  {/* Cover placeholder */}
                  <div
                    className={css({
                      height: "240px",
                      bg: "bg.elevated",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "3xl",
                      color: "text.muted",
                      position: "relative",
                      overflow: "hidden",
                    })}
                  >
                    <div
                      className={css({
                        position: "absolute",
                        inset: 0,
                        background: `linear-gradient(135deg, rgba(212, 160, 23, 0.05), transparent)`,
                      })}
                    />
                    <span className={css({ opacity: 0.3, fontSize: "sm", fontFamily: "mono", textTransform: "uppercase" })}>
                      {doc.original_format}
                    </span>
                  </div>

                  {/* Info */}
                  <div className={css({ padding: "md" })}>
                    <h3
                      className={css({
                        fontSize: "sm",
                        fontWeight: 600,
                        color: "text.primary",
                        lineHeight: 1.3,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        display: "-webkit-box",
                        WebkitLineClamp: 2,
                        WebkitBoxOrient: "vertical",
                      } as any)}
                    >
                      {doc.title || "Untitled"}
                    </h3>
                    <p
                      className={css({
                        fontSize: "xs",
                        color: "text.secondary",
                        marginTop: "xs",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      })}
                    >
                      {doc.author || "Unknown author"}
                    </p>
                    <div
                      className={css({
                        display: "flex",
                        alignItems: "center",
                        gap: "xs",
                        marginTop: "sm",
                      })}
                    >
                      <span
                        className={css({
                          fontSize: "xs",
                          color: "text.muted",
                          fontFamily: "mono",
                        })}
                      >
                        {formatFileSize(doc.file_size)}
                      </span>
                      {doc.status === "processing" && (
                        <motion.span
                          animate={{ opacity: [0.4, 1, 0.4] }}
                          transition={{ duration: 1.5, repeat: Infinity }}
                          className={css({
                            fontSize: "xs",
                            color: "status.warning",
                          })}
                        >
                          Processing...
                        </motion.span>
                      )}
                      {doc.status === "error" && (
                        <span
                          className={css({
                            fontSize: "xs",
                            color: "status.error",
                          })}
                        >
                          Error
                        </span>
                      )}
                    </div>
                  </div>
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>
    </div>
  );
}
