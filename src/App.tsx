import { Routes, Route } from "react-router-dom";
import { css } from "../styled-system/css";
import { Sidebar } from "./components/Sidebar";
import { Library } from "./pages/Library";
import { Import } from "./pages/Import";
import { DocumentView } from "./pages/DocumentView";
import { Search } from "./pages/Search";
import { Settings } from "./pages/Settings";

function App() {
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
          <Route path="/import" element={<Import />} />
          <Route path="/search" element={<Search />} />
          <Route path="/document/:id" element={<DocumentView />} />
          <Route path="/settings" element={<Settings />} />
        </Routes>
      </main>
    </div>
  );
}

export default App;
