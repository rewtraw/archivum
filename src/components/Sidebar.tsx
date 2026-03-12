import { NavLink } from "react-router-dom";
import { css } from "../../styled-system/css";
import { motion } from "framer-motion";

const navItems = [
  { path: "/", label: "Library", icon: "M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" },
  { path: "/search", label: "Search", icon: "M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" },
  { path: "/chat", label: "Chat", icon: "M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" },
];

export function Sidebar() {
  return (
    <nav
      className={css({
        width: "220px",
        bg: "bg.base",
        borderRight: "1px solid",
        borderColor: "border.subtle",
        display: "flex",
        flexDirection: "column",
        padding: "md",
        gap: "xs",
        // macOS traffic light space
        paddingTop: "48px",
        WebkitAppRegion: "drag",
      } as any)}
    >
      <div
        className={css({
          fontSize: "lg",
          fontWeight: 600,
          color: "accent.base",
          letterSpacing: "-0.02em",
          marginBottom: "lg",
          paddingLeft: "sm",
          WebkitAppRegion: "drag",
        } as any)}
      >
        Archivum
      </div>

      <div className={css({ display: "flex", flexDirection: "column", gap: "xs", WebkitAppRegion: "no-drag" } as any)}>
        {navItems.map((item) => (
          <NavLink
            key={item.path}
            to={item.path}
            className={({ isActive }) =>
              css({
                display: "flex",
                alignItems: "center",
                gap: "sm",
                padding: "sm",
                paddingLeft: "sm",
                borderRadius: "md",
                textDecoration: "none",
                fontSize: "sm",
                fontWeight: 500,
                color: isActive ? "text.primary" : "text.secondary",
                bg: isActive ? "bg.elevated" : "transparent",
                transition: "all 200ms",
                cursor: "pointer",
                _hover: {
                  bg: "bg.hover",
                  color: "text.primary",
                },
              } as any)
            }
          >
            <svg
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={1.5}
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d={item.icon} />
            </svg>
            {item.label}
          </NavLink>
        ))}
      </div>

      <div className={css({ flex: 1 })} />

      <div className={css({ WebkitAppRegion: "no-drag" } as any)}>
        <NavLink
          to="/settings"
          className={({ isActive }) =>
            css({
              display: "flex",
              alignItems: "center",
              gap: "sm",
              padding: "sm",
              paddingLeft: "sm",
              borderRadius: "md",
              textDecoration: "none",
              fontSize: "sm",
              fontWeight: 500,
              color: isActive ? "text.primary" : "text.muted",
              bg: isActive ? "bg.elevated" : "transparent",
              transition: "all 200ms",
              cursor: "pointer",
              _hover: {
                bg: "bg.hover",
                color: "text.primary",
              },
            } as any)
          }
        >
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.241-.438.613-.431.992a7.723 7.723 0 010 .255c-.007.378.138.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 010-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28z" />
            <path d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
          Settings
        </NavLink>
      </div>

      <div
        className={css({
          fontSize: "xs",
          color: "text.muted",
          paddingLeft: "sm",
          paddingBottom: "sm",
          paddingTop: "xs",
          WebkitAppRegion: "no-drag",
        } as any)}
      >
        v0.1.0
      </div>
    </nav>
  );
}
