import { useState, useCallback, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion, AnimatePresence } from "framer-motion";
import { searchDocuments, searchSemantic } from "../lib/api";
import type { SearchResult } from "../lib/api";

type SearchMode = "keyword" | "semantic";

export function Search() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searched, setSearched] = useState(false);
  const [searching, setSearching] = useState(false);
  const [mode, setMode] = useState<SearchMode>("keyword");
  const navigate = useNavigate();
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const doSearch = useCallback(
    (value: string, searchMode: SearchMode) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);

      if (value.trim().length < 2) {
        setResults([]);
        setSearched(false);
        return;
      }

      const delay = searchMode === "semantic" ? 500 : 250;

      debounceRef.current = setTimeout(async () => {
        setSearching(true);
        try {
          const res =
            searchMode === "semantic"
              ? await searchSemantic(value.trim())
              : await searchDocuments(value.trim());
          setResults(res);
          setSearched(true);
        } catch (e) {
          console.error("Search failed:", e);
        } finally {
          setSearching(false);
        }
      }, delay);
    },
    []
  );

  const handleSearch = useCallback(
    (value: string) => {
      setQuery(value);
      doSearch(value, mode);
    },
    [mode, doSearch]
  );

  const handleModeChange = useCallback(
    (newMode: SearchMode) => {
      setMode(newMode);
      if (query.trim().length >= 2) {
        doSearch(query, newMode);
      }
    },
    [query, doSearch]
  );

  return (
    <div
      className={css({
        flex: 1,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      })}
    >
      {/* Header with search input */}
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
        <div className={css({ WebkitAppRegion: "no-drag" } as any)}>
          <div
            className={css({
              display: "flex",
              alignItems: "center",
              gap: "sm",
              bg: "bg.surface",
              border: "1px solid",
              borderColor: "border.base",
              borderRadius: "lg",
              padding: "sm",
              paddingLeft: "md",
              transition: "border-color 200ms",
              _focusWithin: {
                borderColor: "accent.dim",
              },
            } as any)}
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
              type="text"
              placeholder={
                mode === "semantic"
                  ? "Describe what you're looking for..."
                  : "Search your archive..."
              }
              value={query}
              onChange={(e) => handleSearch(e.target.value)}
              autoFocus
              className={css({
                flex: 1,
                bg: "transparent",
                border: "none",
                outline: "none",
                color: "text.primary",
                fontSize: "md",
                fontFamily: "body",
                _placeholder: {
                  color: "text.muted",
                },
              } as any)}
            />
            {searching && (
              <motion.div
                animate={{ rotate: 360 }}
                transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
                className={css({ color: "text.muted" })}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                  <path d="M12 2v4m0 12v4m-7.071-15.071l2.828 2.828m8.486 8.486l2.828 2.828M2 12h4m12 0h4M4.929 19.071l2.828-2.828m8.486-8.486l2.828-2.828" />
                </svg>
              </motion.div>
            )}
          </div>

          {/* Mode toggle */}
          <div
            className={css({
              display: "flex",
              gap: "2px",
              marginTop: "sm",
              bg: "bg.surface",
              borderRadius: "md",
              padding: "2px",
              width: "fit-content",
            })}
          >
            {(["keyword", "semantic"] as const).map((m) => (
              <button
                key={m}
                onClick={() => handleModeChange(m)}
                className={css({
                  bg: mode === m ? "bg.elevated" : "transparent",
                  border: "none",
                  color: mode === m ? "text.primary" : "text.muted",
                  fontSize: "xs",
                  fontWeight: 500,
                  padding: "4px 12px",
                  borderRadius: "sm",
                  cursor: "pointer",
                  transition: "all 150ms",
                  textTransform: "capitalize",
                  _hover: { color: "text.primary" },
                } as any)}
              >
                {m === "keyword" ? "Keyword" : "Semantic"}
              </button>
            ))}
          </div>
        </div>
      </header>

      {/* Results */}
      <div
        className={css({
          flex: 1,
          overflow: "auto",
          padding: "lg",
        })}
      >
        <AnimatePresence mode="wait">
          {!searched ? (
            <motion.div
              key="empty"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className={css({
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                height: "100%",
                color: "text.muted",
                fontSize: "sm",
              })}
            >
              {mode === "semantic"
                ? "Find documents by meaning, not just keywords"
                : "Search across all your documents, metadata, and content"}
            </motion.div>
          ) : results.length === 0 ? (
            <motion.div
              key="no-results"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className={css({
                display: "flex",
                justifyContent: "center",
                paddingTop: "3xl",
                color: "text.muted",
                fontSize: "sm",
              })}
            >
              No results found
            </motion.div>
          ) : (
            <motion.div
              key="results"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className={css({
                display: "flex",
                flexDirection: "column",
                gap: "sm",
              })}
            >
              {results.map((result, i) => (
                <motion.div
                  key={result.id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: i * 0.03 }}
                  onClick={() => navigate(`/document/${result.id}`)}
                  className={css({
                    padding: "md",
                    bg: "bg.surface",
                    borderRadius: "md",
                    border: "1px solid",
                    borderColor: "border.subtle",
                    cursor: "pointer",
                    transition: "all 150ms",
                    _hover: {
                      borderColor: "border.strong",
                      bg: "bg.elevated",
                    },
                  } as any)}
                >
                  <div
                    className={css({
                      display: "flex",
                      alignItems: "baseline",
                      gap: "sm",
                    })}
                  >
                    <h3
                      className={css({
                        fontSize: "md",
                        fontWeight: 600,
                        color: "text.primary",
                      })}
                    >
                      {result.title || "Untitled"}
                    </h3>
                    <span
                      className={css({
                        fontSize: "xs",
                        color: "text.muted",
                        fontFamily: "mono",
                        textTransform: "uppercase",
                      })}
                    >
                      {result.original_format}
                    </span>
                  </div>
                  <p
                    className={css({
                      fontSize: "sm",
                      color: "text.secondary",
                      marginTop: "2px",
                    })}
                  >
                    {result.author || "Unknown author"}
                  </p>
                  <p
                    className={css({
                      fontSize: "sm",
                      color: "text.muted",
                      marginTop: "sm",
                      lineHeight: 1.5,
                      "& mark": {
                        bg: "accent.subtle",
                        color: "accent.bright",
                        borderRadius: "2px",
                        padding: "0 2px",
                      },
                    } as any)}
                    dangerouslySetInnerHTML={{ __html: result.snippet }}
                  />
                </motion.div>
              ))}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
