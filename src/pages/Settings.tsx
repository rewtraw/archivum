import { useState, useEffect, useRef, useCallback } from "react";
import { css } from "../../styled-system/css";
import { motion } from "framer-motion";
import {
  getSettings,
  saveSettings,
  validateApiKey,
  getEmbeddingStats,
  batchReembed,
  getTasks,
  listWhisperModels,
  downloadWhisperModel,
  deleteWhisperModel,
  selectWhisperModel,
  checkExternalTools,
  checkOllamaStatus,
  listOllamaModels,
  listRecommendedOllamaModels,
  pullOllamaModel,
  deleteOllamaModel,
  formatFileSize,
  getSystemHardware,
  getModelFits,
  getLibraryOverview,
  refreshLibrarySummary,
} from "../lib/api";
import type {
  Settings as SettingsType,
  EmbeddingStatsResult,
  Task,
  WhisperModel,
  ExternalToolsStatus,
  OllamaStatus,
  OllamaModelInfo,
  RecommendedModel,
  HardwareInfo,
  ModelFitInfo,
  LibrarySummary,
} from "../lib/api";

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
  const [cfAccountId, setCfAccountId] = useState("");
  const [cfApiToken, setCfApiToken] = useState("");
  const [showCfInput, setShowCfInput] = useState(false);
  const [cfSaved, setCfSaved] = useState(false);
  const [embedStats, setEmbedStats] = useState<EmbeddingStatsResult | null>(null);
  const [embedTask, setEmbedTask] = useState<Task | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval>>(undefined);
  const [whisperModels, setWhisperModels] = useState<WhisperModel[]>([]);
  const [externalTools, setExternalTools] = useState<ExternalToolsStatus | null>(null);
  const whisperPollRef = useRef<ReturnType<typeof setInterval>>(undefined);
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus | null>(null);
  const [ollamaModels, setOllamaModels] = useState<OllamaModelInfo[]>([]);
  const [recommendedModels, setRecommendedModels] = useState<RecommendedModel[]>([]);
  const [customModelName, setCustomModelName] = useState("");
  const [pullingModel, setPullingModel] = useState<string | null>(null);
  const ollamaPollRef = useRef<ReturnType<typeof setInterval>>(undefined);
  const [hardwareInfo, setHardwareInfo] = useState<HardwareInfo | null>(null);
  const [modelFits, setModelFits] = useState<ModelFitInfo[]>([]);
  const [useCaseFilter, setUseCaseFilter] = useState<string | null>(null);
  const [librarySummary, setLibrarySummary] = useState<LibrarySummary | null>(null);
  const [libraryLoading, setLibraryLoading] = useState(false);
  const [libraryError, setLibraryError] = useState<string | null>(null);

  const refreshOllama = useCallback(() => {
    checkOllamaStatus().then((s) => {
      setOllamaStatus(s);
      if (s.available) {
        listOllamaModels().then(setOllamaModels).catch(() => {});
      }
    }).catch(() => {});
    listRecommendedOllamaModels().then(setRecommendedModels).catch(() => {});
    getSystemHardware().then(setHardwareInfo).catch(() => {});
    getModelFits(30, useCaseFilter ?? undefined).then(setModelFits).catch(() => {});
  }, [useCaseFilter]);

  useEffect(() => {
    getModelFits(30, useCaseFilter ?? undefined).then(setModelFits).catch(() => {});
  }, [useCaseFilter]);

  const refreshEmbedStats = useCallback(() => {
    getEmbeddingStats().then(setEmbedStats).catch(() => {});
  }, []);

  const refreshWhisperModels = useCallback(() => {
    listWhisperModels().then(setWhisperModels).catch(() => {});
  }, []);

  useEffect(() => {
    getSettings().then(setSettings);
    refreshEmbedStats();
    refreshWhisperModels();
    refreshOllama();
    checkExternalTools().then(setExternalTools).catch(() => {});
    getLibraryOverview().then(setLibrarySummary).catch(() => {});
    // Check if there's already a running batch-embed task
    getTasks(20).then((tasks) => {
      const running = tasks.find(
        (t) => t.task_type === "batch-embed" && (t.status === "running" || t.status === "queued")
      );
      if (running) {
        setEmbedTask(running);
        startPolling();
      }
    }).catch(() => {});
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
      if (whisperPollRef.current) clearInterval(whisperPollRef.current);
      if (ollamaPollRef.current) clearInterval(ollamaPollRef.current);
    };
  }, []);

  const startPolling = useCallback(() => {
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const tasks = await getTasks(20);
        const task = tasks.find((t) => t.task_type === "batch-embed");
        if (task) {
          setEmbedTask(task);
          if (task.status === "complete" || task.status === "failed") {
            if (pollRef.current) clearInterval(pollRef.current);
            pollRef.current = undefined;
            refreshEmbedStats();
          }
        }
      } catch {}
    }, 1500);
  }, [refreshEmbedStats]);

  const handleValidateAndSave = async () => {
    if (!apiKey.trim()) return;

    setValidating(true);
    setValidationResult("idle");

    try {
      const valid = await validateApiKey(apiKey.trim());
      setValidationResult(valid ? "valid" : "invalid");

      if (valid) {
        setSaving(true);
        const updated = await saveSettings({ apiKey: apiKey.trim() });
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
    const updated = await saveSettings({ apiKey: "" });
    setSettings(updated);
  };

  const handleModelChange = async (model: string) => {
    const updated = await saveSettings({ model });
    setSettings(updated);
  };

  const handleSaveCloudflare = async () => {
    if (!cfAccountId.trim() || !cfApiToken.trim()) return;
    const updated = await saveSettings({ cloudflareAccountId: cfAccountId.trim(), cloudflareApiToken: cfApiToken.trim() });
    setSettings(updated);
    setShowCfInput(false);
    setCfAccountId("");
    setCfApiToken("");
    setCfSaved(true);
    setTimeout(() => setCfSaved(false), 2000);
  };

  const handleBatchReembed = async () => {
    try {
      await batchReembed();
      // Immediately start polling for progress
      setEmbedTask({ id: "", document_id: null, task_type: "batch-embed", status: "queued", progress: 0, message: "Starting...", error: null, created_at: "" });
      startPolling();
    } catch (e) {
      console.error("Batch reembed failed:", e);
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    try {
      await downloadWhisperModel(modelId);
      refreshWhisperModels();
      // Start polling for download progress
      if (whisperPollRef.current) clearInterval(whisperPollRef.current);
      whisperPollRef.current = setInterval(() => {
        listWhisperModels().then((models) => {
          setWhisperModels(models);
          const downloading = models.some((m) => m.status === "downloading");
          if (!downloading && whisperPollRef.current) {
            clearInterval(whisperPollRef.current);
            whisperPollRef.current = undefined;
          }
        }).catch(() => {});
      }, 1000);
    } catch (e) {
      console.error("Model download failed:", e);
    }
  };

  const handleDeleteModel = async (modelId: string) => {
    try {
      await deleteWhisperModel(modelId);
      refreshWhisperModels();
      getSettings().then(setSettings);
    } catch (e) {
      console.error("Model delete failed:", e);
    }
  };

  const handleSelectModel = async (modelId: string) => {
    try {
      await selectWhisperModel(modelId);
      getSettings().then(setSettings);
    } catch (e) {
      console.error("Model select failed:", e);
    }
  };

  const handleRemoveCloudflare = async () => {
    const updated = await saveSettings({ cloudflareAccountId: "", cloudflareApiToken: "" });
    setSettings(updated);
  };

  const handleProviderChange = async (provider: string) => {
    const updated = await saveSettings({ aiProvider: provider });
    setSettings(updated);
  };

  const handleOllamaModelSelect = async (modelName: string) => {
    const updated = await saveSettings({ ollamaModel: modelName });
    setSettings(updated);
  };

  const handlePullModel = async (name: string) => {
    try {
      setPullingModel(name);
      await pullOllamaModel(name);
      // Poll for completion
      if (ollamaPollRef.current) clearInterval(ollamaPollRef.current);
      ollamaPollRef.current = setInterval(() => {
        listOllamaModels().then((models) => {
          setOllamaModels(models);
          const found = models.find((m) => m.name === name || m.name === name.split(":")[0] + ":latest" || m.name.startsWith(name.split(":")[0]));
          if (found) {
            setPullingModel(null);
            if (ollamaPollRef.current) clearInterval(ollamaPollRef.current);
          }
        }).catch(() => {});
      }, 2000);
      // Also timeout after 10 minutes
      setTimeout(() => {
        setPullingModel(null);
        if (ollamaPollRef.current) clearInterval(ollamaPollRef.current);
        refreshOllama();
      }, 600000);
    } catch (e) {
      console.error("Model pull failed:", e);
      setPullingModel(null);
    }
  };

  const handleDeleteOllamaModel = async (name: string) => {
    try {
      await deleteOllamaModel(name);
      refreshOllama();
    } catch (e) {
      console.error("Delete Ollama model failed:", e);
    }
  };

  const handlePullCustomModel = async () => {
    if (!customModelName.trim()) return;
    await handlePullModel(customModelName.trim());
    setCustomModelName("");
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

        {/* AI Provider */}
        <section className={css({ marginBottom: "2xl" })}>
          <h2
            className={css({
              fontSize: "md",
              fontWeight: 600,
              color: "text.primary",
              marginBottom: "xs",
            })}
          >
            AI Provider
          </h2>
          <p
            className={css({
              fontSize: "sm",
              color: "text.muted",
              marginBottom: "md",
              lineHeight: 1.5,
            })}
          >
            Used for chat, summaries, and metadata enrichment. Document
            extraction (OCR, image-based formats) always uses Claude.
          </p>

          <div className={css({ display: "flex", gap: "sm", marginBottom: "md" })}>
            {[
              { id: "claude", label: "Claude (Hosted)", desc: "Anthropic API — best quality" },
              { id: "ollama", label: "Ollama (Local)", desc: "Free, runs on your Mac" },
            ].map((p) => (
              <button
                key={p.id}
                onClick={() => handleProviderChange(p.id)}
                className={css({
                  flex: 1,
                  padding: "md",
                  bg: settings.ai_provider === p.id ? "accent.subtle" : "bg.surface",
                  border: "1px solid",
                  borderColor: settings.ai_provider === p.id ? "accent.dim" : "border.subtle",
                  borderRadius: "md",
                  cursor: "pointer",
                  transition: "all 150ms",
                  textAlign: "left",
                  _hover: { borderColor: settings.ai_provider === p.id ? "accent.base" : "border.base" },
                } as any)}
              >
                <div className={css({ fontSize: "sm", fontWeight: 500, color: settings.ai_provider === p.id ? "text.primary" : "text.secondary" })}>
                  {p.label}
                </div>
                <div className={css({ fontSize: "xs", color: "text.muted", marginTop: "2px" })}>
                  {p.desc}
                </div>
              </button>
            ))}
          </div>

          {settings.ai_provider === "claude" ? (
            /* Claude model selector */
            <div className={css({ display: "flex", flexDirection: "column", gap: "sm" })}>
              {MODELS.map((model) => (
                <button
                  key={model.id}
                  onClick={() => handleModelChange(model.id)}
                  className={css({
                    display: "flex",
                    alignItems: "center",
                    gap: "md",
                    padding: "md",
                    bg: settings.model === model.id ? "accent.subtle" : "bg.surface",
                    border: "1px solid",
                    borderColor: settings.model === model.id ? "accent.dim" : "border.subtle",
                    borderRadius: "md",
                    cursor: "pointer",
                    transition: "all 150ms",
                    textAlign: "left",
                    _hover: { borderColor: settings.model === model.id ? "accent.base" : "border.base" },
                  } as any)}
                >
                  <div
                    className={css({
                      width: "16px",
                      height: "16px",
                      borderRadius: "full",
                      border: "2px solid",
                      borderColor: settings.model === model.id ? "accent.base" : "border.base",
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
                        className={css({ width: "8px", height: "8px", borderRadius: "full", bg: "accent.base" })}
                      />
                    )}
                  </div>
                  <div>
                    <div className={css({ fontSize: "sm", fontWeight: 500, color: settings.model === model.id ? "text.primary" : "text.secondary" })}>
                      {model.label}
                    </div>
                    <div className={css({ fontSize: "xs", color: "text.muted", marginTop: "2px" })}>
                      {model.description}
                    </div>
                  </div>
                </button>
              ))}
            </div>
          ) : (
            /* Ollama model management */
            <div className={css({ display: "flex", flexDirection: "column", gap: "md" })}>
              {/* Status */}
              <div className={css({ display: "flex", alignItems: "center", gap: "sm", fontSize: "sm" })}>
                <div
                  className={css({
                    width: "8px",
                    height: "8px",
                    borderRadius: "full",
                    bg: ollamaStatus?.available ? "#34d399" : "#f87171",
                  })}
                />
                <span className={css({ color: "text.secondary" })}>
                  {ollamaStatus?.available
                    ? `Ollama running${ollamaStatus.version ? ` (v${ollamaStatus.version})` : ""}`
                    : "Ollama not detected — install from ollama.com"}
                </span>
                <button
                  onClick={refreshOllama}
                  className={css({
                    marginLeft: "auto",
                    fontSize: "xs",
                    color: "text.muted",
                    cursor: "pointer",
                    _hover: { color: "text.primary" },
                  } as any)}
                >
                  Refresh
                </button>
              </div>

              {/* Hardware info */}
              {hardwareInfo && (
                <div className={css({ fontSize: "xs", color: "text.muted", padding: "sm", bg: "bg.surface", borderRadius: "md", border: "1px solid", borderColor: "border.subtle" })}>
                  {hardwareInfo.cpu_name} · {hardwareInfo.total_ram_gb.toFixed(0)} GB{hardwareInfo.unified_memory ? " unified" : ""} memory · {hardwareInfo.backend}
                </div>
              )}

              {ollamaStatus?.available && (
                <>
                  {/* Installed models */}
                  {ollamaModels.length > 0 && (
                    <div className={css({ display: "flex", flexDirection: "column", gap: "sm" })}>
                      <div className={css({ fontSize: "xs", fontWeight: 600, color: "text.muted", textTransform: "uppercase", letterSpacing: "0.05em" })}>
                        Installed Models
                      </div>
                      {ollamaModels.map((m) => (
                        <div
                          key={m.name}
                          className={css({
                            display: "flex",
                            alignItems: "center",
                            gap: "md",
                            padding: "sm",
                            paddingLeft: "md",
                            bg: settings.ollama_model === m.name ? "accent.subtle" : "bg.surface",
                            border: "1px solid",
                            borderColor: settings.ollama_model === m.name ? "accent.dim" : "border.subtle",
                            borderRadius: "md",
                            cursor: "pointer",
                            transition: "all 150ms",
                            _hover: { borderColor: settings.ollama_model === m.name ? "accent.base" : "border.base" },
                          } as any)}
                          onClick={() => handleOllamaModelSelect(m.name)}
                        >
                          <div
                            className={css({
                              width: "16px",
                              height: "16px",
                              borderRadius: "full",
                              border: "2px solid",
                              borderColor: settings.ollama_model === m.name ? "accent.base" : "border.base",
                              display: "flex",
                              alignItems: "center",
                              justifyContent: "center",
                              flexShrink: 0,
                            })}
                          >
                            {settings.ollama_model === m.name && (
                              <motion.div
                                initial={{ scale: 0 }}
                                animate={{ scale: 1 }}
                                className={css({ width: "8px", height: "8px", borderRadius: "full", bg: "accent.base" })}
                              />
                            )}
                          </div>
                          <div className={css({ flex: 1 })}>
                            <div className={css({ fontSize: "sm", fontWeight: 500, color: "text.primary" })}>
                              {m.name}
                            </div>
                            <div className={css({ fontSize: "xs", color: "text.muted", marginTop: "1px" })}>
                              {m.parameter_size || ""}{m.parameter_size && m.family ? " · " : ""}{m.family || ""}{" · "}{formatFileSize(m.size)}
                              {(() => {
                                const fit = modelFits.find((f) => f.installed && (f.name.toLowerCase() === m.name.toLowerCase() || m.name.toLowerCase().startsWith(f.name.toLowerCase().split(":")[0])));
                                return fit ? <>{" · "}<span style={{ color: fit.fit_level === "Perfect" ? "#34d399" : fit.fit_level === "Good" ? "#60a5fa" : "#fbbf24" }}>{fit.fit_level}</span>{" · "}{fit.estimated_tps.toFixed(0)} tok/s</> : null;
                              })()}
                            </div>
                          </div>
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDeleteOllamaModel(m.name);
                            }}
                            className={css({
                              fontSize: "xs",
                              color: "text.muted",
                              cursor: "pointer",
                              padding: "xs",
                              _hover: { color: "#f87171" },
                            } as any)}
                          >
                            Remove
                          </button>
                        </div>
                      ))}
                    </div>
                  )}

                  {/* Recommended models (hardware-scored) */}
                  {modelFits.length > 0 && (
                    <div className={css({ display: "flex", flexDirection: "column", gap: "sm" })}>
                      <div className={css({ display: "flex", alignItems: "center", gap: "sm" })}>
                        <div className={css({ fontSize: "xs", fontWeight: 600, color: "text.muted", textTransform: "uppercase", letterSpacing: "0.05em" })}>
                          Recommended for Your Hardware
                        </div>
                      </div>
                      {/* Use case filter tabs */}
                      <div className={css({ display: "flex", gap: "xs", flexWrap: "wrap" })}>
                        {[null, "General", "Coding", "Reasoning", "Chat", "Multimodal"].map((uc) => (
                          <button
                            key={uc ?? "all"}
                            onClick={() => setUseCaseFilter(uc)}
                            className={css({
                              fontSize: "xs",
                              padding: "2px",
                              paddingLeft: "sm",
                              paddingRight: "sm",
                              borderRadius: "sm",
                              cursor: "pointer",
                              bg: useCaseFilter === uc ? "accent.subtle" : "transparent",
                              color: useCaseFilter === uc ? "accent.base" : "text.muted",
                              border: "1px solid",
                              borderColor: useCaseFilter === uc ? "accent.dim" : "transparent",
                              _hover: { color: "text.primary" },
                            } as any)}
                          >
                            {uc ?? "All"}
                          </button>
                        ))}
                      </div>
                      {modelFits
                        .filter((f) => !f.installed)
                        .slice(0, 12)
                        .map((f) => (
                          <div
                            key={f.name}
                            className={css({
                              display: "flex",
                              alignItems: "center",
                              gap: "md",
                              padding: "sm",
                              paddingLeft: "md",
                              bg: "bg.surface",
                              border: "1px solid",
                              borderColor: "border.subtle",
                              borderRadius: "md",
                            })}
                          >
                            <div className={css({
                              width: "8px",
                              height: "8px",
                              borderRadius: "full",
                              flexShrink: 0,
                              bg: f.fit_level === "Perfect" ? "#34d399" : f.fit_level === "Good" ? "#60a5fa" : "#fbbf24",
                            })} />
                            <div className={css({ flex: 1, minWidth: 0 })}>
                              <div className={css({ display: "flex", alignItems: "center", gap: "sm" })}>
                                <span className={css({ fontSize: "sm", fontWeight: 500, color: "text.primary" })}>
                                  {f.name}
                                </span>
                                <span className={css({ fontSize: "xs", color: "text.muted" })}>
                                  {f.parameter_count}
                                </span>
                              </div>
                              <div className={css({ fontSize: "xs", color: "text.muted", marginTop: "1px", display: "flex", gap: "sm", flexWrap: "wrap" })}>
                                <span>{f.use_case}</span>
                                <span>·</span>
                                <span>{Math.round(f.score)}/100</span>
                                <span>·</span>
                                <span>{f.estimated_tps.toFixed(0)} tok/s</span>
                                <span>·</span>
                                <span>{f.memory_required_gb.toFixed(1)} GB</span>
                                <span>·</span>
                                <span>{f.best_quant}</span>
                              </div>
                            </div>
                            <button
                              onClick={() => handlePullModel(f.name)}
                              disabled={pullingModel !== null}
                              className={css({
                                fontSize: "xs",
                                fontWeight: 500,
                                color: pullingModel === f.name ? "text.muted" : "accent.base",
                                cursor: pullingModel !== null ? "default" : "pointer",
                                padding: "xs",
                                paddingLeft: "sm",
                                paddingRight: "sm",
                                flexShrink: 0,
                                _hover: pullingModel !== null ? {} : { opacity: 0.8 },
                              } as any)}
                            >
                              {pullingModel === f.name ? "Pulling..." : "Pull"}
                            </button>
                          </div>
                        ))}
                    </div>
                  )}

                  {/* Custom model pull */}
                  <div className={css({ display: "flex", gap: "sm", alignItems: "center" })}>
                    <input
                      type="text"
                      value={customModelName}
                      onChange={(e) => setCustomModelName(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && handlePullCustomModel()}
                      placeholder="Custom model name (e.g. qwen3:8b)"
                      className={css({
                        flex: 1,
                        padding: "sm",
                        paddingLeft: "md",
                        bg: "bg.surface",
                        border: "1px solid",
                        borderColor: "border.base",
                        borderRadius: "md",
                        color: "text.primary",
                        fontSize: "sm",
                        outline: "none",
                        _focus: { borderColor: "accent.base" },
                      } as any)}
                    />
                    <button
                      onClick={handlePullCustomModel}
                      disabled={!customModelName.trim() || pullingModel !== null}
                      className={css({
                        padding: "sm",
                        paddingLeft: "md",
                        paddingRight: "md",
                        bg: "accent.base",
                        color: "white",
                        borderRadius: "md",
                        fontSize: "sm",
                        fontWeight: 500,
                        cursor: !customModelName.trim() || pullingModel !== null ? "default" : "pointer",
                        opacity: !customModelName.trim() || pullingModel !== null ? 0.5 : 1,
                        _hover: { opacity: 0.9 },
                      } as any)}
                    >
                      Pull
                    </button>
                  </div>

                  {pullingModel && (
                    <div className={css({ fontSize: "xs", color: "text.muted", fontStyle: "italic" })}>
                      Pulling {pullingModel}... This may take a few minutes.
                    </div>
                  )}
                </>
              )}
            </div>
          )}
        </section>

        {/* Cloudflare Browser Rendering */}
        <section className={css({ marginBottom: "2xl" })}>
          <h2
            className={css({
              fontSize: "md",
              fontWeight: 600,
              color: "text.primary",
              marginBottom: "xs",
            })}
          >
            Cloudflare Browser Rendering
          </h2>
          <p
            className={css({
              fontSize: "sm",
              color: "text.muted",
              marginBottom: "md",
              lineHeight: 1.5,
            })}
          >
            Required for importing webpages. Needs a Cloudflare account with
            Browser Rendering enabled.
          </p>

          {settings.has_cloudflare && !showCfInput ? (
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
                  {settings.cloudflare_account_id_preview}
                </span>
              </div>
              <button
                onClick={() => setShowCfInput(true)}
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
                onClick={handleRemoveCloudflare}
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
              <input
                type="text"
                placeholder="Account ID"
                value={cfAccountId}
                onChange={(e) => setCfAccountId(e.target.value)}
                className={css({
                  bg: "bg.surface",
                  border: "1px solid",
                  borderColor: "border.base",
                  borderRadius: "md",
                  padding: "sm",
                  paddingLeft: "md",
                  color: "text.primary",
                  fontSize: "sm",
                  fontFamily: "mono",
                  outline: "none",
                  transition: "border-color 200ms",
                  _focus: { borderColor: "accent.dim" },
                  _placeholder: { color: "text.muted" },
                } as any)}
              />
              <input
                type="password"
                placeholder="API Token (Browser Rendering - Edit)"
                value={cfApiToken}
                onChange={(e) => setCfApiToken(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSaveCloudflare();
                  if (e.key === "Escape") {
                    setShowCfInput(false);
                    setCfAccountId("");
                    setCfApiToken("");
                  }
                }}
                className={css({
                  bg: "bg.surface",
                  border: "1px solid",
                  borderColor: "border.base",
                  borderRadius: "md",
                  padding: "sm",
                  paddingLeft: "md",
                  color: "text.primary",
                  fontSize: "sm",
                  fontFamily: "mono",
                  outline: "none",
                  transition: "border-color 200ms",
                  _focus: { borderColor: "accent.dim" },
                  _placeholder: { color: "text.muted" },
                } as any)}
              />
              <div className={css({ display: "flex", gap: "sm" })}>
                <button
                  onClick={handleSaveCloudflare}
                  disabled={!cfAccountId.trim() || !cfApiToken.trim()}
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
                    _hover: { bg: "accent.base", color: "text.inverse" },
                    _disabled: { opacity: 0.4, cursor: "not-allowed" },
                  } as any)}
                >
                  Save
                </button>
                {showCfInput && (
                  <button
                    onClick={() => {
                      setShowCfInput(false);
                      setCfAccountId("");
                      setCfApiToken("");
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
            </div>
          )}

          {cfSaved && (
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
              Cloudflare credentials saved
            </motion.div>
          )}
        </section>

        {/* Library Overview */}
        <section className={css({ marginBottom: "2xl" })}>
          <h2
            className={css({
              fontSize: "md",
              fontWeight: 600,
              color: "text.primary",
              marginBottom: "xs",
            })}
          >
            Library Overview
          </h2>
          <p
            className={css({
              fontSize: "sm",
              color: "text.muted",
              marginBottom: "md",
              lineHeight: 1.5,
            })}
          >
            Generate an AI summary of your entire library. This helps the chat
            agent understand the scope and themes of your collection.
          </p>
          {librarySummary && (
            <div
              className={css({
                padding: "md",
                bg: "bg.surface",
                border: "1px solid",
                borderColor: "border.base",
                borderRadius: "md",
                marginBottom: "md",
              })}
            >
              <div className={css({ fontSize: "sm", color: "text.secondary", lineHeight: 1.6 })}>
                {librarySummary.summary}
              </div>
              {librarySummary.themes && (
                <div className={css({ fontSize: "xs", color: "text.muted", marginTop: "sm" })}>
                  Themes: {(() => {
                    try {
                      return JSON.parse(librarySummary.themes).join(", ");
                    } catch {
                      return librarySummary.themes;
                    }
                  })()}
                </div>
              )}
              <div className={css({ fontSize: "xs", color: "text.muted", marginTop: "xs" })}>
                {librarySummary.document_count} documents · Updated {librarySummary.updated_at}
              </div>
            </div>
          )}
          <button
            onClick={async () => {
              setLibraryLoading(true);
              setLibraryError(null);
              try {
                await refreshLibrarySummary();
                const overview = await getLibraryOverview();
                setLibrarySummary(overview);
              } catch (e: any) {
                console.error("Library summary failed:", e);
                setLibraryError(typeof e === "string" ? e : e?.message || "Failed to generate library overview");
              }
              setLibraryLoading(false);
            }}
            disabled={libraryLoading}
            className={css({
              padding: "sm",
              paddingLeft: "md",
              paddingRight: "md",
              bg: libraryLoading ? "bg.elevated" : "accent.base",
              color: "white",
              borderRadius: "md",
              fontSize: "sm",
              fontWeight: 500,
              cursor: libraryLoading ? "default" : "pointer",
              opacity: libraryLoading ? 0.6 : 1,
              _hover: libraryLoading ? {} : { opacity: 0.9 },
            } as any)}
          >
            {libraryLoading ? "Generating..." : librarySummary ? "Regenerate" : "Generate Library Overview"}
          </button>
          {libraryError && (
            <p className={css({ fontSize: "xs", color: "#f87171", marginTop: "sm" })}>
              {libraryError}
            </p>
          )}
        </section>

        {/* Embedding Index */}
        <section className={css({ marginBottom: "2xl" })}>
          <h2
            className={css({
              fontSize: "md",
              fontWeight: 600,
              color: "text.primary",
              marginBottom: "xs",
            })}
          >
            Embedding Index
          </h2>
          <p
            className={css({
              fontSize: "sm",
              color: "text.muted",
              marginBottom: "md",
              lineHeight: 1.5,
            })}
          >
            Documents are indexed locally for semantic search and AI chat.
            Indexing happens automatically on import but you can re-index
            missing documents here.
          </p>

          {embedStats && (
            <div
              className={css({
                display: "flex",
                alignItems: "center",
                gap: "md",
                padding: "md",
                bg: "bg.surface",
                border: "1px solid",
                borderColor: "border.base",
                borderRadius: "md",
                marginBottom: "md",
              })}
            >
              <div className={css({ flex: 1 })}>
                <span
                  className={css({
                    fontSize: "sm",
                    color: "text.primary",
                    fontWeight: 500,
                  })}
                >
                  {embedStats.embedded_documents} / {embedStats.total_documents}
                </span>
                <span
                  className={css({
                    fontSize: "sm",
                    color: "text.muted",
                    marginLeft: "xs",
                  })}
                >
                  documents indexed
                </span>
              </div>
              {embedStats.embedded_documents < embedStats.total_documents &&
                !(embedTask && (embedTask.status === "running" || embedTask.status === "queued")) && (
                <span
                  className={css({
                    fontSize: "xs",
                    color: "accent.bright",
                    bg: "accent.subtle",
                    padding: "2px 8px",
                    borderRadius: "full",
                  })}
                >
                  {embedStats.total_documents - embedStats.embedded_documents} pending
                </span>
              )}
            </div>
          )}

          {/* Progress bar when indexing */}
          {embedTask && (embedTask.status === "running" || embedTask.status === "queued") && (
            <div
              className={css({
                padding: "md",
                bg: "bg.surface",
                border: "1px solid",
                borderColor: "accent.dim",
                borderRadius: "md",
                marginBottom: "md",
              })}
            >
              <div className={css({ display: "flex", justifyContent: "space-between", marginBottom: "sm" })}>
                <span className={css({ fontSize: "sm", color: "text.primary" })}>
                  {embedTask.message || "Starting..."}
                </span>
                <span className={css({ fontSize: "xs", color: "text.muted", fontFamily: "mono" })}>
                  {Math.round(embedTask.progress * 100)}%
                </span>
              </div>
              <div className={css({ width: "100%", height: "4px", bg: "rgba(255,255,255,0.08)", borderRadius: "full", overflow: "hidden" })}>
                <motion.div
                  animate={{ width: `${Math.max(2, embedTask.progress * 100)}%` }}
                  transition={{ type: "spring", stiffness: 300, damping: 30 }}
                  className={css({ height: "100%", bg: "accent.base", borderRadius: "full" })}
                />
              </div>
            </div>
          )}

          {/* Completed / failed status */}
          {embedTask && embedTask.status === "complete" && (
            <motion.div
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              className={css({
                fontSize: "sm",
                color: "status.success",
                marginBottom: "md",
                padding: "sm md",
                bg: "rgba(52, 211, 153, 0.08)",
                borderRadius: "md",
              })}
            >
              {embedTask.message}
            </motion.div>
          )}
          {embedTask && embedTask.status === "failed" && (
            <motion.div
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              className={css({
                fontSize: "sm",
                color: "status.error",
                marginBottom: "md",
                padding: "sm md",
                bg: "rgba(239, 68, 68, 0.08)",
                borderRadius: "md",
              })}
            >
              {embedTask.error || embedTask.message || "Indexing failed"}
            </motion.div>
          )}

          <button
            onClick={handleBatchReembed}
            disabled={
              (embedTask != null && (embedTask.status === "running" || embedTask.status === "queued")) ||
              !embedStats ||
              embedStats.embedded_documents >= embedStats.total_documents
            }
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
              _hover: { bg: "accent.base", color: "text.inverse" },
              _disabled: { opacity: 0.4, cursor: "not-allowed" },
            } as any)}
          >
            {embedTask && (embedTask.status === "running" || embedTask.status === "queued")
              ? "Indexing..."
              : "Index All Documents"}
          </button>
        </section>

        {/* Whisper Models */}
        <section className={css({ marginBottom: "2xl" })}>
          <h2
            className={css({
              fontSize: "md",
              fontWeight: 600,
              color: "text.primary",
              marginBottom: "xs",
            })}
          >
            Transcription Models
          </h2>
          <p
            className={css({
              fontSize: "sm",
              color: "text.muted",
              marginBottom: "md",
              lineHeight: 1.5,
            })}
          >
            Local Whisper models for audio and video transcription. Larger models
            are more accurate but use more memory and are slower.
          </p>

          {/* External tools status */}
          {externalTools && (
            <div
              className={css({
                display: "flex",
                gap: "lg",
                padding: "md",
                bg: "bg.surface",
                border: "1px solid",
                borderColor: "border.base",
                borderRadius: "md",
                marginBottom: "md",
              })}
            >
              {[
                { name: "ffmpeg", available: externalTools.ffmpeg_available },
                { name: "yt-dlp", available: externalTools.yt_dlp_available },
              ].map((tool) => (
                <div
                  key={tool.name}
                  className={css({ display: "flex", alignItems: "center", gap: "xs" })}
                >
                  <div
                    className={css({
                      width: "6px",
                      height: "6px",
                      borderRadius: "full",
                      bg: tool.available ? "status.success" : "status.error",
                    })}
                  />
                  <span
                    className={css({
                      fontSize: "xs",
                      fontFamily: "mono",
                      color: tool.available ? "text.secondary" : "text.muted",
                    })}
                  >
                    {tool.name}
                  </span>
                </div>
              ))}
              {(!externalTools.ffmpeg_available || !externalTools.yt_dlp_available) && (
                <span className={css({ fontSize: "xs", color: "text.muted" })}>
                  Install via: brew install ffmpeg yt-dlp
                </span>
              )}
            </div>
          )}

          {/* Model list */}
          <div
            className={css({
              display: "flex",
              flexDirection: "column",
              gap: "sm",
            })}
          >
            {whisperModels.map((model) => {
              const isSelected = settings?.selected_whisper_model === model.id;
              const isReady = model.status === "ready";
              const isDownloading = model.status === "downloading";

              return (
                <div
                  key={model.id}
                  className={css({
                    display: "flex",
                    alignItems: "center",
                    gap: "md",
                    padding: "md",
                    bg: isSelected ? "accent.subtle" : "bg.surface",
                    border: "1px solid",
                    borderColor: isSelected ? "accent.dim" : "border.subtle",
                    borderRadius: "md",
                    transition: "all 150ms",
                  })}
                >
                  {/* Radio dot (only for ready models) */}
                  {isReady && (
                    <button
                      onClick={() => handleSelectModel(model.id)}
                      className={css({
                        width: "16px",
                        height: "16px",
                        borderRadius: "full",
                        border: "2px solid",
                        borderColor: isSelected ? "accent.base" : "border.base",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        flexShrink: 0,
                        bg: "transparent",
                        cursor: "pointer",
                        padding: 0,
                      })}
                    >
                      {isSelected && (
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
                    </button>
                  )}

                  <div className={css({ flex: 1, minWidth: 0 })}>
                    <div
                      className={css({
                        fontSize: "sm",
                        fontWeight: 500,
                        color: isSelected ? "text.primary" : "text.secondary",
                      })}
                    >
                      {model.name}
                    </div>
                    <div
                      className={css({
                        fontSize: "xs",
                        color: "text.muted",
                        marginTop: "2px",
                      })}
                    >
                      {formatFileSize(model.size_bytes)}
                    </div>

                    {/* Download progress bar */}
                    {isDownloading && (
                      <div className={css({ marginTop: "sm" })}>
                        <div
                          className={css({
                            width: "100%",
                            height: "3px",
                            bg: "rgba(255,255,255,0.08)",
                            borderRadius: "full",
                            overflow: "hidden",
                          })}
                        >
                          <motion.div
                            animate={{ width: `${Math.max(2, model.download_progress * 100)}%` }}
                            transition={{ type: "spring", stiffness: 300, damping: 30 }}
                            className={css({
                              height: "100%",
                              bg: "accent.base",
                              borderRadius: "full",
                            })}
                          />
                        </div>
                        <span
                          className={css({
                            fontSize: "xs",
                            color: "text.muted",
                            fontFamily: "mono",
                          })}
                        >
                          {Math.round(model.download_progress * 100)}%
                        </span>
                      </div>
                    )}

                    {model.status === "error" && model.error && (
                      <div
                        className={css({
                          fontSize: "xs",
                          color: "status.error",
                          marginTop: "xs",
                        })}
                      >
                        {model.error}
                      </div>
                    )}
                  </div>

                  {/* Actions */}
                  {model.status === "available" && (
                    <button
                      onClick={() => handleDownloadModel(model.id)}
                      className={css({
                        bg: "accent.subtle",
                        color: "accent.bright",
                        border: "1px solid",
                        borderColor: "accent.dim",
                        borderRadius: "md",
                        padding: "xs sm",
                        fontSize: "xs",
                        fontWeight: 500,
                        cursor: "pointer",
                        transition: "all 150ms",
                        whiteSpace: "nowrap",
                        _hover: { bg: "accent.base", color: "text.inverse" },
                      } as any)}
                    >
                      Download
                    </button>
                  )}
                  {model.status === "error" && (
                    <button
                      onClick={() => handleDownloadModel(model.id)}
                      className={css({
                        bg: "transparent",
                        color: "text.muted",
                        border: "1px solid",
                        borderColor: "border.subtle",
                        borderRadius: "md",
                        padding: "xs sm",
                        fontSize: "xs",
                        cursor: "pointer",
                        transition: "all 150ms",
                        whiteSpace: "nowrap",
                        _hover: { color: "text.primary" },
                      } as any)}
                    >
                      Retry
                    </button>
                  )}
                  {isReady && !isSelected && (
                    <button
                      onClick={() => handleDeleteModel(model.id)}
                      className={css({
                        bg: "transparent",
                        border: "1px solid",
                        borderColor: "border.subtle",
                        color: "text.muted",
                        borderRadius: "md",
                        padding: "xs sm",
                        fontSize: "xs",
                        cursor: "pointer",
                        transition: "all 150ms",
                        whiteSpace: "nowrap",
                        _hover: {
                          borderColor: "status.error",
                          color: "status.error",
                        },
                      } as any)}
                    >
                      Delete
                    </button>
                  )}
                  {isDownloading && (
                    <span
                      className={css({
                        fontSize: "xs",
                        color: "text.muted",
                      })}
                    >
                      Downloading...
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </section>
      </div>
    </div>
  );
}
