use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// --- Model manifest ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperModel {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub size_bytes: i64,
    pub status: String,
    pub download_progress: f64,
    pub downloaded_at: Option<String>,
    pub error: Option<String>,
}

pub struct WhisperModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub filename: &'static str,
    pub size_bytes: i64,
    pub url: &'static str,
}

pub const WHISPER_MODELS: &[WhisperModelInfo] = &[
    WhisperModelInfo {
        id: "tiny-en",
        name: "Tiny (English)",
        filename: "ggml-tiny.en.bin",
        size_bytes: 77_691_713,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
    },
    WhisperModelInfo {
        id: "base-en",
        name: "Base (English)",
        filename: "ggml-base.en.bin",
        size_bytes: 147_951_465,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
    },
    WhisperModelInfo {
        id: "small-en",
        name: "Small (English)",
        filename: "ggml-small.en.bin",
        size_bytes: 487_601_967,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
    },
    WhisperModelInfo {
        id: "medium-en",
        name: "Medium (English)",
        filename: "ggml-medium.en.bin",
        size_bytes: 1_533_774_781,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
    },
    WhisperModelInfo {
        id: "large-v3-turbo",
        name: "Large v3 Turbo",
        filename: "ggml-large-v3-turbo.bin",
        size_bytes: 1_622_243_553,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
    },
];

// --- Binary detection ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalToolsStatus {
    pub yt_dlp_available: bool,
    pub yt_dlp_path: Option<String>,
    pub ffmpeg_available: bool,
    pub ffmpeg_path: Option<String>,
}

pub fn find_binary(name: &str) -> Option<PathBuf> {
    let homebrew_paths = ["/opt/homebrew/bin", "/usr/local/bin"];
    for base in &homebrew_paths {
        let path = PathBuf::from(base).join(name);
        if path.exists() {
            return Some(path);
        }
    }
    // Fall back to `which`
    std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()))
}

pub fn check_external_tools() -> ExternalToolsStatus {
    let yt_dlp = find_binary("yt-dlp");
    let ffmpeg = find_binary("ffmpeg");
    ExternalToolsStatus {
        yt_dlp_available: yt_dlp.is_some(),
        yt_dlp_path: yt_dlp.map(|p| p.to_string_lossy().to_string()),
        ffmpeg_available: ffmpeg.is_some(),
        ffmpeg_path: ffmpeg.map(|p| p.to_string_lossy().to_string()),
    }
}

// --- Audio conversion ---

/// Convert any audio/video file to 16kHz mono WAV suitable for whisper
pub fn convert_to_wav(input: &Path, output: &Path) -> Result<(), String> {
    let ffmpeg = find_binary("ffmpeg")
        .ok_or_else(|| "ffmpeg not found. Install via: brew install ffmpeg".to_string())?;

    let result = std::process::Command::new(ffmpeg)
        .args([
            "-i",
            input.to_str().unwrap(),
            "-vn",            // no video
            "-acodec",
            "pcm_s16le",      // 16-bit PCM
            "-ar",
            "16000",          // 16kHz
            "-ac",
            "1",              // mono
            "-y",             // overwrite
            output.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("ffmpeg conversion failed: {}", stderr));
    }
    Ok(())
}

