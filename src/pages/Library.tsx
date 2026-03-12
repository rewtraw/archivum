import { useState, useEffect, useCallback, useRef } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  listDocuments,
  getStats,
  importFiles,
  importUrl,
  importYoutube,
  getTasks,
  getSettings,
  getDocumentCover,
  deleteTask,
  clearFinishedTasks,
  regenerateCovers,
  listTags,
  formatFileSize,
  formatDate,
} from "../lib/api";
import type { Document, LibraryStats, ImportResult, Task, ListDocumentsOptions } from "../lib/api";

function titleToHue(title: string): number {
  let hash = 0;
  for (let i = 0; i < title.length; i++) {
    hash = title.charCodeAt(i) + ((hash << 5) - hash);
  }
  return Math.abs(hash) % 360;
}

function CoverImage({ documentId, hasCover }: { documentId: string; hasCover: boolean }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!hasCover) return;
    let revoke: string | null = null;
    getDocumentCover(documentId)
      .then((bytes) => {
        const blob = new Blob([new Uint8Array(bytes)]);
        const url = URL.createObjectURL(blob);
        revoke = url;
        setSrc(url);
      })
      .catch(() => {});
    return () => { if (revoke) URL.revokeObjectURL(revoke); };
  }, [documentId, hasCover]);

  if (!src) return null;

  return (
    <img
      src={src}
      className={css({
        position: "absolute",
        inset: 0,
        width: "100%",
        height: "100%",
        objectFit: "cover",
      })}
    />
  );
}

function PlaceholderCover({ title, author, format }: { title: string; author: string; format: string }) {
  const hue = titleToHue(title || format);

  return (
    <div
      className={css({
        position: "absolute",
        inset: 0,
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        padding: "md",
        overflow: "hidden",
      })}
      style={{
        background: `linear-gradient(160deg, hsl(${hue}, 30%, 18%) 0%, hsl(${(hue + 30) % 360}, 25%, 12%) 100%)`,
      }}
    >
      {/* Decorative line */}
      <div
        className={css({ width: "32px", height: "2px", borderRadius: "full", opacity: 0.4 })}
        style={{ background: `hsl(${hue}, 40%, 50%)` }}
      />

      {/* Title area */}
      <div className={css({ flex: 1, display: "flex", flexDirection: "column", justifyContent: "center", gap: "xs" })}>
        <p
          className={css({
            fontSize: "sm",
            fontWeight: 700,
            lineHeight: 1.3,
            overflow: "hidden",
            display: "-webkit-box",
            WebkitLineClamp: 4,
            WebkitBoxOrient: "vertical",
          } as any)}
          style={{ color: `hsl(${hue}, 20%, 75%)` }}
        >
          {title || "Untitled"}
        </p>
        {author && author !== "Unknown" && (
          <p
            className={css({
              fontSize: "xs",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              opacity: 0.5,
            })}
            style={{ color: `hsl(${hue}, 15%, 60%)` }}
          >
            {author}
          </p>
        )}
      </div>

      {/* Format badge */}
      <div
        className={css({
          fontSize: "10px",
          fontFamily: "mono",
          textTransform: "uppercase",
          letterSpacing: "0.05em",
          opacity: 0.35,
          alignSelf: "flex-end",
        })}
        style={{ color: `hsl(${hue}, 20%, 55%)` }}
      >
        {format}
      </div>
    </div>
  );
}

