import { useState, useEffect, useMemo, useRef, useCallback, forwardRef, useImperativeHandle } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import ReactMarkdown from "react-markdown";
import { openPath } from "@tauri-apps/plugin-opener";
import ePub from "epubjs";
import type { Book, Rendition, NavItem } from "epubjs";
import {
  getDocument,
  getDocumentMarkdown,
  getDocumentHasChunks,
  getRelatedDocuments,
  updateReadingStatus,
  saveReadingProgress,
  getDocumentToc,
  generateSummary,
  getAllSummaries,
  getOriginalBytes,
  getOriginalPath,
  getMobiHtml,
  deleteDocument,
  formatFileSize,
  formatDate,
  formatDuration,
} from "../lib/api";
import type { Document, SearchResult, TocEntry } from "../lib/api";
import { ChatPanel } from "../components/ChatPanel";

const NATIVE_VIEWER_FORMATS = ["pdf", "epub", "mobi"];

// --- EPUB types and component ---

interface EpubLocation {
  chapter: string;
  percentage: number;
  atStart: boolean;
  atEnd: boolean;
}

interface EpubViewerHandle {
  navigateTo: (href: string) => void;
  setFontSize: (pct: number) => void;
}

interface EpubViewerProps {
  data: Uint8Array;
  fontSize: number;
  onLocationChange?: (loc: EpubLocation) => void;
  onTocLoaded?: (toc: NavItem[]) => void;
}

const EpubViewer = forwardRef<EpubViewerHandle, EpubViewerProps>(
  ({ data, fontSize, onLocationChange, onTocLoaded }, ref) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const renditionRef = useRef<Rendition | null>(null);

    useImperativeHandle(ref, () => ({
      navigateTo: (href: string) => renditionRef.current?.display(href),
      setFontSize: (pct: number) => renditionRef.current?.themes.fontSize(`${pct}%`),
    }));

    useEffect(() => {
      if (!containerRef.current || !data) return;

      const book = ePub(data.buffer);
      const rendition = book.renderTo(containerRef.current, {
        width: "100%",
        height: "100%",
        spread: "none",
        flow: "scrolled",
        manager: "continuous",
      });
      renditionRef.current = rendition;

      rendition.themes.default({
        body: {
          color: "#c8c3bc !important",
          background: "transparent !important",
          "font-family": "'Literata', Georgia, serif !important",
          "line-height": "1.8 !important",
          "max-width": "720px !important",
          margin: "0 auto !important",
          padding: "24px !important",
        },
        "h1, h2, h3, h4, h5, h6": {
          color: "#e8e4df !important",
          "font-family": "'Literata', Georgia, serif !important",
        },
        a: { color: "#b8a88a !important" },
        img: { "max-width": "100% !important", height: "auto !important" },
      });

      rendition.themes.fontSize(`${fontSize}%`);

      book.loaded.navigation.then((nav) => onTocLoaded?.(nav.toc));

      rendition.on("relocated", (loc: any) => {
        const chapter = book.navigation?.get(loc.start.href);
        onLocationChange?.({
          chapter: chapter?.label?.trim() || "",
          percentage: Math.round((loc.start.percentage || 0) * 100),
          atStart: loc.atStart,
          atEnd: loc.atEnd,
        });
      });

      rendition.display();

      return () => {
        rendition.destroy();
        book.destroy();
      };
    }, [data]);

    // Sync font size changes
    useEffect(() => {
      renditionRef.current?.themes.fontSize(`${fontSize}%`);
    }, [fontSize]);

    return (
      <div
        ref={containerRef}
        className={css({
          width: "100%",
          height: "100%",
          overflow: "auto",
          "& iframe": { border: "none !important" },
        } as any)}
      />
    );
  }
);

// --- MOBI viewer (renders extracted HTML in a styled iframe) ---

function MobiViewer({ html }: { html: string }) {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    if (!iframeRef.current) return;
    const doc = iframeRef.current.contentDocument;
    if (!doc) return;

    doc.open();
    doc.write(`<!DOCTYPE html>
<html>
<head>
<style>
  body {
    color: #c8c3bc;
    background: transparent;
    font-family: 'Literata', Georgia, serif;
    line-height: 1.8;
    max-width: 720px;
    margin: 0 auto;
    padding: 24px;
  }
  h1, h2, h3, h4, h5, h6 {
    color: #e8e4df;
    font-family: 'Literata', Georgia, serif;
  }
  a { color: #b8a88a; }
  img { max-width: 100%; height: auto; }
</style>
</head>
<body>${html}</body>
</html>`);
    doc.close();
  }, [html]);

  return (
    <iframe
      ref={iframeRef}
      className={css({
        width: "100%",
        height: "100%",
        border: "none",
      })}
      sandbox="allow-same-origin"
    />
  );
}

