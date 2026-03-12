import { useState, useEffect } from "react";
import { css } from "../../styled-system/css";
import { motion } from "framer-motion";
import { getSettings, saveSettings, validateApiKey } from "../lib/api";
import type { Settings as SettingsType } from "../lib/api";

const MODELS = [
  { id: "claude-sonnet-4-20250514", label: "Claude Sonnet 4", description: "Fast, great for most documents" },
  { id: "claude-opus-4-20250514", label: "Claude Opus 4", description: "Most capable, best for complex documents" },
  { id: "claude-haiku-3-5-20241022", label: "Claude Haiku 3.5", description: "Fastest, good for simple documents" },
];

export function Settings() {
  const [settings, setSettings] = useState<SettingsType | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [showKeyInput, setShowKeyInput] = useState(false);
  const [validating, setValidating] = useState(false);
  const [validationResult, setValidationResult] = useState<
    "idle" | "valid" | "invalid"
  >("idle");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings);
  }, []);

  const handleValidateAndSave = async () => {
    if (!apiKey.trim()) return;

    setValidating(true);
    setValidationResult("idle");

    try {
      const valid = await validateApiKey(apiKey.trim());
      setValidationResult(valid ? "valid" : "invalid");

      if (valid) {
        setSaving(true);
        const updated = await saveSettings(apiKey.trim());
        setSettings(updated);
        setShowKeyInput(false);
        setApiKey("");
        setSaved(true);
        setTimeout(() => setSaved(false), 2000);
      }
    } catch {
      setValidationResult("invalid");
    } finally {
      setValidating(false);
      setSaving(false);
    }
  };

  const handleRemoveKey = async () => {
    const updated = await saveSettings("");
    setSettings(updated);
  };

  const handleModelChange = async (model: string) => {
    const updated = await saveSettings(undefined, model);
    setSettings(updated);
  };

  if (!settings) return null;

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
          Settings
        </h1>
      </header>

      <div
        className={css({
          flex: 1,
          overflow: "auto",
          padding: "lg",
          maxWidth: "600px",
        })}
      >
        {/* API Key Section */}
        <section className={css({ marginBottom: "2xl" })}>
          <h2
            className={css({
              fontSize: "md",
              fontWeight: 600,
              color: "text.primary",
              marginBottom: "xs",
            })}
          >
            Anthropic API Key
          </h2>
          <p
            className={css({
              fontSize: "sm",
              color: "text.muted",
              marginBottom: "md",
              lineHeight: 1.5,
            })}
          >
            Required for document extraction. Get your key from{" "}
            <span className={css({ color: "accent.base" })}>
              console.anthropic.com
            </span>
          </p>

          {settings.has_api_key && !showKeyInput ? (
            <div
              className={css({
                display: "flex",
                alignItems: "center",
                gap: "md",
              })}
            >
              <div
                className={css({
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  gap: "sm",
                  padding: "sm",
                  paddingLeft: "md",
                  bg: "bg.surface",
                  border: "1px solid",
                  borderColor: "border.base",
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
                  className={css({ color: "status.success", flexShrink: 0 })}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                  />
                </svg>
                <span
                  className={css({
                    fontSize: "sm",
                    fontFamily: "mono",
                    color: "text.secondary",
                  })}
                >
                  {settings.api_key_preview}
                </span>
              </div>
              <button
                onClick={() => setShowKeyInput(true)}
                className={css({
                  bg: "transparent",
                  border: "1px solid",
                  borderColor: "border.base",
                  color: "text.secondary",
                  borderRadius: "md",
                  padding: "sm",
                  paddingLeft: "md",
                  paddingRight: "md",
                  fontSize: "sm",
                  cursor: "pointer",
                  transition: "all 150ms",
                  whiteSpace: "nowrap",
                  _hover: {
                    borderColor: "border.strong",
                    color: "text.primary",
                  },
                } as any)}
              >
                Change
              </button>
              <button
                onClick={handleRemoveKey}
                className={css({
                  bg: "transparent",
                  border: "1px solid",
                  borderColor: "border.subtle",
                  color: "text.muted",
                  borderRadius: "md",
                  padding: "sm",
                  paddingLeft: "md",
                  paddingRight: "md",
                  fontSize: "sm",
                  cursor: "pointer",
                  transition: "all 150ms",
                  whiteSpace: "nowrap",
                  _hover: {
                    borderColor: "status.error",
                    color: "status.error",
                  },
                } as any)}
              >
                Remove
              </button>
            </div>
          ) : (
            <div
              className={css({
                display: "flex",
                flexDirection: "column",
                gap: "sm",
              })}
            >
              <div
                className={css({
                  display: "flex",
                  gap: "sm",
                })}
              >
                <input
                  type="password"
                  placeholder="sk-ant-api03-..."
                  value={apiKey}
                  onChange={(e) => {
                    setApiKey(e.target.value);
                    setValidationResult("idle");
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleValidateAndSave();
                    if (e.key === "Escape") {
                      setShowKeyInput(false);
                      setApiKey("");
                      setValidationResult("idle");
                    }
                  }}
                  autoFocus
                  className={css({
                    flex: 1,
                    bg: "bg.surface",
                    border: "1px solid",
                    borderColor:
                      validationResult === "invalid"
                        ? "status.error"
                        : validationResult === "valid"
                        ? "status.success"
                        : "border.base",
                    borderRadius: "md",
                    padding: "sm",
                    paddingLeft: "md",
                    color: "text.primary",
                    fontSize: "sm",
                    fontFamily: "mono",
                    outline: "none",
                    transition: "border-color 200ms",
                    _focus: {
                      borderColor:
                        validationResult === "idle"
                          ? "accent.dim"
                          : undefined,
                    },
                    _placeholder: {
                      color: "text.muted",
                    },
                  } as any)}
                />
                <button
                  onClick={handleValidateAndSave}
                  disabled={!apiKey.trim() || validating}
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
                    transition: "all 150ms",
                    whiteSpace: "nowrap",
                    _hover: {
                      bg: "accent.base",
                      color: "text.inverse",
                    },
                    _disabled: {
                      opacity: 0.4,
                      cursor: "not-allowed",
                    },
                  } as any)}
                >
                  {validating ? "Validating..." : saving ? "Saving..." : "Save"}
                </button>
                {showKeyInput && (
                  <button
                    onClick={() => {
                      setShowKeyInput(false);
                      setApiKey("");
                      setValidationResult("idle");
                    }}
                    className={css({
                      bg: "transparent",
                      border: "1px solid",
                      borderColor: "border.subtle",
                      color: "text.muted",
                      borderRadius: "md",
                      padding: "sm",
                      fontSize: "sm",
                      cursor: "pointer",
                      _hover: { color: "text.primary" },
                    } as any)}
                  >
                    Cancel
                  </button>
                )}
              </div>
              {validationResult === "invalid" && (
                <motion.p
                  initial={{ opacity: 0, y: -4 }}
                  animate={{ opacity: 1, y: 0 }}
                  className={css({
                    fontSize: "xs",
                    color: "status.error",
                  })}
                >
                  Invalid API key. Check your key and try again.
                </motion.p>
              )}
              {validationResult === "valid" && (
                <motion.p
                  initial={{ opacity: 0, y: -4 }}
                  animate={{ opacity: 1, y: 0 }}
                  className={css({
                    fontSize: "xs",
                    color: "status.success",
                  })}
                >
                  API key validated and saved.
                </motion.p>
              )}
            </div>
          )}

          {saved && (
            <motion.div
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className={css({
                fontSize: "xs",
                color: "status.success",
                marginTop: "sm",
              })}
            >
              Settings saved
            </motion.div>
          )}
        </section>

        {/* Model Selection */}
        <section className={css({ marginBottom: "2xl" })}>
          <h2
            className={css({
              fontSize: "md",
              fontWeight: 600,
              color: "text.primary",
              marginBottom: "xs",
            })}
          >
            Extraction Model
          </h2>
          <p
            className={css({
              fontSize: "sm",
              color: "text.muted",
              marginBottom: "md",
              lineHeight: 1.5,
            })}
          >
            Choose which Claude model to use for document extraction
          </p>

          <div
            className={css({
              display: "flex",
              flexDirection: "column",
              gap: "sm",
            })}
          >
            {MODELS.map((model) => (
              <button
                key={model.id}
                onClick={() => handleModelChange(model.id)}
                className={css({
                  display: "flex",
                  alignItems: "center",
                  gap: "md",
                  padding: "md",
                  bg:
                    settings.model === model.id
                      ? "accent.subtle"
                      : "bg.surface",
                  border: "1px solid",
                  borderColor:
                    settings.model === model.id
                      ? "accent.dim"
                      : "border.subtle",
                  borderRadius: "md",
                  cursor: "pointer",
                  transition: "all 150ms",
                  textAlign: "left",
                  _hover: {
                    borderColor:
                      settings.model === model.id
                        ? "accent.base"
                        : "border.base",
                  },
                } as any)}
              >
                {/* Radio dot */}
                <div
                  className={css({
                    width: "16px",
                    height: "16px",
                    borderRadius: "full",
                    border: "2px solid",
                    borderColor:
                      settings.model === model.id
                        ? "accent.base"
                        : "border.base",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    flexShrink: 0,
                  })}
                >
                  {settings.model === model.id && (
                    <motion.div
                      initial={{ scale: 0 }}
                      animate={{ scale: 1 }}
                      className={css({
                        width: "8px",
                        height: "8px",
                        borderRadius: "full",
                        bg: "accent.base",
                      })}
                    />
                  )}
                </div>

                <div>
                  <div
                    className={css({
                      fontSize: "sm",
                      fontWeight: 500,
                      color:
                        settings.model === model.id
                          ? "text.primary"
                          : "text.secondary",
                    })}
                  >
                    {model.label}
                  </div>
                  <div
                    className={css({
                      fontSize: "xs",
                      color: "text.muted",
                      marginTop: "2px",
                    })}
                  >
                    {model.description}
                  </div>
                </div>
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