export function Library() {
  const [documents, setDocuments] = useState<Document[]>([]);
  const [stats, setStats] = useState<LibraryStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasApiKey, setHasApiKey] = useState<boolean | null>(null);
  const [hasCloudflare, setHasCloudflare] = useState<boolean | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [showUrlInput, setShowUrlInput] = useState(false);
  const [urlValue, setUrlValue] = useState("");
  const [dragOver, setDragOver] = useState(false);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [allTags, setAllTags] = useState<string[]>([]);
  const pollRef = useRef<ReturnType<typeof setInterval>>(undefined);
  const gridRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  // Sort & filter state
  const [sortBy, setSortBy] = useState<string>(searchParams.get("sort") || "imported_at");
  const [sortDir, setSortDir] = useState<string>(searchParams.get("dir") || "desc");
  const [formatFilter, setFormatFilter] = useState<string>(searchParams.get("format") || "");
  const [statusFilter, setStatusFilter] = useState<string>(searchParams.get("status") || "");
  const [tagFilter, setTagFilter] = useState<string>(searchParams.get("tag") || "");

  const listOpts: ListDocumentsOptions = {
    sortBy: sortBy || undefined,
    sortDir: sortDir || undefined,
    formatFilter: formatFilter || undefined,
    statusFilter: statusFilter || undefined,
    tagFilter: tagFilter || undefined,
  };

  const reload = useCallback(async () => {
    try {
      const [docs, libraryStats] = await Promise.all([
        listDocuments(listOpts),
        getStats(),
      ]);
      setDocuments(docs);
      setStats(libraryStats);
    } catch (e) {
      console.error("Failed to load library:", e);
    }
  }, [sortBy, sortDir, formatFilter, statusFilter, tagFilter]);

  const coverRegenTriggered = useRef(false);

  useEffect(() => {
    async function init() {
      await reload();
      const [settings, tags] = await Promise.all([
        getSettings().catch(() => null),
        listTags().catch(() => [] as string[]),
      ]);
      if (settings) {
        setHasApiKey(settings.has_api_key);
        setHasCloudflare(settings.has_cloudflare);
      }
      setAllTags(tags);
      setLoading(false);
    }
    init();
  }, [reload]);

  // Auto-regenerate covers for documents missing them (once per session)
  useEffect(() => {
    if (loading || coverRegenTriggered.current) return;
    const missing = documents.filter(
      (d) => !d.cover_path && d.status === "complete" && ["epub", "pdf"].includes(d.original_format)
    );
    if (missing.length > 0) {
      coverRegenTriggered.current = true;
      regenerateCovers()
        .then(() => {
          const poll = setInterval(async () => {
            const t = await getTasks(10);
            const coverTask = t.find((task) => task.task_type === "regenerate-covers");
            if (!coverTask || coverTask.status === "complete" || coverTask.status === "failed") {
              clearInterval(poll);
              if (coverTask) await deleteTask(coverTask.id);
              reload();
            }
          }, 1500);
        })
        .catch(() => {});
    }
  }, [loading, documents, reload]);

  // Refs for drag-drop and import trigger (avoids stale closure issues)
  const handleImportRef = useRef<(paths: string[]) => void>(() => {});
  const handleBrowseRef = useRef<() => void>(() => {});

  // Keyboard navigation in grid
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (documents.length === 0) return;

      const cols = gridRef.current
        ? Math.floor(gridRef.current.offsetWidth / 216) // ~200px min + gap
        : 4;

      switch (e.key) {
        case "ArrowRight":
          e.preventDefault();
          setFocusedIndex((i) => Math.min(i + 1, documents.length - 1));
          break;
        case "ArrowLeft":
          e.preventDefault();
          setFocusedIndex((i) => Math.max(i - 1, 0));
          break;
        case "ArrowDown":
          e.preventDefault();
          setFocusedIndex((i) => Math.min(i + cols, documents.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          setFocusedIndex((i) => Math.max(i - cols, 0));
          break;
        case "Enter":
          if (focusedIndex >= 0 && focusedIndex < documents.length) {
            navigate(`/document/${documents[focusedIndex].id}`);
          }
          break;
        case "Escape":
          setFocusedIndex(-1);
          break;
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [documents, focusedIndex, navigate]);

  // Cleanup polling
  useEffect(() => () => { if (pollRef.current) clearInterval(pollRef.current); }, []);

  const startPolling = useCallback(() => {
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const t = await getTasks(20);
        setTasks(t);
        // Refresh library when imports finish
        // Remove completed tasks automatically, keep failed ones visible
        const completed = t.filter((task) => task.status === "complete");
        for (const task of completed) {
          await deleteTask(task.id);
        }
        const remaining = t.filter((task) => task.status !== "complete");
        setTasks(remaining);
        // Refresh library when anything finishes
        if (completed.length > 0) reload();
        // Stop polling when nothing is in progress
        const allDone = remaining.every((task) => task.status === "failed");
        if (allDone) {
          clearInterval(pollRef.current);
          pollRef.current = undefined;
        }
      } catch {
        if (pollRef.current) clearInterval(pollRef.current);
      }
    }, 1000);
  }, [reload]);

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
        filters: [{ name: "Documents & Media", extensions: ["pdf", "epub", "mobi", "txt", "html", "htm", "md", "djvu", "cbz", "cbr", "mp3", "m4a", "wav", "flac", "ogg", "aac", "mp4", "mkv", "webm", "mov", "avi"] }],
      });
      if (selected) {
        handleImport(Array.isArray(selected) ? selected : [selected]);
      }
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || "Failed to open file dialog");
    }
  }, [handleImport]);

  const handleBrowseFolder = useCallback(async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected) {
        handleImport([Array.isArray(selected) ? selected[0] : selected]);
      }
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || "Failed to open folder dialog");
    }
  }, [handleImport]);

  const handleImportUrl = useCallback(async () => {
    const trimmed = urlValue.trim();
    if (!trimmed) return;
    setError(null);
    setImporting(true);
    try {
      const isYouTube = /^https?:\/\/(www\.)?(youtube\.com|youtu\.be)\//i.test(trimmed);
      if (isYouTube) {
        await importYoutube(trimmed);
      } else {
        await importUrl(trimmed);
      }
      setUrlValue("");
      setShowUrlInput(false);
      startPolling();
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || "URL import failed");
    } finally {
      setImporting(false);
    }
  }, [urlValue, startPolling]);

  // Keep refs in sync for stable effect closures
  handleImportRef.current = handleImport;
  handleBrowseRef.current = handleBrowse;

  // Drag-and-drop import
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onDragDropEvent((event) => {
      if (event.payload.type === "drop") {
        setDragOver(false);
        handleImportRef.current(event.payload.paths);
      } else if (event.payload.type === "enter") {
        setDragOver(true);
      } else if (event.payload.type === "leave") {
        setDragOver(false);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Auto-import trigger from Cmd+I (via URL param)
  useEffect(() => {
    if (searchParams.get("import") === "true") {
      setSearchParams({}, { replace: true });
      handleBrowseRef.current();
    }
  }, []);

  const activeTasks = tasks.filter((t) => t.status === "running" || t.status === "queued");
  const showTaskQueue = tasks.length > 0;

  return (
    <div className={css({ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" })}>
      {/* Header */}
      <header
        className={css({
          padding: "lg", paddingTop: "48px", paddingBottom: "md",
          borderBottom: "1px solid", borderColor: "border.subtle",
          display: "flex", alignItems: "center", justifyContent: "space-between",
          WebkitAppRegion: "drag",
        } as any)}
      >
        <div className={css({ WebkitAppRegion: "no-drag" } as any)}>
          <h1 className={css({ fontSize: "2xl", fontWeight: 700, letterSpacing: "-0.03em", color: "text.primary" })}>
            Library
          </h1>
          {stats && (
            <p className={css({ fontSize: "sm", color: "text.muted", marginTop: "xs" })}>
              {stats.total_documents} documents &middot; {formatFileSize(stats.total_size_bytes)}
            </p>
          )}
        </div>

        <div className={css({ display: "flex", gap: "xs", WebkitAppRegion: "no-drag" } as any)}>
          <button
            onClick={handleBrowse}
            disabled={importing}
            className={css({
              bg: "accent.subtle", color: "accent.bright",
              border: "1px solid", borderColor: "accent.dim",
              borderRadius: "md", padding: "xs", paddingLeft: "sm", paddingRight: "sm",
              fontSize: "xs", fontWeight: 500, cursor: "pointer", transition: "all 150ms",
              display: "flex", alignItems: "center", gap: "xs",
              _hover: { bg: "accent.base", color: "text.inverse" },
              _disabled: { opacity: 0.5, cursor: "not-allowed" },
            } as any)}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
              <path d="M12 4v16m8-8H4" />
            </svg>
            {importing ? "Importing..." : "Import"}
          </button>
          <button
            onClick={handleBrowseFolder}
            disabled={importing}
            className={css({
              bg: "transparent", color: "text.muted",
              border: "1px solid", borderColor: "border.subtle",
              borderRadius: "md", padding: "xs", paddingLeft: "sm", paddingRight: "sm",
              fontSize: "xs", cursor: "pointer", transition: "all 150ms",
              _hover: { borderColor: "border.base", color: "text.primary" },
              _disabled: { opacity: 0.5, cursor: "not-allowed" },
            } as any)}
          >
            Folder
          </button>
          {hasCloudflare && (
            <button
              onClick={() => setShowUrlInput(!showUrlInput)}
              disabled={importing}
              className={css({
                bg: "transparent", color: "text.muted",
                border: "1px solid", borderColor: showUrlInput ? "accent.dim" : "border.subtle",
                borderRadius: "md", padding: "xs", paddingLeft: "sm", paddingRight: "sm",
                fontSize: "xs", cursor: "pointer", transition: "all 150ms",
                _hover: { borderColor: "border.base", color: "text.primary" },
                _disabled: { opacity: 0.5, cursor: "not-allowed" },
              } as any)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
                <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
              </svg>
            </button>
          )}
        </div>
      </header>

      {/* URL import bar */}
      <AnimatePresence>
        {showUrlInput && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className={css({ overflow: "hidden" })}
          >
            <div
              className={css({
                display: "flex", gap: "sm", padding: "sm", paddingLeft: "lg", paddingRight: "lg",
                borderBottom: "1px solid", borderColor: "border.subtle",
                bg: "bg.surface",
              })}
            >
              <input
                type="url"
                placeholder="https://youtube.com/watch?v=... or any URL"
                value={urlValue}
                onChange={(e) => setUrlValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleImportUrl();
                  if (e.key === "Escape") { setShowUrlInput(false); setUrlValue(""); }
                }}
                autoFocus
                className={css({
                  flex: 1, bg: "bg.base", border: "1px solid", borderColor: "border.base",
                  borderRadius: "md", padding: "xs", paddingLeft: "sm",
                  color: "text.primary", fontSize: "sm", outline: "none",
                  _focus: { borderColor: "accent.dim" },
                  _placeholder: { color: "text.muted" },
                } as any)}
              />
              <button
                onClick={handleImportUrl}
                disabled={!urlValue.trim() || importing}
                className={css({
                  bg: "accent.subtle", color: "accent.bright",
                  border: "1px solid", borderColor: "accent.dim",
                  borderRadius: "md", padding: "xs", paddingLeft: "sm", paddingRight: "sm",
                  fontSize: "xs", fontWeight: 500, cursor: "pointer", transition: "all 150ms",
                  whiteSpace: "nowrap",
                  _hover: { bg: "accent.base", color: "text.inverse" },
                  _disabled: { opacity: 0.4, cursor: "not-allowed" },
                } as any)}
              >
                {importing ? "Importing..." : "Import URL"}
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Sort & filter bar */}
      {!loading && documents.length > 0 && (
        <div
          className={css({
            display: "flex",
            alignItems: "center",
            gap: "6px",
            padding: "8px",
            paddingLeft: "lg",
            paddingRight: "lg",
            borderBottom: "1px solid",
            borderColor: "border.subtle",
            flexWrap: "wrap",
          })}
        >
          {/* Sort */}
          <div className={css({ display: "flex", alignItems: "center", gap: "2px", bg: "rgba(255,255,255,0.04)", borderRadius: "md", padding: "2px" })}>
            <select
              value={sortBy}
              onChange={(e) => setSortBy(e.target.value)}
              className={css({
                WebkitAppearance: "none",
                appearance: "none",
                bg: "transparent",
                border: "none",
                color: "text.secondary",
                fontSize: "11px",
                fontWeight: 500,
                fontFamily: "body",
                letterSpacing: "0.01em",
                padding: "4px 8px",
                paddingRight: "4px",
                cursor: "pointer",
                outline: "none",
                _hover: { color: "text.primary" },
              } as any)}
            >
              <option value="imported_at">Date added</option>
              <option value="title">Title</option>
              <option value="author">Author</option>
              <option value="file_size">Size</option>
              <option value="last_read">Last read</option>
            </select>
            <button
              onClick={() => setSortDir((d) => (d === "asc" ? "desc" : "asc"))}
              className={css({
                bg: "transparent", border: "none", color: "text.muted", cursor: "pointer",
                padding: "4px 6px", fontSize: "11px", lineHeight: 1,
                borderRadius: "sm", transition: "all 120ms",
                _hover: { color: "text.primary", bg: "rgba(255,255,255,0.06)" },
              } as any)}
              title={sortDir === "asc" ? "Ascending" : "Descending"}
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round">
                {sortDir === "desc" ? (
                  <path d="M5 2v6M2.5 5.5L5 8l2.5-2.5" />
                ) : (
                  <path d="M5 8V2M2.5 4.5L5 2l2.5 2.5" />
                )}
              </svg>
            </button>
          </div>

          {/* Filters */}
          {[
            {
              value: formatFilter,
              onChange: setFormatFilter,
              placeholder: "Format",
              options: (stats?.format_counts || []).map(([fmt]) => ({ value: fmt, label: fmt.toUpperCase() })),
            },
            {
              value: statusFilter,
              onChange: setStatusFilter,
              placeholder: "Status",
              options: [
                { value: "To Read", label: "To Read" },
                { value: "Reading", label: "Reading" },
                { value: "Read", label: "Read" },
              ],
            },
            ...(allTags.length > 0
              ? [{
                  value: tagFilter,
                  onChange: setTagFilter,
                  placeholder: "Tag",
                  options: allTags.map((t) => ({ value: t, label: t })),
                }]
              : []),
          ].map(({ value, onChange, placeholder, options }) => (
            <select
              key={placeholder}
              value={value}
              onChange={(e) => onChange(e.target.value)}
              className={css({
                WebkitAppearance: "none",
                appearance: "none",
                bg: value ? "rgba(184, 168, 138, 0.1)" : "rgba(255,255,255,0.04)",
                border: "none",
                borderRadius: "md",
                color: value ? "accent.bright" : "text.muted",
                fontSize: "11px",
                fontWeight: 500,
                fontFamily: "body",
                letterSpacing: "0.01em",
                padding: "6px 10px",
                cursor: "pointer",
                outline: "none",
                transition: "all 150ms",
                maxWidth: "130px",
                _hover: { bg: value ? "rgba(184, 168, 138, 0.14)" : "rgba(255,255,255,0.07)" },
              } as any)}
            >
              <option value="">{placeholder}</option>
              {options.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          ))}

          {/* Clear filters */}
          <AnimatePresence>
            {(formatFilter || statusFilter || tagFilter) && (
              <motion.button
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
                onClick={() => { setFormatFilter(""); setStatusFilter(""); setTagFilter(""); }}
                className={css({
                  bg: "transparent", border: "none", color: "text.muted", cursor: "pointer",
                  fontSize: "11px", padding: "4px 8px", borderRadius: "md",
                  transition: "all 120ms",
                  _hover: { color: "text.primary", bg: "rgba(255,255,255,0.04)" },
                } as any)}
              >
                Clear
              </motion.button>
            )}
          </AnimatePresence>
        </div>
      )}

      {/* Drag-and-drop overlay */}
      <AnimatePresence>
        {dragOver && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className={css({
              position: "fixed",
              inset: 0,
              zIndex: 100,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              bg: "rgba(0, 0, 0, 0.6)",
              backdropFilter: "blur(4px)",
            })}
          >
            <div
              className={css({
                padding: "3xl",
                border: "2px dashed",
                borderColor: "accent.dim",
                borderRadius: "xl",
                bg: "bg.surface",
                textAlign: "center",
              })}
            >
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1} className={css({ color: "accent.bright", marginBottom: "md", margin: "0 auto" })}>
                <path d="M12 4v16m8-8H4" />
              </svg>
              <p className={css({ color: "text.primary", fontSize: "lg", fontWeight: 600 })}>Drop files to import</p>
              <p className={css({ color: "text.muted", fontSize: "sm", marginTop: "xs" })}>PDF, EPUB, MOBI, TXT, HTML, and more</p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Content */}
      <div className={css({ flex: 1, overflow: "auto", padding: "lg", display: "flex", flexDirection: "column", gap: "lg" })}>
        {/* API key warning */}
        {hasApiKey === false && (
          <motion.div
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            className={css({
              display: "flex", alignItems: "center", gap: "sm", padding: "md",
              bg: "rgba(251, 191, 36, 0.08)", border: "1px solid rgba(251, 191, 36, 0.2)", borderRadius: "md",
            })}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} className={css({ color: "status.warning", flexShrink: 0 })}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
            </svg>
            <span className={css({ fontSize: "sm", color: "status.warning", flex: 1 })}>
              No API key configured. Files will be stored but metadata won't be enriched.
            </span>
            <button
              onClick={() => navigate("/settings")}
              className={css({ fontSize: "sm", fontWeight: 500, color: "accent.bright", bg: "transparent", border: "none", cursor: "pointer", whiteSpace: "nowrap", _hover: { textDecoration: "underline" } } as any)}
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
                display: "flex", alignItems: "center", gap: "sm", padding: "md",
                bg: "rgba(248, 113, 113, 0.08)", border: "1px solid rgba(248, 113, 113, 0.2)", borderRadius: "md",
              })}
            >
              <span className={css({ fontSize: "sm", color: "status.error", flex: 1 })}>{error}</span>
              <button
                onClick={() => setError(null)}
                className={css({ bg: "transparent", border: "none", color: "text.muted", cursor: "pointer", fontSize: "sm", _hover: { color: "text.primary" } } as any)}
              >
                Dismiss
              </button>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Import task queue */}
        <AnimatePresence>
          {showTaskQueue && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
            >
              <div className={css({ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "sm" })}>
                <span className={css({ fontSize: "sm", fontWeight: 600, color: "text.primary" })}>
                  Import Queue
                  {activeTasks.length > 0 && (
                    <span className={css({ color: "text.muted", fontWeight: 400 })}> &middot; {activeTasks.length} active</span>
                  )}
                </span>
                <button
                  onClick={async () => {
                    await clearFinishedTasks();
                    const t = await getTasks(20);
                    setTasks(t);
                  }}
                  className={css({ bg: "transparent", border: "none", color: "text.muted", fontSize: "xs", cursor: "pointer", _hover: { color: "text.primary" } } as any)}
                >
                  Clear finished
                </button>
              </div>
              <div className={css({ display: "flex", flexDirection: "column", gap: "xs" })}>
                {tasks.map((task) => (
                  <div
                    key={task.id}
                    className={css({
                      display: "flex", alignItems: "center", gap: "md",
                      padding: "sm", paddingLeft: "md",
                      bg: "bg.surface", borderRadius: "md",
                      border: "1px solid", borderColor: "border.subtle",
                    })}
                  >
                    <div className={css({
                      width: "8px", height: "8px", borderRadius: "full", flexShrink: 0,
                      bg: task.status === "complete" ? "status.success" : task.status === "failed" ? "status.error" : task.status === "running" ? "status.warning" : "text.muted",
                    })} />
                    <div className={css({ flex: 1, minWidth: 0 })}>
                      <p className={css({ fontSize: "sm", color: "text.primary", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" })}>
                        {task.message || "Queued..."}
                      </p>
                      {task.error && (
                        <p className={css({ fontSize: "xs", color: "status.error", marginTop: "2px" })}>{task.error}</p>
                      )}
                    </div>
                    {task.status === "running" && (
                      <div className={css({ width: "80px", height: "4px", bg: "bg.elevated", borderRadius: "full", overflow: "hidden" })}>
                        <motion.div
                          initial={{ width: 0 }}
                          animate={{ width: `${task.progress * 100}%` }}
                          className={css({ height: "100%", bg: "accent.base", borderRadius: "full" })}
                        />
                      </div>
                    )}
                    {task.status === "complete" && <span className={css({ fontSize: "xs", color: "status.success" })}>Done</span>}
                    {task.status === "failed" && <span className={css({ fontSize: "xs", color: "status.error" })}>Failed</span>}
                    <button
                      onClick={async (e) => {
                        e.stopPropagation();
                        await deleteTask(task.id);
                        const t = await getTasks(20);
                        setTasks(t);
                      }}
                      className={css({
                        bg: "transparent", border: "none", color: "text.muted", cursor: "pointer",
                        padding: "2px", flexShrink: 0, opacity: 0.5, transition: "all 150ms",
                        _hover: { opacity: 1, color: "text.primary" },
                      } as any)}
                    >
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                ))}
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Document grid */}
        {loading ? (
          <div className={css({ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "text.muted" })}>
            <motion.div animate={{ opacity: [0.3, 1, 0.3] }} transition={{ duration: 1.5, repeat: Infinity }}>Loading...</motion.div>
          </div>
        ) : documents.length === 0 && !showTaskQueue ? (
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className={css({ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", height: "100%", gap: "md" })}
          >
            <div className={css({ fontSize: "3xl", opacity: 0.2 })}>
              <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1}>
                <path d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
              </svg>
            </div>
            <p className={css({ color: "text.muted", fontSize: "md" })}>Your archive is empty</p>
            <button
              onClick={handleBrowse}
              className={css({
                bg: "accent.subtle", color: "accent.bright",
                border: "1px solid", borderColor: "accent.dim",
                borderRadius: "md", padding: "sm", paddingLeft: "md", paddingRight: "md",
                fontSize: "sm", fontWeight: 500, cursor: "pointer", transition: "all 200ms",
                _hover: { bg: "accent.base", color: "text.inverse" },
              } as any)}
            >
              Import your first document
            </button>
          </motion.div>
        ) : (
          <div ref={gridRef} className={css({ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))", gap: "md" })}>
            <AnimatePresence>
              {documents.map((doc, i) => (
                <motion.div
                  key={doc.id}
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: i * 0.03 }}
                  onClick={() => { setFocusedIndex(i); navigate(`/document/${doc.id}`); }}
                  className={css({
                    bg: "bg.surface", borderRadius: "lg",
                    border: "1px solid", borderColor: "border.subtle",
                    overflow: "hidden", cursor: "pointer", transition: "all 200ms",
                    _hover: { borderColor: "border.strong", transform: "translateY(-2px)" },
                  } as any)}
                  style={focusedIndex === i ? { outline: "2px solid var(--colors-accent-dim)", outlineOffset: "2px", borderRadius: "8px" } : undefined}
                >
                  <div className={css({
                    height: "240px", bg: "bg.elevated",
                    display: "flex", alignItems: "center", justifyContent: "center",
                    fontSize: "3xl", color: "text.muted", position: "relative", overflow: "hidden",
                  })}>
                    <CoverImage documentId={doc.id} hasCover={!!doc.cover_path} />
                    {!doc.cover_path && (
                      <PlaceholderCover
                        title={doc.title}
                        author={doc.author}
                        format={doc.original_format}
                      />
                    )}
                    {/* Reading progress bar */}
                    {doc.reading_progress != null && doc.reading_progress > 0 && (
                      <div className={css({
                        position: "absolute", bottom: 0, left: 0, right: 0,
                        height: "3px", bg: "rgba(0,0,0,0.3)",
                      })}>
                        <div
                          className={css({ height: "100%", bg: "accent.base", borderRadius: "0 1px 0 0" })}
                          style={{ width: `${Math.round(doc.reading_progress * 100)}%` }}
                        />
                      </div>
                    )}
                  </div>
                  <div className={css({ padding: "md" })}>
                    <h3 className={css({
                      fontSize: "sm", fontWeight: 600, color: "text.primary", lineHeight: 1.3,
                      overflow: "hidden", textOverflow: "ellipsis",
                      display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical",
                    } as any)}>
                      {doc.title || "Untitled"}
                    </h3>
                    <p className={css({ fontSize: "xs", color: "text.secondary", marginTop: "xs", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" })}>
                      {doc.author || "Unknown author"}
                    </p>
                    <div className={css({ display: "flex", alignItems: "center", gap: "xs", marginTop: "sm" })}>
                      <span className={css({ fontSize: "xs", color: "text.muted", fontFamily: "mono" })}>
                        {formatFileSize(doc.file_size)}
                      </span>
                      {doc.status === "processing" && (
                        <motion.span animate={{ opacity: [0.4, 1, 0.4] }} transition={{ duration: 1.5, repeat: Infinity }} className={css({ fontSize: "xs", color: "status.warning" })}>
                          Processing...
                        </motion.span>
                      )}
                      {doc.status === "error" && (
                        <span className={css({ fontSize: "xs", color: "status.error" })}>Error</span>
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