/// Get duration of a media file in seconds using ffprobe
pub fn get_duration(path: &Path) -> Option<f64> {
    let ffprobe = find_binary("ffprobe")?;
    let output = std::process::Command::new(ffprobe)
        .args([
            "-v", "quiet",
            "-show_entries", "format=duration",
            "-of", "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
}

// --- YouTube download ---

#[derive(Debug)]
pub struct YtDlpResult {
    pub audio_path: PathBuf,
    pub title: String,
    pub uploader: String,
    pub duration: f64,
    pub description: String,
}

pub fn download_youtube_audio(url: &str, temp_dir: &Path) -> Result<YtDlpResult, String> {
    let yt_dlp = find_binary("yt-dlp")
        .ok_or_else(|| "yt-dlp not found. Install via: brew install yt-dlp".to_string())?;

    // Ensure ffmpeg is available (yt-dlp needs it for audio extraction)
    find_binary("ffmpeg")
        .ok_or_else(|| "ffmpeg not found. Install via: brew install ffmpeg".to_string())?;

    std::fs::create_dir_all(temp_dir)
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let output_template = temp_dir.join("%(id)s.%(ext)s");

    let result = std::process::Command::new(&yt_dlp)
        .args([
            "-f", "ba",
            "--extract-audio",
            "--audio-format", "wav",
            "--audio-quality", "0",
            "-o", output_template.to_str().unwrap(),
            "--no-playlist",
            "--print-json",
            url,
        ])
        .output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("yt-dlp failed: {}", stderr));
    }

    // Parse the JSON metadata from stdout
    let stdout = String::from_utf8_lossy(&result.stdout);
    let meta: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse yt-dlp output: {}", e))?;

    let video_id = meta["id"].as_str().unwrap_or("unknown");
    let title = meta["title"].as_str().unwrap_or("Untitled").to_string();
    let uploader = meta["uploader"].as_str().unwrap_or("Unknown").to_string();
    let duration = meta["duration"].as_f64().unwrap_or(0.0);
    let description = meta["description"].as_str().unwrap_or("").to_string();

    let audio_path = temp_dir.join(format!("{}.wav", video_id));
    if !audio_path.exists() {
        // yt-dlp may have used a different extension; find the file
        let entries: Vec<_> = std::fs::read_dir(temp_dir)
            .map_err(|e| format!("Failed to read temp dir: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with(video_id)
            })
            .collect();

        if let Some(entry) = entries.first() {
            // Convert to wav if needed
            let found = entry.path();
            if found.extension().and_then(|e| e.to_str()) != Some("wav") {
                let wav_out = temp_dir.join(format!("{}.wav", video_id));
                convert_to_wav(&found, &wav_out)?;
                let _ = std::fs::remove_file(&found);
                return Ok(YtDlpResult {
                    audio_path: wav_out,
                    title,
                    uploader,
                    duration,
                    description,
                });
            }
            return Ok(YtDlpResult {
                audio_path: found,
                title,
                uploader,
                duration,
                description,
            });
        }

        return Err("yt-dlp did not produce an audio file".to_string());
    }

    Ok(YtDlpResult {
        audio_path,
        title,
        uploader,
        duration,
        description,
    })
}

// --- Whisper transcription ---

pub struct TranscriptionSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

pub struct TranscriptionResult {
    pub segments: Vec<TranscriptionSegment>,
    pub full_text: String,
}

/// Read a WAV file and return f32 PCM samples
fn read_wav_samples(path: &Path) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV: {}", e))?;

    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16000 {
        return Err(format!(
            "Expected 16kHz mono WAV, got {}Hz {}ch",
            spec.sample_rate, spec.channels
        ));
    }

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            reader
                .into_samples::<i16>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / 32768.0)
                .collect()
        }
        hound::SampleFormat::Float => {
            reader
                .into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
    };

    Ok(samples)
}

/// Run whisper transcription on a 16kHz mono WAV file
pub fn transcribe(wav_path: &Path, model_path: &Path) -> Result<TranscriptionResult, String> {
    let ctx = whisper_rs::WhisperContext::new_with_params(
        model_path.to_str().unwrap(),
        whisper_rs::WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load whisper model: {}", e))?;

    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Failed to create whisper state: {}", e))?;

    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_translate(false);
    params.set_no_timestamps(false);
    // Use half the available cores to leave headroom for the UI
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32 / 2)
        .unwrap_or(4)
        .max(1);
    params.set_n_threads(n_threads);

    let audio_data = read_wav_samples(wav_path)?;

    state
        .full(params, &audio_data)
        .map_err(|e| format!("Whisper transcription failed: {}", e))?;

    let num_segments = state
        .full_n_segments()
        .map_err(|e| format!("Failed to get segment count: {}", e))?;

    let mut segments = Vec::new();
    let mut full_text = String::new();

    for i in 0..num_segments {
        let start = state.full_get_segment_t0(i)
            .map_err(|e| format!("Failed to get segment start: {}", e))?;
        let end = state.full_get_segment_t1(i)
            .map_err(|e| format!("Failed to get segment end: {}", e))?;
        let text = state.full_get_segment_text(i)
            .map_err(|e| format!("Failed to get segment text: {}", e))?;

        segments.push(TranscriptionSegment {
            start_ms: start as i64 * 10, // whisper timestamps are in centiseconds
            end_ms: end as i64 * 10,
            text: text.clone(),
        });

        if !full_text.is_empty() {
            full_text.push(' ');
        }
        full_text.push_str(text.trim());
    }

    Ok(TranscriptionResult { segments, full_text })
}

