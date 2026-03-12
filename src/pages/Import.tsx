import { useState, useCallback, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import { open } from "@tauri-apps/plugin-dialog";
import { importFiles, getTasks, getSettings, deleteTask, clearFinishedTasks } from "../lib/api";
import type { ImportResult, Task } from "../lib/api";

export function Import() {
  const [results, setResults] = useState<ImportResult[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasApiKey, setHasApiKey] = useState<boolean | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval>>(undefined);
  const navigate = useNavigate();

  // Check for API key on mount
  useEffect(() => {
    getSettings().then((s) => setHasApiKey(s.has_api_key)).catch(() => {});
  }, []);

  const startPolling = useCallback(() => {
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const t = await getTasks(20);
        setTasks(t);
        const allDone = t.every(
          (task) => task.status === "complete" || task.status === "failed"
        );
        if (allDone && t.length > 0) {
          clearInterval(pollRef.current);
          pollRef.current = undefined;
        }
      } catch {
        if (pollRef.current) clearInterval(pollRef.current);
      }
    }, 1000);
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, []);

  const handleImport = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      setError(null);
      setImporting(true);
      try {
        const imported = await importFiles(paths);
        if (imported.length === 0) {
          setError("No supported files found.");
          return;
        }
        setResults((prev) => [...imported, ...prev]);
        startPolling();
      } catch (e: any) {
        setError(typeof e === "string" ? e : e?.message || "Import failed");
      } finally {
        setImporting(false);
      }
    },
    [startPolling]
  );

  const handleBrowse = useCallback(async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Documents",
            extensions: [
              "pdf", "epub", "mobi", "txt", "html", "htm", "md",
              "djvu", "cbz", "cbr",
            ],
          },
        ],
      });
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        handleImport(paths);
      }
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || "Failed to open file dialog");
    }
  }, [handleImport]);

  const handleBrowseFolder = useCallback(async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected) {
        const path = Array.isArray(selected) ? selected[0] : selected;
        handleImport([path]);
      }
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || "Failed to open folder dialog");
    }
  }, [handleImport]);

  const [isDragActive, setIsDragActive] = useState(false);

  // Determine which list to show — prefer tasks once they start coming in
  const displayItems = tasks.length > 0 ? tasks : results;

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
        <h1
          className={css({
            fontSize: "2xl",
            fontWeight: 700,
            letterSpacing: "-0.03em",
            color: "text.primary",
            WebkitAppRegion: "no-drag",
          } as any)}
        >
          Import
        </h1>
      </header>

      <div
        className={css({
          flex: 1,
          overflow: "auto",
          padding: "lg",
          display: "flex",
          flexDirection: "column",
          gap: "lg",
        })}
      >
        {/* API key warning */}
        {hasApiKey === false && (
          <motion.div
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            className={css({
              display: "flex",
              alignItems: "center",
              gap: "sm",
              padding: "md",
              bg: "rgba(251, 191, 36, 0.08)",
              border: "1px solid rgba(251, 191, 36, 0.2)",
              borderRadius: "md",
            })}
          >
            <svg
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={1.5}
              className={css({ color: "status.warning", flexShrink: 0 })}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"
              />
            </svg>
            <span className={css({ fontSize: "sm", color: "status.warning", flex: 1 })}>
              No API key configured. Files will be stored but not processed.
            </span>
            <button
              onClick={() => navigate("/settings")}
              className={css({
                fontSize: "sm",
                fontWeight: 500,
                color: "accent.bright",
                bg: "transparent",
                border: "none",
                cursor: "pointer",
                whiteSpace: "nowrap",
                _hover: { textDecoration: "underline" },
              } as any)}
            >
              Add key
            </button>
          </motion.div>
        )}

        {/* Error banner */}
        <AnimatePresence>
          {error && (
            <motion.div
              initial={{ opacity: 0, y: -8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              className={css({
                display: "flex",
                alignItems: "center",
                gap: "sm",
                padding: "md",
                bg: "rgba(248, 113, 113, 0.08)",
                border: "1px solid rgba(248, 113, 113, 0.2)",
                borderRadius: "md",
              })}
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={1.5}
                className={css({ color: "status.error", flexShrink: 0 })}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z"
                />
              </svg>
              <span className={css({ fontSize: "sm", color: "status.error", flex: 1 })}>
                {error}
              </span>
              <button
                onClick={() => setError(null)}
                className={css({
                  bg: "transparent",
                  border: "none",
                  color: "text.muted",
                  cursor: "pointer",
                  fontSize: "sm",
                  _hover: { color: "text.primary" },
                } as any)}
              >
                Dismiss
              </button>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Drop zone */}
        <motion.div
          animate={{
            borderColor: isDragActive
              ? "rgba(212, 160, 23, 0.5)"
              : "rgba(255, 255, 255, 0.1)",
            backgroundColor: isDragActive
              ? "rgba(212, 160, 23, 0.05)"
              : "transparent",
          }}
          className={css({
            border: "2px dashed",
            borderColor: "border.base",
            borderRadius: "xl",
            padding: "3xl",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: "md",
            minHeight: "240px",
            transition: "all 200ms",
          })}
          onDragEnter={() => setIsDragActive(true)}
          onDragLeave={() => setIsDragActive(false)}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            setIsDragActive(false);
          }}
        >
          <svg
            width="40"
            height="40"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={1}
            className={css({ color: "text.muted" })}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5"
            />
          </svg>

          <p className={css({ color: "text.secondary", fontSize: "md" })}>
            {isDragActive
              ? "Drop files here..."
              : "Drag files or folders here"}
          </p>

          <div className={css({ display: "flex", gap: "sm" })}>
            <button
              onClick={handleBrowse}
              disabled={importing}
              className={css({
                bg: "bg.elevated",
                color: "text.primary",
                border: "1px solid",
                borderColor: "border.base",
                borderRadius: "md",
                padding: "sm",
                paddingLeft: "md",
                paddingRight: "md",
                fontSize: "sm",
                fontWeight: 500,
                cursor: "pointer",
                transition: "all 150ms",
                _hover: {
                  bg: "bg.hover",
                  borderColor: "border.strong",
                },
                _disabled: {
                  opacity: 0.5,
                  cursor: "not-allowed",
                },
              } as any)}
            >
              {importing ? "Importing..." : "Browse files"}
            </button>
            <button
              onClick={handleBrowseFolder}
              disabled={importing}
              className={css({
                bg: "transparent",
                color: "text.secondary",
                border: "1px solid",
                borderColor: "border.subtle",
                borderRadius: "md",
                padding: "sm",
                paddingLeft: "md",
                paddingRight: "md",
                fontSize: "sm",
                fontWeight: 500,
                cursor: "pointer",
                transition: "all 150ms",
                _hover: {
                  color: "text.primary",
                  borderColor: "border.base",
                },
                _disabled: {
                  opacity: 0.5,
                  cursor: "not-allowed",
                },
              } as any)}
            >
              Browse folder
            </button>
          </div>

          <p className={css({ fontSize: "xs", color: "text.muted" })}>
            PDF, EPUB, MOBI, TXT, HTML, Markdown, DjVu, CBZ/CBR
          </p>
        </motion.div>

        {/* Task list */}
        {displayItems.length > 0 && (
          <div>
            <div className={css({ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "md" })}>
              <h2
                className={css({
                  fontSize: "md",
                  fontWeight: 600,
                  color: "text.primary",
                })}
              >
                Import Queue
              </h2>
              <button
                onClick={async () => {
                  await clearFinishedTasks();
                  const t = await getTasks(20);
                  setTasks(t);
                  if (t.length === 0) setResults([]);
                }}
                className={css({
                  bg: "transparent",
                  border: "none",
                  color: "text.muted",
                  fontSize: "xs",
                  cursor: "pointer",
                  _hover: { color: "text.primary" },
                } as any)}
              >
                Clear finished
              </button>
            </div>
            <div
              className={css({
                display: "flex",
                flexDirection: "column",
                gap: "xs",
              })}
            >
              <AnimatePresence>
                {displayItems.map((item, i) => {
                  const task = "progress" in item ? (item as Task) : null;
                  const result = !task ? (item as ImportResult) : null;

                  return (
                    <motion.div
                      key={task?.id || result?.task_id || i}
                      initial={{ opacity: 0, x: -10 }}
                      animate={{ opacity: 1, x: 0 }}
                      className={css({
                        display: "flex",
                        alignItems: "center",
                        gap: "md",
                        padding: "sm",
                        paddingLeft: "md",
                        bg: "bg.surface",
                        borderRadius: "md",
                        border: "1px solid",
                        borderColor: "border.subtle",
                      })}
                    >
                      {/* Status indicator */}
                      <div
                        className={css({
                          width: "8px",
                          height: "8px",
                          borderRadius: "full",
                          flexShrink: 0,
                          bg:
                            task?.status === "complete"
                              ? "status.success"
                              : task?.status === "failed"
                              ? "status.error"
                              : task?.status === "running"
                              ? "status.warning"
                              : "text.muted",
                        })}
                      />

                      <div className={css({ flex: 1, minWidth: 0 })}>
                        <p
                          className={css({
                            fontSize: "sm",
                            color: "text.primary",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          })}
                        >
                          {result?.filename || task?.message || "Queued..."}
                        </p>
                        {task?.error && (
                          <p
                            className={css({
                              fontSize: "xs",
                              color: "status.error",
                              marginTop: "2px",
                            })}
                          >
                            {task.error}
                          </p>
                        )}
                      </div>

                      {/* Progress */}
                      {task && task.status === "running" && (
                        <div
                          className={css({
                            width: "80px",
                            height: "4px",
                            bg: "bg.elevated",
                            borderRadius: "full",
                            overflow: "hidden",
                          })}
                        >
                          <motion.div
                            initial={{ width: 0 }}
                            animate={{ width: `${task.progress * 100}%` }}
                            className={css({
                              height: "100%",
                              bg: "accent.base",
                              borderRadius: "full",
                            })}
                          />
                        </div>
                      )}

                      {/* Status label for completed/failed */}
                      {task?.status === "complete" && (
                        <span className={css({ fontSize: "xs", color: "status.success" })}>
                          Done
                        </span>
                      )}
                      {task?.status === "failed" && (
                        <span className={css({ fontSize: "xs", color: "status.error" })}>
                          Failed
                        </span>
                      )}

                      {/* Delete button */}
                      <button
                        onClick={async (e) => {
                          e.stopPropagation();
                          const taskId = task?.id || result?.task_id;
                          if (taskId) {
                            await deleteTask(taskId);
                            const t = await getTasks(20);
                            setTasks(t);
                            if (t.length === 0) setResults([]);
                          }
                        }}
                        className={css({
                          bg: "transparent",
                          border: "none",
                          color: "text.muted",
                          cursor: "pointer",
                          padding: "2px",
                          flexShrink: 0,
                          opacity: 0.5,
                          transition: "all 150ms",
                          _hover: { opacity: 1, color: "text.primary" },
                        } as any)}
                      >
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      </button>
                    </motion.div>
                  );
                })}
              </AnimatePresence>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
