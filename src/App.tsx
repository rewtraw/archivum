import { useState, useEffect, useCallback } from "react";
import { Routes, Route, useNavigate } from "react-router-dom";
import { css } from "../styled-system/css";
import { Sidebar } from "./components/Sidebar";
import { Library } from "./pages/Library";
import { DocumentView } from "./pages/DocumentView";
import { Search } from "./pages/Search";
import { Settings } from "./pages/Settings";
import { Chat } from "./pages/Chat";
import { QuickSearch } from "./components/QuickSearch";

function App() {
  const [quickSearchOpen, setQuickSearchOpen] = useState(false);
  const navigate = useNavigate();

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Cmd+K: Quick search
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setQuickSearchOpen((open) => !open);
      }
      // Cmd+I: Import
      if ((e.metaKey || e.ctrlKey) && e.key === "i") {
        e.preventDefault();
        navigate("/?import=true");
      }
    },
    [navigate]
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  return (
    <div
      className={css({
        display: "flex",
        height: "100vh",
        width: "100vw",
        overflow: "hidden",
      })}
    >
      <Sidebar />
      <main
        className={css({
          flex: 1,
          overflow: "hidden",
          display: "flex",
          flexDirection: "column",
        })}
      >
        <Routes>
          <Route path="/" element={<Library />} />
          <Route path="/search" element={<Search />} />
          <Route path="/document/:id" element={<DocumentView />} />
          <Route path="/chat" element={<Chat />} />
          <Route path="/settings" element={<Settings />} />
        </Routes>
      </main>
      <QuickSearch
        open={quickSearchOpen}
        onClose={() => setQuickSearchOpen(false)}
      />
    </div>
  );
}

export default App;
