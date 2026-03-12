import { useState, useEffect, useRef, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import { searchDocuments } from "../lib/api";
import type { SearchResult } from "../lib/api";

interface QuickSearchProps {
  open: boolean;
  onClose: () => void;
}

export function QuickSearch({ open, onClose }: QuickSearchProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [searching, setSearching] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const navigate = useNavigate();

  useEffect(() => {
    if (open) {
      setQuery("");
      setResults([]);
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  const doSearch = useCallback((value: string) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (value.trim().length < 2) {
      setResults([]);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      setSearching(true);
      try {
        const res = await searchDocuments(value.trim(), 8);
        setResults(res);
        setSelectedIndex(0);
      } catch {
        // ignore
      } finally {
        setSearching(false);
      }
    }, 150);
  }, []);

  const handleSelect = useCallback(
    (id: string) => {
      onClose();
      navigate(`/document/${id}`);
    },
    [onClose, navigate]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((i) => Math.max(i - 1, 0));
          break;
        case "Enter":
          if (results[selectedIndex]) {
            handleSelect(results[selectedIndex].id);
          }
          break;
        case "Escape":
          onClose();
          break;
      }
    },
    [results, selectedIndex, handleSelect, onClose]
  );

  if (!open) return null;

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className={css({
          position: "fixed",
          inset: 0,
          zIndex: 200,
          display: "flex",
          alignItems: "flex-start",
          justifyContent: "center",
          paddingTop: "120px",
          bg: "rgba(0, 0, 0, 0.5)",
          backdropFilter: "blur(4px)",
        })}
        onClick={(e) => {
          if (e.target === e.currentTarget) onClose();
        }}
      >
        <motion.div
          initial={{ opacity: 0, scale: 0.96, y: -8 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.96, y: -8 }}
          transition={{ duration: 0.15 }}
          className={css({
            width: "560px",
            maxHeight: "460px",
            bg: "bg.surface",
            border: "1px solid",
            borderColor: "border.base",
            borderRadius: "xl",
            overflow: "hidden",
            boxShadow: "0 25px 50px rgba(0,0,0,0.4)",
          })}
          onKeyDown={handleKeyDown}
        >
          {/* Search input */}
          <div
            className={css({
              display: "flex",
              alignItems: "center",
              gap: "sm",
              padding: "md",
              borderBottom: "1px solid",
              borderColor: "border.subtle",
            })}
          >
            <svg
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={1.5}
              className={css({ color: "text.muted", flexShrink: 0 })}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"
              />
            </svg>
            <input
              ref={inputRef}
              type="text"
              placeholder="Search your library..."
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                doSearch(e.target.value);
              }}
              className={css({
                flex: 1,
                bg: "transparent",
                border: "none",
                outline: "none",
                color: "text.primary",
                fontSize: "md",
                fontFamily: "body",
                _placeholder: { color: "text.muted" },
              } as any)}
            />
            <kbd
              className={css({
                fontSize: "xs",
                color: "text.muted",
                bg: "bg.elevated",
                padding: "1px 6px",
                borderRadius: "sm",
                border: "1px solid",
                borderColor: "border.subtle",
                fontFamily: "mono",
              })}
            >
              esc
            </kbd>
          </div>

          {/* Results */}
          <div className={css({ overflow: "auto", maxHeight: "380px" })}>
            {results.length === 0 && query.length >= 2 && !searching && (
              <div
                className={css({
                  padding: "lg",
                  textAlign: "center",
                  color: "text.muted",
                  fontSize: "sm",
                })}
              >
                No results found
              </div>
            )}
            {results.length === 0 && query.length < 2 && (
              <div
                className={css({
                  padding: "lg",
                  textAlign: "center",
                  color: "text.muted",
                  fontSize: "sm",
                })}
              >
                Type to search across your documents
              </div>
            )}
            {results.map((result, i) => (
              <div
                key={result.id}
                onClick={() => handleSelect(result.id)}
                onMouseEnter={() => setSelectedIndex(i)}
                className={css({
                  display: "flex",
                  alignItems: "center",
                  gap: "sm",
                  padding: "sm",
                  paddingLeft: "md",
                  paddingRight: "md",
                  cursor: "pointer",
                  transition: "background 100ms",
                })}
                style={{
                  background:
                    i === selectedIndex
                      ? "var(--colors-bg-elevated)"
                      : "transparent",
                }}
              >
                <div className={css({ flex: 1, minWidth: 0 })}>
                  <p
                    className={css({
                      fontSize: "sm",
                      fontWeight: 500,
                      color: "text.primary",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    })}
                  >
                    {result.title || "Untitled"}
                  </p>
                  <p
                    className={css({
                      fontSize: "xs",
                      color: "text.muted",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    })}
                  >
                    {result.author || "Unknown"} &middot;{" "}
                    {result.original_format.toUpperCase()}
                  </p>
                </div>
                {i === selectedIndex && (
                  <kbd
                    className={css({
                      fontSize: "xs",
                      color: "text.muted",
                      fontFamily: "mono",
                    })}
                  >
                    &crarr;
                  </kbd>
                )}
              </div>
            ))}
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
