import { useState } from "react";
import { css } from "../../styled-system/css";
import { ChatPanel } from "../components/ChatPanel";

export function Chat() {
  const [isOpen, setIsOpen] = useState(true);

  return (
    <div
      className={css({
        flex: 1,
        display: "flex",
        flexDirection: "column",
        position: "relative",
        overflow: "hidden",
      })}
    >
      {!isOpen && (
        <div
          className={css({
            flex: 1,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: "md",
            paddingTop: "48px",
          })}
        >
          <svg
            width="48"
            height="48"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={1}
            className={css({ color: "text.muted", opacity: 0.4 })}
          >
            <path d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
          </svg>
          <p className={css({ fontSize: "sm", color: "text.muted" })}>
            Chat with your entire library
          </p>
          <button
            onClick={() => setIsOpen(true)}
            className={css({
              bg: "accent.subtle",
              color: "accent.bright",
              border: "1px solid",
              borderColor: "accent.dim",
              borderRadius: "md",
              padding: "sm",
              paddingLeft: "lg",
              paddingRight: "lg",
              fontSize: "sm",
              fontWeight: 500,
              cursor: "pointer",
              transition: "all 150ms",
              _hover: { bg: "accent.base", color: "text.inverse" },
            } as any)}
          >
            Start chatting
          </button>
        </div>
      )}

      <ChatPanel
        mode="library"
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
        fullHeight
      />
    </div>
  );
}