// --- Small reusable button ---

function ControlButton({
  onClick,
  title,
  active,
  children,
}: {
  onClick: () => void;
  title: string;
  active?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={css({
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        width: "28px",
        height: "28px",
        bg: active ? "rgba(255,255,255,0.08)" : "transparent",
        border: "none",
        borderRadius: "md",
        color: active ? "accent.base" : "text.muted",
        cursor: "pointer",
        transition: "all 120ms",
        _hover: { bg: "rgba(255,255,255,0.08)", color: "text.primary" },
      } as any)}
    >
      {children}
    </button>
  );
}

// --- Main page ---

export function DocumentView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [doc, setDoc] = useState<Document | null>(null);
  const [markdown, setMarkdown] = useState<string | null>(null);
  const [originalData, setOriginalData] = useState<Uint8Array | null>(null);
  const [mobiHtml, setMobiHtml] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<"read" | "info" | "original">("read");
  const [hasChunks, setHasChunks] = useState(false);
  const [askOpen, setAskOpen] = useState(false);
  const [related, setRelated] = useState<SearchResult[]>([]);
  const [readingStatus, setReadingStatus] = useState<string | null>(null);

  // EPUB reader state (lifted out of EpubViewer)
  const epubRef = useRef<EpubViewerHandle>(null);
  const [epubLoc, setEpubLoc] = useState<EpubLocation>({ chapter: "", percentage: 0, atStart: true, atEnd: false });
  const [epubToc, setEpubToc] = useState<NavItem[]>([]);
  const [epubFontSize, setEpubFontSize] = useState(100);
  const [tocOpen, setTocOpen] = useState(false);

  // Reading progress
  const scrollRef = useRef<HTMLDivElement>(null);
  const progressDebounce = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Markdown TOC
  const [mdToc, setMdToc] = useState<TocEntry[]>([]);
  const [mdTocOpen, setMdTocOpen] = useState(false);

  // Summaries
  const [summaries, setSummaries] = useState<Record<string, string>>({});
  const [summaryLength, setSummaryLength] = useState<string>("short");
  const [summaryLoading, setSummaryLoading] = useState(false);

  const hasNativeViewer = doc && NATIVE_VIEWER_FORMATS.includes(doc.original_format);
  const isEpub = doc?.original_format === "epub";
  const isMobi = doc?.original_format === "mobi";

  useEffect(() => {
    if (!id) return;
    async function load() {
      try {
        const document = await getDocument(id!);
        setDoc(document);
        if (document && NATIVE_VIEWER_FORMATS.includes(document.original_format)) {
          setActiveTab("original");
          if (document.original_format === "mobi") {
            const html = await getMobiHtml(id!).catch(() => null);
            setMobiHtml(html);
          } else {
            const bytes = await getOriginalBytes(id!).catch(() => null);
            if (bytes) setOriginalData(new Uint8Array(bytes));
          }
        }
        if (document) {
          setReadingStatus(document.reading_status);
        }
        const md = await getDocumentMarkdown(id!).catch(() => null);
        setMarkdown(md);
        getDocumentHasChunks(id!).then(setHasChunks).catch(() => {});
        getRelatedDocuments(id!).then(setRelated).catch(() => {});
        getDocumentToc(id!).then(setMdToc).catch(() => {});
        getAllSummaries(id!).then((pairs) => {
          const map: Record<string, string> = {};
          for (const [len, content] of pairs) map[len] = content;
          setSummaries(map);
        }).catch(() => {});
      } catch (e) {
        console.error("Failed to load document:", e);
      } finally {
        setLoading(false);
      }
    }
    load();
  }, [id]);

  const pdfBlobUrl = useMemo(() => {
    if (!originalData || doc?.original_format !== "pdf") return null;
    const blob = new Blob([originalData], { type: "application/pdf" });
    return URL.createObjectURL(blob);
  }, [originalData, doc?.original_format]);

  useEffect(() => {
    return () => { if (pdfBlobUrl) URL.revokeObjectURL(pdfBlobUrl); };
  }, [pdfBlobUrl]);

  const handleOpenOriginal = useCallback(async () => {
    if (!id) return;
    try {
      const path = await getOriginalPath(id);
      await openPath(path);
    } catch (e) {
      console.error("Failed to open original:", e);
    }
  }, [id]);

  const handleReadingStatus = useCallback(async (status: string | null) => {
    if (!id) return;
    setReadingStatus(status);
    await updateReadingStatus(id, status).catch(() => {});
  }, [id]);

  const handleDelete = async () => {
    if (!id) return;
    await deleteDocument(id);
    navigate("/");
  };

  const handleFontSize = useCallback((delta: number) => {
    setEpubFontSize((prev) => Math.min(160, Math.max(70, prev + delta)));
  }, []);

  // Reading progress: track scroll on the markdown reader
  const handleScroll = useCallback(() => {
    if (!scrollRef.current || !id) return;
    const el = scrollRef.current;
    const pos = el.scrollTop / Math.max(el.scrollHeight - el.clientHeight, 1);
    if (progressDebounce.current) clearTimeout(progressDebounce.current);
    progressDebounce.current = setTimeout(() => {
      saveReadingProgress(id, Math.min(1, Math.max(0, pos))).catch(() => {});
    }, 800);
  }, [id]);

  // Restore reading position
  useEffect(() => {
    if (!doc?.reading_progress || !scrollRef.current || activeTab !== "read") return;
    const el = scrollRef.current;
    // Wait for content to render
    const timer = setTimeout(() => {
      el.scrollTop = doc.reading_progress! * (el.scrollHeight - el.clientHeight);
    }, 200);
    return () => clearTimeout(timer);
  }, [doc?.reading_progress, markdown, activeTab]);

  // Save EPUB progress
  useEffect(() => {
    if (!id || !isEpub || epubLoc.percentage === 0) return;
    const pos = epubLoc.percentage / 100;
    if (progressDebounce.current) clearTimeout(progressDebounce.current);
    progressDebounce.current = setTimeout(() => {
      saveReadingProgress(id, pos).catch(() => {});
    }, 800);
  }, [id, isEpub, epubLoc.percentage]);

  const handleGenerateSummary = useCallback(async (length: string) => {
    if (!id) return;
    setSummaryLength(length);
    if (summaries[length]) return;
    setSummaryLoading(true);
    try {
      const content = await generateSummary(id, length);
      setSummaries((prev) => ({ ...prev, [length]: content }));
    } catch (e) {
      console.error("Summary generation failed:", e);
    } finally {
      setSummaryLoading(false);
    }
  }, [id, summaries]);

  if (loading) {
    return (
      <div className={css({ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", color: "text.muted" })}>
        <motion.div animate={{ opacity: [0.3, 1, 0.3] }} transition={{ duration: 1.5, repeat: Infinity }}>Loading...</motion.div>
      </div>
    );
  }

  if (!doc) {
    return (
      <div className={css({ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", color: "text.muted" })}>
        Document not found
      </div>
    );
  }

  const renderTocItems = (items: NavItem[], depth = 0): React.ReactNode =>
    items.map((item) => (
      <div key={item.id || item.href}>
        <button
          onClick={() => { epubRef.current?.navigateTo(item.href); setTocOpen(false); }}
          className={css({
            display: "block", width: "100%", textAlign: "left",
            bg: "transparent", border: "none",
            color: epubLoc.chapter === item.label?.trim() ? "accent.base" : "text.secondary",
            fontSize: "sm", fontFamily: "body",
            padding: "6px 16px", paddingLeft: `${16 + depth * 16}px`,
            cursor: "pointer", borderRadius: "sm", transition: "all 120ms",
            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
            _hover: { bg: "bg.hover", color: "text.primary" },
          } as any)}
        >
          {item.label?.trim()}
        </button>
        {item.subitems && renderTocItems(item.subitems, depth + 1)}
      </div>
    ));

  return (
    <div className={css({ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" })}>
      {/* Header */}
      <header
        className={css({
          padding: "lg", paddingTop: "48px", paddingBottom: "md",
          borderBottom: "1px solid", borderColor: "border.subtle",
          WebkitAppRegion: "drag",
        } as any)}
      >
        <div className={css({ display: "flex", alignItems: "flex-start", justifyContent: "space-between", WebkitAppRegion: "no-drag" } as any)}>
          <div>
            <button
              onClick={() => navigate(-1)}
              className={css({
                bg: "transparent", border: "none", color: "text.muted", fontSize: "sm",
                cursor: "pointer", padding: 0, marginBottom: "xs",
                display: "flex", alignItems: "center", gap: "xs",
                _hover: { color: "text.primary" },
              } as any)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
              </svg>
              Back
            </button>
            <h1 className={css({ fontSize: "xl", fontWeight: 700, letterSpacing: "-0.02em", color: "text.primary" })}>
              {doc.title || "Untitled"}
            </h1>
            <p className={css({ fontSize: "sm", color: "text.secondary", marginTop: "2px" })}>
              {doc.author || "Unknown author"}
            </p>
          </div>
          <div className={css({ display: "flex", gap: "xs", alignItems: "center" })}>
            {/* Reading status */}
            <select
              value={readingStatus || ""}
              onChange={(e) => handleReadingStatus(e.target.value || null)}
              className={css({
                bg: readingStatus ? "rgba(184,168,138,0.1)" : "rgba(255,255,255,0.04)",
                border: "none",
                color: readingStatus ? "accent.bright" : "text.muted",
                borderRadius: "md",
                padding: "4px 22px 4px 8px",
                fontSize: "11px",
                fontFamily: "body",
                cursor: "pointer",
                outline: "none",
                transition: "all 150ms",
                _hover: { bg: "rgba(255,255,255,0.08)" },
              } as any)}
              style={{
                appearance: "none",
                WebkitAppearance: "none",
                backgroundImage: `url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1l3 3 3-3' stroke='%23888' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`,
                backgroundRepeat: "no-repeat",
                backgroundPosition: "right 6px center",
              }}
            >
              <option value="">Not set</option>
              <option value="to_read">To Read</option>
              <option value="reading">Reading</option>
              <option value="read">Read</option>
            </select>
            <button
              onClick={() => setAskOpen(true)}
              className={css({
                bg: "accent.subtle", color: "accent.bright",
                border: "1px solid", borderColor: "accent.dim",
                borderRadius: "md", padding: "xs", paddingLeft: "sm", paddingRight: "sm",
                fontSize: "xs", fontWeight: 500, cursor: "pointer", transition: "all 150ms",
                display: "flex", alignItems: "center", gap: "xs",
                _hover: { bg: "accent.base", color: "text.inverse" },
              } as any)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                <path d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
              </svg>
              Ask
            </button>
            <button
              onClick={handleOpenOriginal}
              className={css({
                bg: "transparent", border: "1px solid", borderColor: "border.subtle", color: "text.muted",
                borderRadius: "md", padding: "xs", paddingLeft: "sm", paddingRight: "sm",
                fontSize: "xs", cursor: "pointer", transition: "all 150ms",
                _hover: { borderColor: "accent.dim", color: "text.primary" },
              } as any)}
            >
              Open Original
            </button>
            <button
              onClick={handleDelete}
              className={css({
                bg: "transparent", border: "1px solid", borderColor: "border.subtle", color: "text.muted",
                borderRadius: "md", padding: "xs", paddingLeft: "sm", paddingRight: "sm",
                fontSize: "xs", cursor: "pointer", transition: "all 150ms",
                _hover: { borderColor: "status.error", color: "status.error" },
              } as any)}
            >
              Delete
            </button>
          </div>
        </div>

        {/* Tabs */}
        <div className={css({ display: "flex", gap: "md", marginTop: "md", WebkitAppRegion: "no-drag" } as any)}>
          {([...(hasNativeViewer ? ["original" as const] : []), "read" as const, "info" as const]).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={css({
                bg: "transparent", border: "none",
                color: activeTab === tab ? "text.primary" : "text.muted",
                fontSize: "sm", fontWeight: 500, cursor: "pointer",
                padding: 0, paddingBottom: "xs",
                borderBottom: "2px solid",
                borderColor: activeTab === tab ? "accent.base" : "transparent",
                transition: "all 150ms", textTransform: "capitalize",
                _hover: { color: "text.primary" },
              } as any)}
            >
              {tab}
            </button>
          ))}
        </div>
      </header>

      {/* Content + Ask Drawer wrapper */}
      <div className={css({ flex: 1, overflow: "hidden", display: "flex", position: "relative" })}>
      <div
        ref={activeTab === "read" ? scrollRef : undefined}
        onScroll={activeTab === "read" ? handleScroll : undefined}
        className={css({
          flex: 1,
          overflow: activeTab === "original" ? "hidden" : "auto",
          padding: activeTab === "original" ? 0 : "lg",
          position: "relative",
        })}
      >
        {activeTab === "original" ? (
          <>
            {/* Viewer fills the area */}
            {doc.original_format === "pdf" && pdfBlobUrl ? (
              <object
                data={pdfBlobUrl}
                type="application/pdf"
                className={css({ width: "100%", height: "100%", border: "none", borderRadius: "md" })}
              >
                <div className={css({ color: "text.muted", textAlign: "center", paddingTop: "3xl" })}>
                  PDF preview not available.{" "}
                  <button onClick={handleOpenOriginal} className={css({ bg: "transparent", border: "none", color: "accent.base", cursor: "pointer", textDecoration: "underline", fontSize: "inherit", fontFamily: "inherit" })}>
                    Open in external viewer
                  </button>
                </div>
              </object>
            ) : isEpub && originalData ? (
              <EpubViewer
                ref={epubRef}
                data={originalData}
                fontSize={epubFontSize}
                onLocationChange={setEpubLoc}
                onTocLoaded={setEpubToc}
              />
            ) : isMobi && mobiHtml ? (
              <MobiViewer html={mobiHtml} />
            ) : (
              <div className={css({ color: "text.muted", textAlign: "center", paddingTop: "3xl" })}>
                Loading original...
              </div>
            )}

            {/* EPUB floating overlays — rendered as siblings OUTSIDE the epub iframe tree */}
            {isEpub && originalData && (
              <>
                {/* Progress pill — bottom center */}
                <div
                  className={css({
                    position: "absolute", bottom: "16px", left: "50%", transform: "translateX(-50%)",
                    display: "flex", alignItems: "center", gap: "10px",
                    bg: "rgba(10, 10, 12, 0.88)", backdropFilter: "blur(12px)",
                    border: "1px solid rgba(255,255,255,0.08)",
                    borderRadius: "full", padding: "6px 16px",
                    zIndex: 50, pointerEvents: "auto",
                  })}
                >
                  {epubLoc.chapter && (
                    <>
                      <span className={css({ fontSize: "xs", color: "text.secondary", maxWidth: "200px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" })}>
                        {epubLoc.chapter}
                      </span>
                      <span className={css({ width: "1px", height: "12px", bg: "border.base", flexShrink: 0 })} />
                    </>
                  )}
                  <div className={css({ width: "80px", height: "3px", bg: "rgba(255,255,255,0.08)", borderRadius: "full", overflow: "hidden", flexShrink: 0 })}>
                    <motion.div
                      animate={{ width: `${epubLoc.percentage}%` }}
                      transition={{ type: "spring", stiffness: 300, damping: 30 }}
                      className={css({ height: "100%", bg: "accent.dim", borderRadius: "full" })}
                    />
                  </div>
                  <span className={css({ fontSize: "xs", color: "text.muted", fontFamily: "mono", fontVariantNumeric: "tabular-nums", minWidth: "28px", textAlign: "right" })}>
                    {epubLoc.percentage}%
                  </span>
                </div>

                {/* Controls — bottom right */}
                <div
                  className={css({
                    position: "absolute", bottom: "16px", right: "16px",
                    display: "flex", alignItems: "center", gap: "2px",
                    bg: "rgba(10, 10, 12, 0.88)", backdropFilter: "blur(12px)",
                    border: "1px solid rgba(255,255,255,0.08)",
                    borderRadius: "lg", padding: "2px",
                    zIndex: 50, pointerEvents: "auto",
                  })}
                >
                  <ControlButton onClick={() => handleFontSize(-10)} title="Decrease font size">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><path d="M5 12h14" /></svg>
                  </ControlButton>
                  <span className={css({ fontSize: "xs", color: "text.muted", fontFamily: "mono", minWidth: "32px", textAlign: "center" })}>
                    {epubFontSize}%
                  </span>
                  <ControlButton onClick={() => handleFontSize(10)} title="Increase font size">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><path d="M12 5v14M5 12h14" /></svg>
                  </ControlButton>
                  <span className={css({ width: "1px", height: "16px", bg: "border.base", margin: "0 2px" })} />
                  <ControlButton onClick={() => setTocOpen((v) => !v)} active={tocOpen} title="Table of contents">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><path d="M3 12h18M3 6h18M3 18h18" /></svg>
                  </ControlButton>
                </div>

                {/* TOC drawer */}
                <AnimatePresence>
                  {tocOpen && (
                    <motion.div
                      initial={{ opacity: 0, x: 40 }}
                      animate={{ opacity: 1, x: 0 }}
                      exit={{ opacity: 0, x: 40 }}
                      transition={{ type: "spring", stiffness: 400, damping: 30 }}
                      className={css({
                        position: "absolute", top: 0, right: 0, bottom: 0, width: "280px",
                        bg: "rgba(10, 10, 12, 0.92)", backdropFilter: "blur(16px)",
                        borderLeft: "1px solid rgba(255,255,255,0.06)",
                        display: "flex", flexDirection: "column",
                        zIndex: 60, overflow: "hidden",
                      })}
                    >
                      <div className={css({ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "12px 16px", borderBottom: "1px solid rgba(255,255,255,0.06)" })}>
                        <span className={css({ fontSize: "xs", fontWeight: 600, color: "text.muted", textTransform: "uppercase", letterSpacing: "0.05em" })}>
                          Contents
                        </span>
                        <ControlButton onClick={() => setTocOpen(false)} title="Close">
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><path d="M18 6L6 18M6 6l12 12" /></svg>
                        </ControlButton>
                      </div>
                      <div className={css({ flex: 1, overflow: "auto", padding: "8px 0" })}>
                        {renderTocItems(epubToc)}
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </>
            )}
          </>
        ) : activeTab === "read" ? (
          <>
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className={`selectable ${css({
                maxWidth: "720px", margin: "0 auto", fontFamily: "reading",
                fontSize: "md", lineHeight: 1.8, color: "text.primary",
                "& h1": { fontSize: "2xl", fontWeight: 700, marginTop: "xl", marginBottom: "md", fontFamily: "heading", letterSpacing: "-0.02em" },
                "& h2": { fontSize: "xl", fontWeight: 600, marginTop: "xl", marginBottom: "sm", fontFamily: "heading" },
                "& h3": { fontSize: "lg", fontWeight: 600, marginTop: "lg", marginBottom: "sm", fontFamily: "heading" },
                "& p": { marginBottom: "md" },
                "& ul, & ol": { paddingLeft: "lg", marginBottom: "md" },
                "& li": { marginBottom: "xs" },
                "& blockquote": { borderLeft: "3px solid", borderColor: "accent.dim", paddingLeft: "md", color: "text.secondary", fontStyle: "italic", margin: "md 0" },
                "& code": { fontFamily: "mono", fontSize: "sm", bg: "bg.elevated", padding: "2px 6px", borderRadius: "sm" },
                "& pre": { bg: "bg.surface", border: "1px solid", borderColor: "border.subtle", borderRadius: "md", padding: "md", overflow: "auto", marginBottom: "md" },
                "& pre code": { bg: "transparent", padding: 0 },
                "& hr": { border: "none", borderTop: "1px solid", borderColor: "border.subtle", margin: "xl 0" },
                "& a": { color: "accent.base", textDecoration: "none", _hover: { textDecoration: "underline" } },
                "& table": { width: "100%", borderCollapse: "collapse", marginBottom: "md" },
                "& th, & td": { border: "1px solid", borderColor: "border.subtle", padding: "sm", textAlign: "left", fontSize: "sm" },
                "& th": { bg: "bg.surface", fontWeight: 600 },
              } as any)}`}
            >
              {markdown ? (
                <ReactMarkdown
                  components={{
                    h1: ({ children, ...props }) => {
                      const text = typeof children === "string" ? children : String(children);
                      const slug = text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
                      return <h1 id={slug} {...props}>{children}</h1>;
                    },
                    h2: ({ children, ...props }) => {
                      const text = typeof children === "string" ? children : String(children);
                      const slug = text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
                      return <h2 id={slug} {...props}>{children}</h2>;
                    },
                    h3: ({ children, ...props }) => {
                      const text = typeof children === "string" ? children : String(children);
                      const slug = text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
                      return <h3 id={slug} {...props}>{children}</h3>;
                    },
                  }}
                >{markdown}</ReactMarkdown>
              ) : doc.status === "processing" ? (
                <div className={css({ color: "text.muted", textAlign: "center", paddingTop: "3xl" })}>
                  <motion.div animate={{ opacity: [0.3, 1, 0.3] }} transition={{ duration: 1.5, repeat: Infinity }}>Processing with Claude...</motion.div>
                </div>
              ) : (
                <div className={css({ color: "text.muted", textAlign: "center", paddingTop: "3xl" })}>No content available</div>
              )}
            </motion.div>

            {/* Markdown TOC button */}
            {mdToc.length > 0 && (
              <div
                className={css({
                  position: "absolute", bottom: "16px", right: "16px",
                  display: "flex", alignItems: "center", gap: "2px",
                  bg: "rgba(10, 10, 12, 0.88)", backdropFilter: "blur(12px)",
                  border: "1px solid rgba(255,255,255,0.08)",
                  borderRadius: "lg", padding: "2px",
                  zIndex: 50,
                })}
              >
                <ControlButton onClick={() => setMdTocOpen((v) => !v)} active={mdTocOpen} title="Table of contents">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><path d="M3 12h18M3 6h18M3 18h18" /></svg>
                </ControlButton>
              </div>
            )}

            {/* Markdown TOC drawer */}
            <AnimatePresence>
              {mdTocOpen && mdToc.length > 0 && (
                <motion.div
                  initial={{ opacity: 0, x: 40 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: 40 }}
                  transition={{ type: "spring", stiffness: 400, damping: 30 }}
                  className={css({
                    position: "absolute", top: 0, right: 0, bottom: 0, width: "280px",
                    bg: "rgba(10, 10, 12, 0.92)", backdropFilter: "blur(16px)",
                    borderLeft: "1px solid rgba(255,255,255,0.06)",
                    display: "flex", flexDirection: "column",
                    zIndex: 60, overflow: "hidden",
                  })}
                >
                  <div className={css({ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "12px 16px", borderBottom: "1px solid rgba(255,255,255,0.06)" })}>
                    <span className={css({ fontSize: "xs", fontWeight: 600, color: "text.muted", textTransform: "uppercase", letterSpacing: "0.05em" })}>
                      Contents
                    </span>
                    <ControlButton onClick={() => setMdTocOpen(false)} title="Close">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><path d="M18 6L6 18M6 6l12 12" /></svg>
                    </ControlButton>
                  </div>
                  <div className={css({ flex: 1, overflow: "auto", padding: "8px 0" })}>
                    {mdToc.map((entry, i) => (
                      <button
                        key={i}
                        onClick={() => {
                          if (entry.href) {
                            const el = document.getElementById(entry.href);
                            el?.scrollIntoView({ behavior: "smooth", block: "start" });
                          }
                          setMdTocOpen(false);
                        }}
                        className={css({
                          display: "block", width: "100%", textAlign: "left",
                          bg: "transparent", border: "none",
                          color: "text.secondary",
                          fontSize: "sm", fontFamily: "body",
                          padding: "6px 16px",
                          cursor: "pointer", borderRadius: "sm", transition: "all 120ms",
                          overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                          _hover: { bg: "bg.hover", color: "text.primary" },
                        } as any)}
                        style={{ paddingLeft: `${16 + (entry.level - 1) * 16}px` }}
                      >
                        {entry.title}
                      </button>
                    ))}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </>
        ) : (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className={css({ maxWidth: "560px", display: "flex", flexDirection: "column", gap: "md" })}
          >
            {[
              ["Format", doc.original_format.toUpperCase()],
              ["Size", formatFileSize(doc.file_size)],
              ["Duration", doc.duration_seconds ? formatDuration(doc.duration_seconds) : null],
              ["Source", doc.source_url],
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
                <div key={label} className={css({ display: "flex", alignItems: "baseline", gap: "md", padding: "sm 0", borderBottom: "1px solid", borderColor: "border.subtle" })}>
                  <span className={css({ fontSize: "sm", color: "text.muted", width: "100px", flexShrink: 0 })}>{label}</span>
                  <span className={css({ fontSize: "sm", color: "text.primary", fontFamily: label === "Hash" ? "mono" : "body", wordBreak: "break-all" })}>{value}</span>
                </div>
              ))}
            {doc.description && (
              <div className={css({ marginTop: "md" })}>
                <span className={css({ fontSize: "sm", color: "text.muted" })}>Description</span>
                <p className={css({ fontSize: "sm", color: "text.secondary", marginTop: "xs", lineHeight: 1.6 })}>{doc.description}</p>
              </div>
            )}
            {doc.tags.length > 0 && (
              <div className={css({ marginTop: "sm" })}>
                <span className={css({ fontSize: "sm", color: "text.muted" })}>Tags</span>
                <div className={css({ display: "flex", flexWrap: "wrap", gap: "xs", marginTop: "xs" })}>
                  {doc.tags.map((tag) => (
                    <span key={tag} className={css({ fontSize: "xs", color: "accent.base", bg: "accent.subtle", borderRadius: "full", padding: "2px 10px" })}>{tag}</span>
                  ))}
                </div>
              </div>
            )}
            {/* Summaries */}
            <div className={css({ marginTop: "lg" })}>
              <span className={css({ fontSize: "sm", color: "text.muted" })}>Summary</span>
              <div className={css({ display: "flex", gap: "2px", marginTop: "sm", bg: "bg.surface", borderRadius: "md", padding: "2px", width: "fit-content" })}>
                {(["short", "medium", "long"] as const).map((len) => (
                  <button
                    key={len}
                    onClick={() => handleGenerateSummary(len)}
                    className={css({
                      bg: summaryLength === len ? "bg.elevated" : "transparent",
                      border: "none",
                      color: summaryLength === len ? "text.primary" : "text.muted",
                      fontSize: "xs", fontWeight: 500,
                      padding: "4px 12px", borderRadius: "sm",
                      cursor: "pointer", transition: "all 150ms",
                      textTransform: "capitalize",
                      _hover: { color: "text.primary" },
                    } as any)}
                  >
                    {len}
                    {summaries[len] && <span className={css({ color: "accent.dim" })}> &#x2713;</span>}
                  </button>
                ))}
              </div>
              {summaryLoading && !summaries[summaryLength] && (
                <motion.div
                  animate={{ opacity: [0.3, 1, 0.3] }}
                  transition={{ duration: 1.5, repeat: Infinity }}
                  className={css({ color: "text.muted", fontSize: "sm", marginTop: "sm" })}
                >
                  Generating summary...
                </motion.div>
              )}
              {summaries[summaryLength] && (
                <div className={css({ marginTop: "sm" })}>
                  <ReactMarkdown
                    components={{
                      p: ({ children }) => <p className={css({ fontSize: "sm", color: "text.secondary", lineHeight: 1.6, marginBottom: "sm" })}>{children}</p>,
                    }}
                  >
                    {summaries[summaryLength]}
                  </ReactMarkdown>
                </div>
              )}
              {!summaryLoading && !summaries[summaryLength] && (
                <p className={css({ fontSize: "xs", color: "text.muted", marginTop: "sm" })}>
                  Click a length to generate a summary with Claude
                </p>
              )}
            </div>

            {related.length > 0 && (
              <div className={css({ marginTop: "lg" })}>
                <span className={css({ fontSize: "sm", color: "text.muted" })}>Similar Documents</span>
                <div className={css({ display: "flex", flexDirection: "column", gap: "xs", marginTop: "sm" })}>
                  {related.map((r) => (
                    <div
                      key={r.id}
                      onClick={() => navigate(`/document/${r.id}`)}
                      className={css({
                        display: "flex",
                        alignItems: "baseline",
                        gap: "sm",
                        padding: "sm",
                        borderRadius: "md",
                        cursor: "pointer",
                        transition: "all 150ms",
                        _hover: { bg: "bg.hover" },
                      } as any)}
                    >
                      <span className={css({ fontSize: "sm", color: "text.primary", flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" })}>
                        {r.title || "Untitled"}
                      </span>
                      <span className={css({ fontSize: "xs", color: "text.muted" })}>
                        {r.author}
                      </span>
                      <span className={css({ fontSize: "xs", color: "text.muted", fontFamily: "mono", textTransform: "uppercase" })}>
                        {r.original_format}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </motion.div>
        )}
      </div>

      {/* Ask Panel */}
      <ChatPanel
        mode="document"
        documentId={id!}
        documentTitle={doc.title || "Untitled"}
        hasChunks={hasChunks}
        isOpen={askOpen}
        onClose={() => setAskOpen(false)}
      />
      </div>
    </div>
  );
}