// --- Format transcript as markdown ---

fn format_timestamp(ms: i64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

fn format_duration(seconds: f64) -> String {
    let total = seconds as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

pub fn format_transcript_markdown(
    result: &TranscriptionResult,
    title: &str,
    source_url: Option<&str>,
    duration: Option<f64>,
    model_name: &str,
) -> String {
    let mut md = format!("# {}\n\n", title);

    if let Some(url) = source_url {
        md.push_str(&format!("**Source:** {}\n", url));
    }
    if let Some(dur) = duration {
        md.push_str(&format!("**Duration:** {}\n", format_duration(dur)));
    }
    md.push_str(&format!("**Transcribed:** {} using {}\n", chrono::Local::now().format("%Y-%m-%d"), model_name));
    md.push_str("\n---\n\n");

    // Group segments into paragraphs based on pauses (>2 seconds gap)
    let mut current_paragraph = String::new();
    let mut paragraph_start_ms: Option<i64> = None;
    let mut last_end_ms: i64 = 0;

    for seg in &result.segments {
        let gap = seg.start_ms - last_end_ms;

        if gap > 2000 && !current_paragraph.is_empty() {
            // Flush the current paragraph
            if let Some(start) = paragraph_start_ms {
                md.push_str(&format!("[{}] ", format_timestamp(start)));
            }
            md.push_str(current_paragraph.trim());
            md.push_str("\n\n");
            current_paragraph.clear();
            paragraph_start_ms = None;
        }

        if paragraph_start_ms.is_none() {
            paragraph_start_ms = Some(seg.start_ms);
        }

        current_paragraph.push_str(seg.text.trim());
        current_paragraph.push(' ');
        last_end_ms = seg.end_ms;
    }

    // Flush final paragraph
    if !current_paragraph.is_empty() {
        if let Some(start) = paragraph_start_ms {
            md.push_str(&format!("[{}] ", format_timestamp(start)));
        }
        md.push_str(current_paragraph.trim());
        md.push('\n');
    }

    md
}

// --- Model download ---

/// Download a model file with progress reporting
pub fn download_model(
    url: &str,
    dest: &Path,
    progress_callback: &dyn Fn(f64),
) -> Result<(), String> {
    std::fs::create_dir_all(dest.parent().unwrap())
        .map_err(|e| format!("Failed to create models dir: {}", e))?;

    let part_path = dest.with_extension("bin.part");

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create runtime: {}", e))?;

    rt.block_on(async {
        let client = reqwest::Client::new();
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Download request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Download failed with status: {}", resp.status()));
        }

        let total_size = resp.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(&part_path)
            .await
            .map_err(|e| format!("Failed to create file: {}", e))?;

        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
            file.write_all(&bytes)
                .await
                .map_err(|e| format!("Write error: {}", e))?;
            downloaded += bytes.len() as u64;
            if total_size > 0 {
                progress_callback(downloaded as f64 / total_size as f64);
            }
        }

        file.flush()
            .await
            .map_err(|e| format!("Flush error: {}", e))?;

        Ok(())
    })?;

    // Rename .part to final path
    std::fs::rename(&part_path, dest)
        .map_err(|e| format!("Failed to rename model file: {}", e))?;

    Ok(())
}

/// Get the models directory within app data
pub fn models_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join("whisper")
}

/// Check if a specific model is downloaded
pub fn is_model_downloaded(data_dir: &Path, filename: &str) -> bool {
    models_dir(data_dir).join(filename).exists()
}

/// Get the path to a downloaded model
pub fn model_path(data_dir: &Path, filename: &str) -> PathBuf {
    models_dir(data_dir).join(filename)
}

/// Delete a downloaded model file
pub fn delete_model_file(data_dir: &Path, filename: &str) -> Result<(), String> {
    let path = models_dir(data_dir).join(filename);
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete model: {}", e))?;
    }
    Ok(())
}

// --- Media format detection ---

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "wav", "flac", "ogg", "aac", "wma"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "wmv"];

pub fn is_audio_format(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

pub fn is_video_format(ext: &str) -> bool {
    VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

pub fn is_media_format(ext: &str) -> bool {
    is_audio_format(ext) || is_video_format(ext)
}
